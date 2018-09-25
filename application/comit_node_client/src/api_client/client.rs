use bitcoin_rpc_client;
use bitcoin_support::BitcoinQuantity;
use common_types;
use ethereum_support::{self, EthereumQuantity};
use offer::Symbol;
use reqwest;
use std::{fmt, str::FromStr};
use uuid::{ParseError, Uuid};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TradeId(Uuid);

impl FromStr for TradeId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
        let uid = Uuid::from_str(s)?;
        Ok(TradeId(uid))
    }
}

impl fmt::Display for TradeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug)]
pub struct ComitNodeApiUrl(pub String);

#[allow(dead_code)]
#[derive(Debug)]
pub struct DefaultApiClient {
    pub url: ComitNodeApiUrl,
    pub client: reqwest::Client,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct BuyOfferRequestBody {
    amount: f64,
}

impl BuyOfferRequestBody {
    pub fn new(amount: f64) -> BuyOfferRequestBody {
        BuyOfferRequestBody { amount }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OfferResponseBody {
    pub uid: TradeId,
    pub symbol: Symbol,
    pub rate: f64,
    //TODO: trading-cli should be agnostic of the currencies
    pub buy_amount: EthereumQuantity,
    pub sell_amount: BitcoinQuantity,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct BuyOrderRequestBody {
    alice_success_address: ethereum_support::Address,
    alice_refund_address: bitcoin_rpc_client::Address,
}

impl BuyOrderRequestBody {
    pub fn new(alice_success_address: &str, alice_refund_address: &str) -> BuyOrderRequestBody {
        let alice_success_address = alice_success_address.trim_left_matches("0x");

        let alice_success_address = ethereum_support::Address::from_str(&alice_success_address)
            .expect("Could not convert the success address");
        let alice_refund_address = bitcoin_rpc_client::Address::from_str(alice_refund_address)
            .expect("Could not convert the Bitcoin refund address");

        BuyOrderRequestBody {
            alice_success_address,
            alice_refund_address,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RequestToFund {
    pub address_to_fund: bitcoin_rpc_client::Address,
    pub btc_amount: BitcoinQuantity,
    pub eth_amount: EthereumQuantity,
}

#[derive(Deserialize, Debug)]
pub struct RedeemDetails {
    pub address: ethereum_support::Address,
    pub data: common_types::secret::Secret,
    pub gas: u64,
}

#[derive(Debug)]
pub enum TradingServiceError {
    OfferAborted(reqwest::Error),
    OrderAborted(reqwest::Error),
    RedeemAborted(reqwest::Error),
}

pub trait ApiClient {
    fn request_offer(
        &self,
        symbol: &Symbol,
        offer_request: &BuyOfferRequestBody,
    ) -> Result<OfferResponseBody, TradingServiceError>;

    fn request_order(
        &self,
        symbol: &Symbol,
        uid: Uuid,
        request: &BuyOrderRequestBody,
    ) -> Result<RequestToFund, TradingServiceError>;

    fn request_redeem_details(
        &self,
        symbol: Symbol,
        uid: Uuid,
    ) -> Result<RedeemDetails, TradingServiceError>;
}

impl ApiClient for DefaultApiClient {
    fn request_offer(
        &self,
        symbol: &Symbol,
        request: &BuyOfferRequestBody,
    ) -> Result<OfferResponseBody, TradingServiceError> {
        let client = reqwest::Client::new();
        client
            .post(format!("{}/cli/trades/{}/buy-offers", self.url.0, symbol).as_str())
            .json(request)
            .send()
            .and_then(|mut res| res.json::<OfferResponseBody>())
            .map_err(TradingServiceError::OfferAborted)
    }

    fn request_order(
        &self,
        symbol: &Symbol,
        uid: Uuid,
        request: &BuyOrderRequestBody,
    ) -> Result<RequestToFund, TradingServiceError> {
        let client = reqwest::Client::new();
        client
            .post(format!("{}/cli/trades/{}/{}/buy-orders", self.url.0, symbol, uid).as_str())
            .json(request)
            .send()
            .and_then(|mut res| res.json::<RequestToFund>())
            .map_err(TradingServiceError::OrderAborted)
    }

    fn request_redeem_details(
        &self,
        symbol: Symbol,
        uid: Uuid,
    ) -> Result<RedeemDetails, TradingServiceError> {
        let client = reqwest::Client::new();
        client
            .get(format!("{}/cli/trades/{}/{}/redeem-orders", self.url.0, symbol, uid).as_str())
            .send()
            .and_then(|mut res| res.json::<RedeemDetails>())
            .map_err(TradingServiceError::RedeemAborted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_an_hex_address_with_0x_should_remove_0x() {
        let address = "0x00a329c0648769a73afac7f9381e08fb43dbea72".to_string();
        let refund_address = "tb1qj3z3ymhfawvdp4rphamc7777xargzufztd44fv".to_string();
        let order_request_body = BuyOrderRequestBody::new(&address, &refund_address);

        let eth_address =
            ethereum_support::Address::from_str("00a329c0648769a73afac7f9381e08fb43dbea72")
                .unwrap();
        assert_eq!(order_request_body.alice_success_address, eth_address)
    }

    #[test]
    fn given_an_hex_address_without_0x_should_return_same_address() {
        let address = "00a329c0648769a73afac7f9381e08fb43dbea72".to_string();
        let refund_address = "tb1qj3z3ymhfawvdp4rphamc7777xargzufztd44fv".to_string();
        let order_request_body = BuyOrderRequestBody::new(&address, &refund_address);

        let eth_address =
            ethereum_support::Address::from_str("00a329c0648769a73afac7f9381e08fb43dbea72")
                .unwrap();
        assert_eq!(order_request_body.alice_success_address, eth_address)
    }

}
