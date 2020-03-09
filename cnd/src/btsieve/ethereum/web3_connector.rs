use crate::{
    btsieve::{ethereum::ReceiptByHash, BlockByHash, LatestBlock},
    ethereum::{TransactionReceipt, H256},
    jsonrpc,
};
use async_trait::async_trait;

#[derive(Debug)]
pub struct Web3Connector {
    client: jsonrpc::Client,
}

impl Web3Connector {
    pub fn new(node_url: reqwest::Url) -> Self {
        Self {
            client: jsonrpc::Client::new(node_url),
        }
    }
}

#[async_trait]
impl LatestBlock for Web3Connector {
    type Block = crate::ethereum::Block;

    async fn latest_block(&self) -> anyhow::Result<Self::Block> {
        let block: Self::Block = self
            .client
            .send(jsonrpc::Request::new("eth_getBlockByNumber", vec![
                jsonrpc::serialize("latest")?,
                jsonrpc::serialize(true)?,
            ]))
            .await?;

        tracing::trace!(
            "Fetched block from web3: {:x}",
            block.hash.expect("blocks to have a hash")
        );

        Ok(block)
    }
}

#[async_trait]
impl BlockByHash for Web3Connector {
    type Block = crate::ethereum::Block;
    type BlockHash = crate::ethereum::H256;

    async fn block_by_hash(&self, block_hash: Self::BlockHash) -> anyhow::Result<Self::Block> {
        let block = self
            .client
            .send(jsonrpc::Request::new("eth_getBlockByHash", vec![
                jsonrpc::serialize(&block_hash)?,
                jsonrpc::serialize(true)?,
            ]))
            .await?;

        tracing::trace!("Fetched block from web3: {:x}", block_hash);

        Ok(block)
    }
}

#[async_trait]
impl ReceiptByHash for Web3Connector {
    async fn receipt_by_hash(&self, transaction_hash: H256) -> anyhow::Result<TransactionReceipt> {
        let receipt = self
            .client
            .send(jsonrpc::Request::new("eth_getTransactionReceipt", vec![
                jsonrpc::serialize(transaction_hash)?,
            ]))
            .await?;

        tracing::trace!("Fetched receipt from web3: {:x}", transaction_hash);

        Ok(receipt)
    }
}
