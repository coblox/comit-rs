use bitcoin_support::BitcoinQuantity;
use ethereum_support::Erc20Quantity;
use swap_protocols::{
    ledger::{Bitcoin, Ethereum},
    rfc003::{
        actions::{
            bitcoin::BitcoinRedeem,
            ethereum::{Erc20Deploy, Erc20Fund, Erc20Refund},
            Action, StateActions,
        },
        roles::Alice,
        state_machine::*,
    },
};

impl StateActions for SwapStates<Alice<Ethereum, Bitcoin, Erc20Quantity, BitcoinQuantity>> {
    type Accept = ();
    type Decline = ();
    type Deploy = Erc20Deploy;
    type Fund = Erc20Fund;
    type Redeem = BitcoinRedeem;
    type Refund = Erc20Refund;

    fn actions(&self) -> Vec<Action<(), (), Erc20Deploy, Erc20Fund, BitcoinRedeem, Erc20Refund>> {
        use self::SwapStates as SS;
        match *self {
            SS::Start { .. } => vec![],
            SS::Accepted(Accepted { ref swap, .. }) => vec![Action::Deploy(Erc20Deploy {
                data: unimplemented!(),
                gas_cost: unimplemented!(),
                gas_limit: unimplemented!(),
            })],
            SS::AlphaDeployed { .. } => vec![], // TODO: Add Fund Action
            SS::AlphaFunded { .. } => vec![],
            SS::AlphaFundedBetaDeployed { .. } => vec![],
            SS::BothFunded(BothFunded {
                ref alpha_htlc_location,
                ref beta_htlc_location,
                ref swap,
                ..
            }) => vec![
                Action::Redeem(BitcoinRedeem {
                    contract_address: *beta_htlc_location,
                    data: swap.secret,
                    gas_limit: 42.into(), //TODO come up with correct gas limit
                    gas_cost: 42.into(),  //TODO come up with correct gas cost
                }),
                Action::Refund(Erc20Refund {
                    outpoint: *alpha_htlc_location,
                    htlc: swap.alpha_htlc_params().into(),
                    value: swap.alpha_asset,
                    transient_keypair: swap.alpha_ledger_refund_identity,
                }),
            ],
            SS::AlphaFundedBetaRefunded(AlphaFundedBetaRefunded {
                ref swap,
                ref alpha_htlc_location,
                ..
            })
            | SS::AlphaFundedBetaRedeemed(AlphaFundedBetaRedeemed {
                ref swap,
                ref alpha_htlc_location,
                ..
            }) => vec![Action::Refund(Erc20Refund {
                outpoint: *alpha_htlc_location,
                htlc: swap.alpha_htlc_params().into(),
                value: swap.alpha_asset,
                transient_keypair: swap.alpha_ledger_refund_identity,
            })],
            SS::AlphaRefundedBetaFunded(AlphaRefundedBetaFunded {
                ref beta_htlc_location,
                ref swap,
                ..
            })
            | SS::AlphaRedeemedBetaFunded(AlphaRedeemedBetaFunded {
                ref beta_htlc_location,
                ref swap,
                ..
            }) => vec![Action::Redeem(BitcoinRedeem {
                contract_address: *beta_htlc_location,
                data: swap.secret,
                gas_limit: 42.into(), //TODO come up with correct gas limit
                gas_cost: 42.into(),  //TODO come up with correct gas cost
            })],
            SS::Error(_) => vec![],
            SS::Final(_) => vec![],
        }
    }
}
