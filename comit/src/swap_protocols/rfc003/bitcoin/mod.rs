mod extract_secret;
mod htlc_events;

use crate::swap_protocols::{
    ledger::bitcoin,
    rfc003::{create_swap::HtlcParams, Ledger},
};
use ::bitcoin::{
    hashes::{hash160, Hash},
    Address, OutPoint, Transaction,
};
use blockchain_contracts::bitcoin::rfc003::bitcoin_htlc::BitcoinHtlc;

pub use self::htlc_events::*;
use crate::{asset, bitcoin::PublicKey};

impl<B: bitcoin::Bitcoin + 'static> Ledger for B {
    type HtlcLocation = OutPoint;
    type Identity = PublicKey;
    type Transaction = Transaction;
}

impl<B: bitcoin::Bitcoin + 'static> From<HtlcParams<B, asset::Bitcoin>> for BitcoinHtlc {
    fn from(htlc_params: HtlcParams<B, asset::Bitcoin>) -> Self {
        let refund_public_key = ::bitcoin::PublicKey::from(htlc_params.refund_identity);
        let redeem_public_key = ::bitcoin::PublicKey::from(htlc_params.redeem_identity);

        let refund_identity = hash160::Hash::hash(&refund_public_key.key.serialize());
        let redeem_identity = hash160::Hash::hash(&redeem_public_key.key.serialize());

        BitcoinHtlc::new(
            htlc_params.expiry.into(),
            refund_identity,
            redeem_identity,
            htlc_params.secret_hash.into_raw(),
        )
    }
}

impl<B: bitcoin::Bitcoin + 'static> HtlcParams<B, asset::Bitcoin> {
    pub fn compute_address(&self) -> Address {
        BitcoinHtlc::from(*self).compute_address(B::network())
    }
}
