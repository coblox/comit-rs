use crate::{
    btsieve::{BlockByHash, LatestBlock, ReceiptByHash},
    ethereum::{
        web3::{
            self,
            transports::{EventLoopHandle, Http},
            Web3,
        },
        BlockId, BlockNumber,
    },
};
use futures::Future;
use reqwest::Url;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Web3Connector {
    web3: Arc<Web3<Http>>,
}

impl Web3Connector {
    pub fn new(node_url: Url) -> Result<(Self, EventLoopHandle), web3::Error> {
        let (event_loop_handle, http_transport) = Http::new(node_url.as_str())?;
        Ok((
            Self {
                web3: Arc::new(Web3::new(http_transport)),
            },
            event_loop_handle,
        ))
    }
}

impl LatestBlock for Web3Connector {
    type Error = crate::ethereum::web3::Error;
    type Block = Option<crate::ethereum::Block<crate::ethereum::Transaction>>;
    type BlockHash = crate::ethereum::H256;

    fn latest_block(
        &mut self,
    ) -> Box<dyn Future<Item = Self::Block, Error = Self::Error> + Send + 'static> {
        let web = self.web3.clone();
        Box::new(
            web.eth()
                .block_with_txs(BlockId::Number(BlockNumber::Latest)),
        )
    }
}

impl BlockByHash for Web3Connector {
    type Error = crate::ethereum::web3::Error;
    type Block = Option<crate::ethereum::Block<crate::ethereum::Transaction>>;
    type BlockHash = crate::ethereum::H256;

    fn block_by_hash(
        &self,
        block_hash: Self::BlockHash,
    ) -> Box<dyn Future<Item = Self::Block, Error = Self::Error> + Send + 'static> {
        let web = self.web3.clone();
        let block = web.eth().block_with_txs(BlockId::Hash(block_hash));
        log::trace!("Fetched block from web3: {}", block_hash);
        Box::new(block)
    }
}

impl ReceiptByHash for Web3Connector {
    type Receipt = Option<crate::ethereum::TransactionReceipt>;
    type TransactionHash = crate::ethereum::H256;
    type Error = crate::ethereum::web3::Error;

    fn receipt_by_hash(
        &self,
        transaction_hash: Self::TransactionHash,
    ) -> Box<dyn Future<Item = Self::Receipt, Error = Self::Error> + Send + 'static> {
        let web = self.web3.clone();
        Box::new(web.eth().transaction_receipt(transaction_hash))
    }
}
