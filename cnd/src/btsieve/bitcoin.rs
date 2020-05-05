mod bitcoind_connector;
mod cache;

pub use self::{
    bitcoind_connector::{BitcoindConnector, ChainInfo},
    cache::Cache,
};
use crate::{
    btsieve::{
        find_relevant_blocks, BlockByHash, BlockHash, LatestBlock, Predates, PreviousBlockHash,
    },
    identity,
};
use bitcoin::{
    consensus::{encode::deserialize, Decodable},
    BitcoinHash, OutPoint,
};
use chrono::NaiveDateTime;
use genawaiter::{sync::Gen, GeneratorState};
use reqwest::{Client, Url};

type Hash = bitcoin::BlockHash;
type Block = bitcoin::Block;

impl BlockHash for Block {
    type BlockHash = Hash;

    fn block_hash(&self) -> Hash {
        self.bitcoin_hash()
    }
}

impl PreviousBlockHash for Block {
    type BlockHash = Hash;

    fn previous_block_hash(&self) -> Hash {
        self.header.prev_blockhash
    }
}

pub async fn watch_for_spent_outpoint<C>(
    blockchain_connector: &C,
    start_of_swap: NaiveDateTime,
    from_outpoint: OutPoint,
    identity: identity::Bitcoin,
) -> anyhow::Result<(bitcoin::Transaction, bitcoin::TxIn)>
where
    C: LatestBlock<Block = Block> + BlockByHash<Block = Block, BlockHash = Hash>,
{
    let (transaction, txin) = watch(blockchain_connector, start_of_swap, |transaction| {
        transaction
            .input
            .iter()
            .filter(|txin| txin.previous_output == from_outpoint)
            .find(|txin| txin.witness.contains(&identity.to_bytes()))
            .cloned()
    })
    .await?;

    Ok((transaction, txin))
}

pub async fn watch_for_created_outpoint<C>(
    blockchain_connector: &C,
    start_of_swap: NaiveDateTime,
    compute_address: bitcoin::Address,
) -> anyhow::Result<(bitcoin::Transaction, bitcoin::OutPoint)>
where
    C: LatestBlock<Block = Block> + BlockByHash<Block = Block, BlockHash = Hash>,
{
    let (transaction, out_point) = watch(blockchain_connector, start_of_swap, |transaction| {
        let txid = transaction.txid();
        transaction
            .output
            .iter()
            .enumerate()
            .map(|(index, txout)| {
                // Casting a usize to u32 can lead to truncation on 64bit platforms
                // However, bitcoin limits the number of inputs to u32 anyway, so this
                // is not a problem for us.
                #[allow(clippy::cast_possible_truncation)]
                (index as u32, txout)
            })
            .find(|(_, txout)| txout.script_pubkey == compute_address.script_pubkey())
            .map(|(vout, _txout)| OutPoint { txid, vout })
    })
    .await?;

    Ok((transaction, out_point))
}

async fn watch<C, S, M>(
    connector: &C,
    start_of_swap: NaiveDateTime,
    sieve: S,
) -> anyhow::Result<(bitcoin::Transaction, M)>
where
    C: LatestBlock<Block = Block> + BlockByHash<Block = Block, BlockHash = Hash>,
    S: Fn(&bitcoin::Transaction) -> Option<M>,
{
    let mut block_generator =
        Gen::new({ |co| async { find_relevant_blocks(connector, co, start_of_swap).await } });

    loop {
        match block_generator.async_resume().await {
            GeneratorState::Yielded(block) => {
                for transaction in block.txdata.into_iter() {
                    if let Some(result) = sieve(&transaction) {
                        tracing::trace!("transaction matched {:x}", transaction.txid());
                        return Ok((transaction, result));
                    }
                }
            }
            GeneratorState::Complete(Err(e)) => return Err(e),
            // By matching against the never type explicitly, we assert that the `Ok` value of the
            // result is actually the never type and has not been changed since this line was
            // written. The never type can never be constructed, so we can never reach this line.
            GeneratorState::Complete(Ok(never)) => match never {},
        }
    }
}

impl Predates for Block {
    fn predates(&self, timestamp: NaiveDateTime) -> bool {
        let unix_timestamp = timestamp.timestamp();
        let block_time = self.header.time as i64;

        block_time < unix_timestamp
    }
}

pub async fn bitcoin_http_request_for_hex_encoded_object<T>(
    request_url: Url,
    client: &Client,
) -> anyhow::Result<T>
where
    T: Decodable,
{
    let response_text = client.get(request_url).send().await?.text().await?;
    let decoded_response = decode_response(response_text)?;

    Ok(decoded_response)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported network: {0}")]
    UnsupportedNetwork(String),
    #[error("reqwest: ")]
    Reqwest(#[from] reqwest::Error),
    #[error("hex: ")]
    Hex(#[from] hex::FromHexError),
    #[error("deserialization: ")]
    Deserialization(#[from] bitcoin::consensus::encode::Error),
}

pub fn decode_response<T>(response_text: String) -> Result<T, Error>
where
    T: Decodable,
{
    let bytes = hex::decode(response_text.trim()).map_err(Error::Hex)?;
    deserialize(bytes.as_slice()).map_err(Error::Deserialization)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction;
    use spectral::prelude::*;

    #[test]
    fn can_decode_tx_from_bitcoind_http_interface() {
        // the line break here is on purpose, as it is returned like that from bitcoind
        let transaction = r#"02000000014135047eff77c95bce4955f630bc3e334690d31517176dbc23e9345493c48ecf000000004847304402200da78118d6970bca6f152a6ca81fa8c4dde856680eb6564edb329ce1808207c402203b3b4890dd203cc4c9361bbbeb7ebce70110d4b07f411208b2540b10373755ba01feffffff02644024180100000017a9142464790f3a3fddb132691fac9fd02549cdc09ff48700a3e1110000000017a914c40a2c4fd9dcad5e1694a41ca46d337eb59369d78765000000
"#.to_owned();

        let bytes = decode_response::<transaction::Bitcoin>(transaction);

        assert_that(&bytes).is_ok();
    }

    #[test]
    fn can_decode_block_from_bitcoind_http_interface() {
        // the line break here is on purpose, as it is returned like that from bitcoind
        let block = r#"00000020837603de6069115e22e7fbf063c2a6e3bc3b3206f0b7e08d6ab6c168c2e50d4a9b48676dedc93d05f677778c1d83df28fd38d377548340052823616837666fb8be1b795dffff7f200000000001020000000001010000000000000000000000000000000000000000000000000000000000000000ffffffff0401650101ffffffff0200f2052a0100000023210205980e76eee77386241a3a7a5af65e910fb7be411b98e609f7c0d97c50ab8ebeac0000000000000000266a24aa21a9ede2f61c3f71d1defd3fa999dfa36953755c690689799962b48bebd836974e8cf90120000000000000000000000000000000000000000000000000000000000000000000000000
"#.to_owned();

        let bytes = decode_response::<Block>(block);

        assert_that(&bytes).is_ok();
    }
}
