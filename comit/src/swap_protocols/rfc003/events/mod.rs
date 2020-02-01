// This is fine because we're using associated types
// see: https://github.com/rust-lang/rust/issues/21903
#![allow(type_alias_bounds)]

use crate::{
    asset::Asset,
    swap_protocols::rfc003::{create_swap::HtlcParams, ledger::Ledger, Secret},
};
use chrono::NaiveDateTime;
use futures_core::future::Either;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Funded<L: Ledger, A: Asset> {
    pub transaction: L::Transaction,
    pub asset: A,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Redeemed<L: Ledger> {
    pub transaction: L::Transaction,
    pub secret: Secret,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Deployed<L: Ledger> {
    pub transaction: L::Transaction,
    pub location: L::HtlcLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Refunded<L: Ledger> {
    pub transaction: L::Transaction,
}

#[async_trait::async_trait]
pub trait HtlcEvents<L: Ledger, A: Asset>: Send + Sync + Sized + 'static {
    async fn htlc_deployed(
        &self,
        htlc_params: HtlcParams<L, A>,
        start_of_swap: NaiveDateTime,
    ) -> anyhow::Result<Deployed<L>>;
    async fn htlc_funded(
        &self,
        htlc_params: HtlcParams<L, A>,
        htlc_deployment: &Deployed<L>,
        start_of_swap: NaiveDateTime,
    ) -> anyhow::Result<Funded<L, A>>;
    async fn htlc_redeemed_or_refunded(
        &self,
        htlc_params: HtlcParams<L, A>,
        htlc_deployment: &Deployed<L>,
        htlc_funding: &Funded<L, A>,
        start_of_swap: NaiveDateTime,
    ) -> anyhow::Result<Either<Redeemed<L>, Refunded<L>>>;
}
