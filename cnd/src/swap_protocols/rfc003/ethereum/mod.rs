use crate::{
    asset,
    ethereum::{Address, Bytes, Transaction},
    swap_protocols::{
        ledger::Ethereum,
        rfc003::{create_swap::HtlcParams, Ledger},
    },
};
use blockchain_contracts::ethereum::rfc003::{erc20_htlc::Erc20Htlc, ether_htlc::EtherHtlc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Deserialize, Serialize, Debug)]
pub struct ByteCode(pub String);

impl Into<Bytes> for ByteCode {
    fn into(self) -> Bytes {
        Bytes(hex::decode(self.0).unwrap())
    }
}

pub trait Htlc {
    fn compile_to_hex(&self) -> ByteCode;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Seconds(pub u64);

impl From<Duration> for Seconds {
    fn from(duration: Duration) -> Self {
        Seconds(duration.as_secs())
    }
}

impl From<Seconds> for Duration {
    fn from(seconds: Seconds) -> Duration {
        Duration::from_secs(seconds.0)
    }
}

impl Ledger for Ethereum {
    type HtlcLocation = Address;
    type Identity = Address;
    type Transaction = Transaction;
}

impl From<HtlcParams<Ethereum, asset::Ether>> for EtherHtlc {
    fn from(htlc_params: HtlcParams<Ethereum, asset::Ether>) -> Self {
        let refund_address =
            blockchain_contracts::ethereum::Address(htlc_params.refund_identity.into());
        let redeem_address =
            blockchain_contracts::ethereum::Address(htlc_params.redeem_identity.into());

        EtherHtlc::new(
            htlc_params.expiry.into(),
            refund_address,
            redeem_address,
            htlc_params.secret_hash.into(),
        )
    }
}

impl HtlcParams<Ethereum, asset::Ether> {
    pub fn bytecode(&self) -> Bytes {
        EtherHtlc::from(self.clone()).into()
    }
}

impl From<HtlcParams<Ethereum, asset::Erc20>> for Erc20Htlc {
    fn from(htlc_params: HtlcParams<Ethereum, asset::Erc20>) -> Self {
        let refund_address =
            blockchain_contracts::ethereum::Address(htlc_params.refund_identity.into());
        let redeem_address =
            blockchain_contracts::ethereum::Address(htlc_params.redeem_identity.into());
        let token_contract_address =
            blockchain_contracts::ethereum::Address(htlc_params.asset.token_contract.into());

        Erc20Htlc::new(
            htlc_params.expiry.into(),
            refund_address,
            redeem_address,
            htlc_params.secret_hash.into(),
            token_contract_address,
            htlc_params.asset.quantity.into(),
        )
    }
}

impl HtlcParams<Ethereum, asset::Erc20> {
    pub fn bytecode(self) -> Bytes {
        Erc20Htlc::from(self).into()
    }
}
