use crate::swap_protocols::{
    actions::bitcoin::{SendToAddress, SpendOutput},
    ledger::Bitcoin,
    rfc003::{
        actions::{FundAction, RedeemAction, RefundAction},
        create_swap::HtlcParams,
        secret_source::SecretSource,
        Secret,
    },
};
use bitcoin::{Amount, OutPoint, Transaction};
use blockchain_contracts::bitcoin::{rfc003::bitcoin_htlc::BitcoinHtlc, witness::PrimedInput};

impl FundAction<Bitcoin, Amount> for (Bitcoin, Amount) {
    type FundActionOutput = SendToAddress;

    fn fund_action(htlc_params: HtlcParams<Bitcoin, Amount>) -> Self::FundActionOutput {
        let to = htlc_params.compute_address();

        SendToAddress {
            to,
            amount: htlc_params.asset,
            network: htlc_params.ledger.network,
        }
    }
}

impl RefundAction<Bitcoin, Amount> for (Bitcoin, Amount) {
    type RefundActionOutput = SpendOutput;

    fn refund_action(
        htlc_params: HtlcParams<Bitcoin, Amount>,
        htlc_location: OutPoint,
        secret_source: &dyn SecretSource,
        fund_transaction: &Transaction,
    ) -> Self::RefundActionOutput {
        let htlc = BitcoinHtlc::from(htlc_params);

        SpendOutput {
            output: PrimedInput::new(
                htlc_location,
                Amount::from_sat(fund_transaction.output[htlc_location.vout as usize].value),
                htlc.unlock_after_timeout(&*crate::SECP, secret_source.secp256k1_refund()),
            ),
            network: htlc_params.ledger.network,
        }
    }
}

impl RedeemAction<Bitcoin, Amount> for (Bitcoin, Amount) {
    type RedeemActionOutput = SpendOutput;

    fn redeem_action(
        htlc_params: HtlcParams<Bitcoin, Amount>,
        htlc_location: OutPoint,
        secret_source: &dyn SecretSource,
        secret: Secret,
    ) -> Self::RedeemActionOutput {
        let htlc = BitcoinHtlc::from(htlc_params);

        SpendOutput {
            output: PrimedInput::new(
                htlc_location,
                htlc_params.asset,
                htlc.unlock_with_secret(
                    &*crate::SECP,
                    secret_source.secp256k1_redeem(),
                    secret.into_raw_secret(),
                ),
            ),
            network: htlc_params.ledger.network,
        }
    }
}
