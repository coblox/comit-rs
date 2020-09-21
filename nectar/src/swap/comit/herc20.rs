use anyhow::Result;
use chrono::{DateTime, Utc};

pub use comit::{
    actions::ethereum::*,
    asset,
    btsieve::{ethereum::ReceiptByHash, BlockByHash, LatestBlock},
    ethereum::{Block, ChainId, Hash},
    herc20::*,
    identity, transaction, Secret, SecretHash, Timestamp,
};

#[async_trait::async_trait]
pub trait ExecuteDeploy {
    async fn execute_deploy(&self, params: Params) -> Result<Deployed>;
}

#[async_trait::async_trait]
pub trait ExecuteFund {
    async fn execute_fund(
        &self,
        params: Params,
        deploy_event: Deployed,
        utc_start_of_swap: DateTime<Utc>,
    ) -> Result<Funded>;
}

#[async_trait::async_trait]
pub trait ExecuteRedeem {
    async fn execute_redeem(
        &self,
        params: Params,
        secret: Secret,
        deploy_event: Deployed,
        utc_start_of_swap: DateTime<Utc>,
    ) -> Result<Redeemed>;
}

#[async_trait::async_trait]
pub trait ExecuteRefund {
    async fn execute_refund(
        &self,
        params: Params,
        deploy_event: Deployed,
        utc_start_of_swap: DateTime<Utc>,
    ) -> Result<Refunded>;
}

#[derive(Debug, Clone)]
pub struct Funded {
    pub transaction: transaction::Ethereum,
    pub asset: asset::Erc20,
}

pub async fn watch_for_funded<C>(
    connector: &C,
    params: Params,
    utc_start_of_swap: DateTime<Utc>,
    deployed: Deployed,
) -> Result<Funded>
where
    C: LatestBlock<Block = Block> + BlockByHash<Block = Block, BlockHash = Hash> + ReceiptByHash,
{
    match comit::herc20::watch_for_funded(connector, params, utc_start_of_swap, deployed).await? {
        comit::herc20::Funded::Correctly { transaction, asset } => {
            Ok(Funded { transaction, asset })
        }
        comit::herc20::Funded::Incorrectly { .. } => {
            anyhow::bail!("Ethereum HTLC incorrectly funded")
        }
    }
}

#[cfg(test)]
pub fn params(
    secret_hash: SecretHash,
    chain_id: crate::swap::ethereum::ChainId,
    redeem_identity: identity::Ethereum,
    refund_identity: identity::Ethereum,
    token_contract: crate::swap::ethereum::Address,
    expiry: Timestamp,
) -> Params {
    let quantity = asset::ethereum::FromWei::from_wei(1_000_000_000u64);
    let asset = asset::Erc20::new(token_contract, quantity);

    Params {
        asset,
        redeem_identity,
        refund_identity,
        expiry,
        chain_id,
        secret_hash,
    }
}
