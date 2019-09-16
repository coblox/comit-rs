use crate::rfc003::ledger::Ledger;
use serde::Serialize;
use strum_macros::EnumDiscriminants;

#[derive(Clone, Debug, PartialEq, EnumDiscriminants)]
#[strum_discriminants(
    name(HtlcState),
    derive(Serialize),
    serde(rename_all = "SCREAMING_SNAKE_CASE")
)]
pub enum LedgerState<L: Ledger> {
    NotDeployed,
    Deployed {
        htlc_location: L::HtlcLocation,
        deploy_transaction: L::Transaction,
    },
    Funded {
        htlc_location: L::HtlcLocation,
        deploy_transaction: L::Transaction,
        fund_transaction: L::Transaction,
    },
    Redeemed {
        htlc_location: L::HtlcLocation,
        deploy_transaction: L::Transaction,
        fund_transaction: L::Transaction,
        redeem_transaction: L::Transaction,
    },
    Refunded {
        htlc_location: L::HtlcLocation,
        deploy_transaction: L::Transaction,
        fund_transaction: L::Transaction,
        refund_transaction: L::Transaction,
    },
    IncorrectlyFunded {
        htlc_location: L::HtlcLocation,
        deploy_transaction: L::Transaction,
        fund_transaction: L::Transaction,
    },
}

impl Default for HtlcState {
    fn default() -> Self {
        HtlcState::NotDeployed
    }
}

#[cfg(test)]
impl quickcheck::Arbitrary for HtlcState {
    fn arbitrary<G: quickcheck::Gen>(g: &mut G) -> Self {
        match g.next_u32() % 6 {
            0 => HtlcState::NotDeployed,
            1 => HtlcState::Deployed,
            2 => HtlcState::Funded,
            3 => HtlcState::Redeemed,
            4 => HtlcState::Refunded,
            5 => HtlcState::IncorrectlyFunded,
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn not_deployed_serializes_correctly_to_json() {
        let state = HtlcState::NotDeployed;
        let serialized = serde_json::to_string(&state).unwrap();
        assert_eq!(serialized, r#""NOT_DEPLOYED""#);
    }
}
