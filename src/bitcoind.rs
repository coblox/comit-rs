use crate::jsonrpc;
use anyhow::Context;
use bitcoin::{
    consensus::encode::serialize_hex, hashes::hex::FromHex, Address, Amount, Network, Transaction,
    Txid,
};
use serde::Deserialize;

pub const JSONRPC_VERSION: &str = "1.0";

#[derive(Debug, Clone)]
pub struct Client {
    rpc_client: jsonrpc::Client,
}

impl Client {
    pub fn new(url: url::Url) -> Self {
        Client {
            rpc_client: jsonrpc::Client::new(url),
        }
    }

    pub async fn network(&self) -> anyhow::Result<Network> {
        let blockchain_info = self
            .rpc_client
            .send::<Vec<()>, BlockchainInfo>(jsonrpc::Request::new(
                "getblockchaininfo",
                vec![],
                JSONRPC_VERSION.into(),
            ))
            .await?;

        Ok(blockchain_info.chain)
    }

    pub async fn create_wallet(
        &self,
        wallet_name: &str,
        disable_private_keys: Option<bool>,
        blank: Option<bool>,
        passphrase: Option<String>,
        avoid_reuse: Option<bool>,
    ) -> anyhow::Result<CreateWalletResponse> {
        let response = self
            .rpc_client
            .send(jsonrpc::Request::new(
                "createwallet",
                vec![
                    jsonrpc::serialize(wallet_name)?,
                    jsonrpc::serialize(disable_private_keys)?,
                    jsonrpc::serialize(blank)?,
                    jsonrpc::serialize(passphrase)?,
                    jsonrpc::serialize(avoid_reuse)?,
                ],
                JSONRPC_VERSION.into(),
            ))
            .await
            .context("failed to create wallet")?;
        Ok(response)
    }

    pub async fn get_balance(
        &self,
        wallet_name: &str,
        minimum_confirmation: Option<u32>,
        include_watch_only: Option<bool>,
        avoid_reuse: Option<bool>,
    ) -> anyhow::Result<Amount> {
        let response = self
            .rpc_client
            .send_with_path(
                format!("/wallet/{}", wallet_name),
                jsonrpc::Request::new(
                    "getbalance",
                    vec![
                        jsonrpc::serialize('*')?,
                        jsonrpc::serialize(minimum_confirmation)?,
                        jsonrpc::serialize(include_watch_only)?,
                        jsonrpc::serialize(avoid_reuse)?,
                    ],
                    JSONRPC_VERSION.into(),
                ),
            )
            .await?;
        let amount = Amount::from_btc(response)?;
        Ok(amount)
    }

    pub async fn set_hd_seed(
        &self,
        wallet_name: &str,
        new_key_pool: Option<bool>,
        wif_private_key: Option<String>,
    ) -> anyhow::Result<()> {
        self.rpc_client
            .send_with_path(
                format!("/wallet/{}", wallet_name),
                jsonrpc::Request::new(
                    "sethdseed",
                    vec![
                        jsonrpc::serialize(new_key_pool)?,
                        jsonrpc::serialize(wif_private_key)?,
                    ],
                    JSONRPC_VERSION.into(),
                ),
            )
            .await
            .context("failed to set HD seed")?;

        Ok(())
    }

    pub async fn get_new_address(
        &self,
        wallet_name: &str,
        label: Option<String>,
        address_type: Option<String>,
    ) -> anyhow::Result<Address> {
        let address = self
            .rpc_client
            .send_with_path(
                format!("/wallet/{}", wallet_name),
                jsonrpc::Request::new(
                    "getnewaddress",
                    vec![
                        jsonrpc::serialize(label)?,
                        jsonrpc::serialize(address_type)?,
                    ],
                    JSONRPC_VERSION.into(),
                ),
            )
            .await
            .context("failed to get new address")?;
        Ok(address)
    }

    pub async fn get_wallet_info(&self, wallet_name: &str) -> anyhow::Result<WalletInfoResponse> {
        let response = self
            .rpc_client
            .send_with_path::<Vec<()>, _>(
                format!("/wallet/{}", wallet_name),
                jsonrpc::Request::new("getwalletinfo", vec![], JSONRPC_VERSION.into()),
            )
            .await?;
        Ok(response)
    }

    pub async fn send_to_address(
        &self,
        wallet_name: &str,
        address: Address,
        amount: Amount,
    ) -> anyhow::Result<Txid> {
        let txid: String = self
            .rpc_client
            .send_with_path(
                format!("/wallet/{}", wallet_name),
                jsonrpc::Request::new(
                    "sendtoaddress",
                    vec![
                        jsonrpc::serialize(address)?,
                        jsonrpc::serialize(amount.as_btc())?,
                    ],
                    JSONRPC_VERSION.into(),
                ),
            )
            .await
            .context("failed to send to address")?;
        let txid = Txid::from_hex(&txid)?;

        Ok(txid)
    }

    pub async fn send_raw_transaction(
        &self,
        wallet_name: &str,
        transaction: Transaction,
    ) -> anyhow::Result<Txid> {
        let txid: String = self
            .rpc_client
            .send_with_path(
                format!("/wallet/{}", wallet_name),
                jsonrpc::Request::new(
                    "sendrawtransaction",
                    vec![serialize_hex(&transaction)],
                    JSONRPC_VERSION.into(),
                ),
            )
            .await
            .context("failed to send raw transaction")?;
        let txid = Txid::from_hex(&txid)?;
        Ok(txid)
    }

    pub async fn get_raw_transaction(
        &self,
        wallet_name: &str,
        txid: Txid,
    ) -> anyhow::Result<Transaction> {
        let hex: String = self
            .rpc_client
            .send_with_path(
                format!("/wallet/{}", wallet_name),
                jsonrpc::Request::new(
                    "getrawtransaction",
                    vec![jsonrpc::serialize(txid)?],
                    JSONRPC_VERSION.into(),
                ),
            )
            .await
            .context("failed to get raw transaction")?;
        let bytes: Vec<u8> = FromHex::from_hex(&hex)?;
        let transaction = bitcoin::consensus::encode::deserialize(&bytes)?;

        Ok(transaction)
    }

    #[cfg(test)]
    pub async fn generate_to_address(
        &self,
        nblocks: u32,
        address: Address,
        max_tries: Option<u32>,
    ) -> anyhow::Result<Vec<BlockHash>> {
        let response = self
            .rpc_client
            .send(jsonrpc::Request::new(
                "generatetoaddress",
                vec![
                    jsonrpc::serialize(nblocks)?,
                    jsonrpc::serialize(address)?,
                    jsonrpc::serialize(max_tries)?,
                ],
                JSONRPC_VERSION.into(),
            ))
            .await
            .context("failed to generate to address")?;
        Ok(response)
    }
}

#[derive(Debug, Deserialize)]
struct BlockchainInfo {
    chain: Network,
}

#[derive(Debug, Deserialize)]
pub struct BlockHash(String);

#[derive(Debug, Deserialize)]
pub struct CreateWalletResponse {
    name: String,
    warning: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WalletInfoResponse {
    #[serde(rename = "walletname")]
    wallet_name: String,
    #[serde(rename = "walletversion")]
    wallet_version: u32,
    #[serde(rename = "txcount")]
    tx_count: u32,
    #[serde(rename = "keypoololdest")]
    keypool_oldest: u32,
    #[serde(rename = "keypoolsize_hd_internal")]
    keypool_size_hd_internal: u32,
    unlocked_until: Option<u32>,
    #[serde(rename = "paytxfee")]
    pay_tx_fee: f64,
    #[serde(rename = "hdseedid")]
    hd_seed_id: Option<String>, // Hash 160
    private_keys_enabled: bool,
    avoid_reuse: bool,
    scanning: ScanProgress,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ScanProgress {
    Bool(bool),
    Progress { duration: u32, progress: f64 },
}

#[cfg(all(test, feature = "test-docker"))]
mod test {
    use super::*;
    use crate::test_harness::bitcoin;
    use testcontainers::clients;

    #[tokio::test]
    async fn get_network_info() {
        let client = {
            let tc_client = clients::Cli::default();
            let blockchain = bitcoin::Blockchain::new(&tc_client).unwrap();

            Client::new(blockchain.node_url)
        };

        let network = client.network().await.unwrap();

        assert_eq!(network, Network::Regtest)
    }

    #[test]
    fn decode_wallet_info() {
        let json = r#"{
        "walletname":"nectar_7426b018",
        "walletversion":169900,
        "balance":0.00000000,
        "unconfirmed_balance":0.00000000,
        "immature_balance":0.00000000,
        "txcount":0,
        "keypoololdest":1592792998,
        "keypoolsize":1000,
        "keypoolsize_hd_internal":1000,
        "paytxfee":0.00000000,
        "hdseedid":"4959e065fd8e278e4ffe62254897ddac18b02674",
        "private_keys_enabled":true,
        "avoid_reuse":false,
        "scanning":false
        }"#;

        let info: WalletInfoResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(
            info,
            WalletInfoResponse {
                wallet_name: "nectar_7426b018".into(),
                wallet_version: 169_900,
                tx_count: 0,
                keypool_oldest: 1_592_792_998,
                keypool_size_hd_internal: 1000,
                unlocked_until: None,
                pay_tx_fee: 0.0,
                hd_seed_id: Some("4959e065fd8e278e4ffe62254897ddac18b02674".into()),
                private_keys_enabled: true,
                avoid_reuse: false,
                scanning: ScanProgress::Bool(false)
            }
        )
    }
}
