pub use comit::actions::*;

/// Common interface across all protocols supported by COMIT
///
/// This trait is intended to be implemented on an Actor's state and return
/// the actions which are currently available in a given state.
pub trait Actions {
    /// Different protocols have different kinds of requirements for
    /// actions. Hence they get to choose the type here.
    type ActionKind;

    fn actions(&self) -> Vec<Self::ActionKind>;
}

/// These are the traits that represent the steps involved in a COMIT atomic
/// swap.  Different protocols have different requirements/functionality for
/// each trait method but the abstractions are the same for all protocols.

/// Describes how to get the `init` action from the current state.
///
/// If `init` is not feasible in the current state, this should return `None`.
pub trait InitAction {
    type Output;

    fn init_action(&self) -> anyhow::Result<Self::Output>;
}

/// Describes how to get the `fund` action from the current state.
///
/// If `fund` is not feasible in the current state, this should return `None`.
pub trait FundAction {
    type Output;

    fn fund_action(&self) -> anyhow::Result<Self::Output>;
}

pub trait DeployAction {
    type Output;

    fn deploy_action(&self) -> anyhow::Result<Self::Output>;
}

/// Describes how to get the `redeem` action from the current state.
///
/// If `redeem` is not feasible in the current state, this should return `None`.
pub trait RedeemAction {
    type Output;

    fn redeem_action(&self) -> anyhow::Result<Self::Output>;
}

/// Describes how to get the `refund` action from the current state.
///
/// If `refund` is not feasible in the current state, this should return `None`.
pub trait RefundAction {
    type Output;

    fn refund_action(&self) -> anyhow::Result<Self::Output>;
}
