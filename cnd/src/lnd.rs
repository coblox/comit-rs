use crate::swap_protocols::{
    halight::{data, Accepted, Cancelled, Opened, Params, Settled},
    rfc003::{Secret, SecretHash},
};
use anyhow::{Context, Error};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Certificate, StatusCode, Url,
};
use serde::Deserialize;
use std::{io::Read, path::PathBuf, time::Duration};

/// Invoice states.  These mirror the invoice states used by lnd.
// ref: https://api.lightning.community/#invoicestate
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, strum_macros::Display)]
#[serde(untagged)]
enum InvoiceState {
    #[serde(rename = "0")]
    Opened,
    #[serde(rename = "1")]
    Settled,
    #[serde(rename = "2")]
    Cancelled,
    #[serde(rename = "3")]
    Accepted,
}

/// Payment status.  These mirror the payment status' used by lnd.
// ref: https://api.lightning.community/#paymentstatus
#[derive(Copy, Clone, Debug, Deserialize, PartialEq)]
enum PaymentStatus {
    #[serde(rename = "0")]
    Unknown,
    #[serde(rename = "1")]
    InFlight,
    #[serde(rename = "2")]
    Succeed,
    #[serde(rename = "3")]
    Failed,
}

#[derive(Debug, Deserialize)]
struct Invoice {
    pub value: Option<String>,
    pub value_msat: Option<String>,
    pub r_hash: SecretHash,
    pub amt_paid_sat: Option<String>,
    pub amt_paid_msat: Option<String>,
    pub settled: bool,
    pub cltv_expiry: String,
    pub state: InvoiceState,
    pub r_preimage: Secret,
}

#[derive(Clone, Debug, Deserialize)]
struct Payment {
    pub value_msat: Option<String>,
    pub payment_preimage: Option<Secret>,
    pub status: PaymentStatus,
    pub payment_hash: SecretHash,
}

#[derive(Clone, Debug)]
pub struct LndConnectorParams {
    pub lnd_url: Url,
    pub retry_interval_ms: u64,
    pub certificate_path: PathBuf,
    pub macaroon_path: PathBuf,
}

#[derive(Clone, Debug)]
enum LazyCertificate {
    Path(PathBuf),
    Certificate(Certificate),
}

impl LazyCertificate {
    pub fn new(path: PathBuf) -> Self {
        Self::Path(path)
    }

    pub fn read(self) -> Result<Self, Error> {
        use LazyCertificate::*;

        match self {
            Certificate(_) => Ok(self),
            Path(path) => {
                let mut buf = Vec::new();
                std::fs::File::open(path)?.read_to_end(&mut buf)?;
                let certificate = reqwest::Certificate::from_pem(&buf)?;
                Ok(LazyCertificate::Certificate(certificate))
            }
        }
    }

    pub fn certificate(&self) -> Result<&Certificate, Error> {
        use LazyCertificate::*;
        match self {
            Path(_) => Err(anyhow::anyhow!("Certificate was not read.")),
            Certificate(certificate) => Ok(certificate),
        }
    }
}

// TODO remove this duplication with LazyCertificate
#[derive(Clone, Debug)]
enum LazyMacaroon {
    Path(PathBuf),
    Macaroon(String), // already hex encoded
}

impl LazyMacaroon {
    pub fn new(path: PathBuf) -> Self {
        Self::Path(path)
    }

    pub fn read(self) -> Result<Self, Error> {
        use LazyMacaroon::*;

        match self {
            Macaroon(_) => Ok(self),
            Path(path) => {
                let mut buf = Vec::new();
                std::fs::File::open(path)?.read_to_end(&mut buf)?;
                let hex = hex::encode(buf);

                Ok(LazyMacaroon::Macaroon(hex))
            }
        }
    }

    pub fn macaroon(&self) -> Result<&str, Error> {
        use LazyMacaroon::*;
        match self {
            Path(_) => Err(anyhow::anyhow!("Macaroon was not read.")),
            Macaroon(hex) => Ok(hex.as_ref()),
        }
    }
}

// TODO: remove the duplication between these connectors by having one low-level
// connector that is responsible for the connection and two more high-level ones
// that implement the actual traits

/// LND connector for connecting to an LND node when sending a lightning
/// payment.
///
/// When connecting to LND as the sender all state decisions must be made based
/// on the payment status.  This is because only the receiver has the invoice,
/// the sender makes payments using the swap parameters.
#[derive(Clone, Debug)]
pub struct LndConnectorAsSender {
    lnd_url: Url,
    retry_interval_ms: u64,
    certificate: LazyCertificate,
    macaroon: LazyMacaroon,
}

impl From<LndConnectorParams> for LndConnectorAsSender {
    fn from(params: LndConnectorParams) -> Self {
        Self {
            lnd_url: params.lnd_url,
            retry_interval_ms: params.retry_interval_ms,
            certificate: LazyCertificate::new(params.certificate_path),
            macaroon: LazyMacaroon::new(params.macaroon_path),
        }
    }
}

impl LndConnectorAsSender {
    pub fn read_certificate(self) -> Result<Self, Error> {
        Ok(Self {
            certificate: self.certificate.read()?,
            ..self
        })
    }

    pub fn read_macaroon(self) -> Result<Self, Error> {
        Ok(Self {
            macaroon: self.macaroon.read()?,
            ..self
        })
    }

    fn payment_url(&self) -> Url {
        self.lnd_url
            .join("/v1/payments?include_incomplete=true")
            .expect("append valid string to url")
    }

    async fn find_payment(
        &self,
        secret_hash: SecretHash,
        status: PaymentStatus,
    ) -> Result<Option<Payment>, Error> {
        let payments = client(self.certificate.certificate()?, self.macaroon.macaroon()?)?
            .get(self.payment_url())
            .send()
            .await?
            .json::<Vec<Payment>>()
            .await?;
        let payment = payments
            .iter()
            .find(|&payment| payment.payment_hash == secret_hash && payment.status == status);

        Ok(payment.cloned())
    }
}

#[async_trait::async_trait]
impl<L, A, I> Opened<L, A, I> for LndConnectorAsSender
where
    L: Send + 'static,
    A: Send + 'static,
    I: Send + 'static,
{
    async fn opened(&self, _params: Params<L, A, I>) -> Result<data::Opened, Error> {
        // At this stage there is no way for the sender to know when the invoice is
        // added on receiver's side.
        Ok(data::Opened)
    }
}

#[async_trait::async_trait]
impl<L, A, I> Accepted<L, A, I> for LndConnectorAsSender
where
    L: Send + 'static,
    A: Send + 'static,
    I: Send + 'static,
{
    async fn accepted(&self, params: Params<L, A, I>) -> Result<data::Accepted, Error> {
        // No validation of the parameters because once the payment has been
        // sent the sender cannot cancel it.
        while self
            .find_payment(params.secret_hash, PaymentStatus::InFlight)
            .await?
            .is_none()
        {
            tokio::time::delay_for(Duration::from_millis(self.retry_interval_ms)).await;
        }

        Ok(data::Accepted)
    }
}

#[async_trait::async_trait]
impl<L, A, I> Settled<L, A, I> for LndConnectorAsSender
where
    A: Send + 'static,
    L: Send + 'static,
    I: Send + 'static,
{
    async fn settled(&self, params: Params<L, A, I>) -> Result<data::Settled, Error> {
        let payment = loop {
            match self
                .find_payment(params.secret_hash, PaymentStatus::Succeed)
                .await?
            {
                Some(payment) => break payment,
                None => {
                    tokio::time::delay_for(Duration::from_millis(self.retry_interval_ms)).await;
                }
            }
        };

        let secret = match payment.payment_preimage {
            Some(secret) => Ok(secret),
            None => Err(anyhow::anyhow!(
                "Pre-image is not present on lnd response for a successful payment: {}",
                params.secret_hash
            )),
        }?;
        Ok(data::Settled { secret })
    }
}

#[async_trait::async_trait]
impl<L, A, I> Cancelled<L, A, I> for LndConnectorAsSender
where
    L: Send + 'static,
    A: Send + 'static,
    I: Send + 'static,
{
    async fn cancelled(&self, params: Params<L, A, I>) -> Result<data::Cancelled, Error> {
        while self
            .find_payment(params.secret_hash, PaymentStatus::Failed)
            .await?
            .is_none()
        {
            tokio::time::delay_for(Duration::from_millis(self.retry_interval_ms)).await;
        }

        Ok(data::Cancelled)
    }
}

/// LND connector for connecting to an LND node when receiving a lightning
/// payment.
///
/// When connecting to LND as the receiver all state decisions can be made based
/// on the invoice state.  Since as the receiver, we add the invoice we have
/// access to its state.
#[derive(Clone, Debug)]
pub struct LndConnectorAsReceiver {
    lnd_url: Url,
    retry_interval_ms: u64,
    certificate: LazyCertificate,
    macaroon: LazyMacaroon,
}

impl From<LndConnectorParams> for LndConnectorAsReceiver {
    fn from(params: LndConnectorParams) -> Self {
        Self {
            lnd_url: params.lnd_url,
            retry_interval_ms: params.retry_interval_ms,
            certificate: LazyCertificate::new(params.certificate_path),
            macaroon: LazyMacaroon::new(params.macaroon_path),
        }
    }
}

impl LndConnectorAsReceiver {
    pub fn read_certificate(self) -> Result<Self, Error> {
        Ok(Self {
            certificate: self.certificate.read()?,
            ..self
        })
    }

    fn invoice_url(&self, secret_hash: SecretHash) -> Result<Url, Error> {
        Ok(self
            .lnd_url
            .join("/v1/invoice/")
            .expect("append valid string to url")
            .join(format!("{:x}", secret_hash).as_str())?)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn find_invoice(
        &self,
        secret_hash: SecretHash,
        expected_state: InvoiceState,
    ) -> Result<Option<Invoice>, Error> {
        let response = client(self.certificate.certificate()?, self.macaroon.macaroon()?)?
            .get(self.invoice_url(secret_hash)?)
            .send()
            .await?;

        if response.status() == StatusCode::NOT_FOUND {
            tracing::debug!("invoice not found");
            return Ok(None);
        }

        if !response.status().is_success() {
            let status_code = response.status();
            let lnd_error = response
                .json::<LndError>()
                .await
                // yes we can fail while we already encoundered an error ...
                .with_context(|| format!("encountered {} while fetching invoice but couldn't deserialize error response🙄🙄🙄 🙄", status_code))?;

            return Err(lnd_error.into());
        }

        let invoice = response
            .json::<Invoice>()
            .await
            .context("failed to deserialize response as invoice")?;

        if invoice.state == expected_state {
            Ok(Some(invoice))
        } else {
            tracing::debug!("invoice exists but is in state {}", invoice.state);
            Ok(None)
        }
    }
}

#[derive(Deserialize, Debug, thiserror::Error)]
#[error("{message}")]
struct LndError {
    error: String,
    message: String,
    code: u32,
}

#[async_trait::async_trait]
impl<L, A, I> Opened<L, A, I> for LndConnectorAsReceiver
where
    L: Send + 'static,
    A: Send + 'static,
    I: Send + 'static,
{
    async fn opened(&self, params: Params<L, A, I>) -> Result<data::Opened, Error> {
        // FIXME: Do we want to validate that the user used the correct swap parameters
        // when adding the invoice?
        while self
            .find_invoice(params.secret_hash, InvoiceState::Opened)
            .await?
            .is_none()
        {
            tokio::time::delay_for(Duration::from_millis(self.retry_interval_ms)).await;
        }

        Ok(data::Opened)
    }
}

#[async_trait::async_trait]
impl<L, A, I> Accepted<L, A, I> for LndConnectorAsReceiver
where
    L: Send + 'static,
    A: Send + 'static,
    I: Send + 'static,
{
    async fn accepted(&self, params: Params<L, A, I>) -> Result<data::Accepted, Error> {
        // Validation that sender payed the correct invoice is provided by LND.
        // Since the sender uses the params to make the payment (as apposed to
        // the invoice) LND guarantees that the params match the invoice when
        // updating the invoice status.
        while self
            .find_invoice(params.secret_hash, InvoiceState::Accepted)
            .await?
            .is_none()
        {
            tokio::time::delay_for(Duration::from_millis(self.retry_interval_ms)).await;
        }
        Ok(data::Accepted)
    }
}

#[async_trait::async_trait]
impl<L, A, I> Settled<L, A, I> for LndConnectorAsReceiver
where
    L: Send + 'static,
    A: Send + 'static,
    I: Send + 'static,
{
    async fn settled(&self, params: Params<L, A, I>) -> Result<data::Settled, Error> {
        let invoice = loop {
            match self
                .find_invoice(params.secret_hash, InvoiceState::Settled)
                .await?
            {
                Some(invoice) => break invoice,
                None => tokio::time::delay_for(Duration::from_millis(self.retry_interval_ms)).await,
            }
        };

        Ok(data::Settled {
            secret: invoice.r_preimage,
        })
    }
}

#[async_trait::async_trait]
impl<L, A, I> Cancelled<L, A, I> for LndConnectorAsReceiver
where
    L: Send + 'static,
    A: Send + 'static,
    I: Send + 'static,
{
    async fn cancelled(&self, params: Params<L, A, I>) -> Result<data::Cancelled, Error> {
        while self
            .find_invoice(params.secret_hash, InvoiceState::Cancelled)
            .await?
            .is_none()
        {
            tokio::time::delay_for(Duration::from_millis(self.retry_interval_ms)).await;
        }
        Ok(data::Cancelled)
    }
}

fn client(certificate: &Certificate, macaroon: &str) -> Result<reqwest::Client, Error> {
    let cert = certificate.clone();
    let mut default_headers = HeaderMap::with_capacity(1);
    default_headers.insert("Grpc-Metadata-macaroon", HeaderValue::from_str(macaroon)?);

    Ok(reqwest::Client::builder()
        .add_root_certificate(cert)
        .default_headers(default_headers)
        .build()?)
}
