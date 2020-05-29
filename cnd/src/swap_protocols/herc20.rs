use crate::{
    swap_protocols::{state, state::Update, LocalSwapId},
    tracing_ext::InstrumentProtocol,
};
use chrono::NaiveDateTime;
use comit::{asset, htlc_location, transaction, Protocol, Role, Secret, Side};
pub use comit::{herc20::*, identity};
use futures::TryStreamExt;
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};
use tokio::sync::Mutex;

/// Creates a new instance of the herc20 protocol, annotated with tracing spans
/// and saves all events in the `States` hashmap.
///
/// This wrapper functions allows us to reuse code within `cnd` without having
/// to give knowledge about tracing or the state hashmaps to the `comit` crate.
pub async fn new<C>(
    id: LocalSwapId,
    params: Params,
    start_of_swap: NaiveDateTime,
    role: Role,
    side: Side,
    states: Arc<States>,
    connector: Arc<C>,
) where
    C: WaitForDeployed + WaitForFunded + WaitForRedeemed + WaitForRefunded,
{
    let mut events = comit::herc20::new(connector.as_ref(), params, start_of_swap)
        .instrument_protocol(id, role, side, Protocol::Herc20)
        .inspect_ok(|event| tracing::info!("yielded event {}", event))
        .inspect_err(|error| tracing::error!("swap failed with {:?}", error));

    while let Ok(Some(event)) = events.try_next().await {
        states.update(&id, event).await;
    }

    tracing::info!("swap finished");
}

#[derive(Default, Debug)]
pub struct States(Mutex<HashMap<LocalSwapId, State>>);

impl State {
    pub fn transition_to_deployed(&mut self, deployed: Deployed) {
        match std::mem::replace(self, State::None) {
            State::None => *self = State::Deployed(deployed),
            other => panic!("expected state NotDeployed, got {}", other),
        }
    }

    pub fn transition_to_funded(&mut self, funded: Funded) {
        match std::mem::replace(self, State::None) {
            State::Deployed(_) => *self = State::Funded(funded),
            other => panic!("expected state Deployed, got {}", other),
        }
    }

    pub fn transition_to_incorrectly_funded(&mut self, funded: Funded) {
        match std::mem::replace(self, State::None) {
            State::Deployed(_) => *self = State::IncorrectlyFunded(funded),
            other => panic!("expected state Deployed, got {}", other),
        }
    }

    pub fn transition_to_redeemed(&mut self, redeemed: Redeemed) {
        let Redeemed {
            transaction,
            secret,
        } = redeemed;

        match std::mem::replace(self, State::None) {
            State::Funded(Funded {
                deploy_transaction,
                location,
                asset,
                transaction: fund_transaction,
            }) => {
                *self = State::Redeemed {
                    deploy_transaction,
                    htlc_location: location,
                    fund_transaction,
                    redeem_transaction: transaction,
                    asset,
                    secret,
                }
            }
            other => panic!("expected state Funded, got {}", other),
        }
    }

    pub fn transition_to_refunded(&mut self, refunded: Refunded) {
        let Refunded { transaction } = refunded;

        match std::mem::replace(self, State::None) {
            State::Funded(Funded {
                deploy_transaction,
                location,
                asset,
                transaction: fund_transaction,
            })
            | State::IncorrectlyFunded(Funded {
                deploy_transaction,
                location,
                asset,
                transaction: fund_transaction,
            }) => {
                *self = State::Refunded {
                    deploy_transaction,
                    htlc_location: location,
                    fund_transaction,
                    refund_transaction: transaction,
                    asset,
                }
            }
            other => panic!("expected state Funded or IncorrectlyFunded, got {}", other),
        }
    }
}

#[async_trait::async_trait]
impl state::Get<State> for States {
    async fn get(&self, key: &LocalSwapId) -> anyhow::Result<Option<State>> {
        let states = self.0.lock().await;
        let state = states.get(key).cloned();

        Ok(state)
    }
}

#[async_trait::async_trait]
impl state::Update<Event> for States {
    async fn update(&self, key: &LocalSwapId, event: Event) {
        let mut states = self.0.lock().await;
        let entry = states.entry(*key);

        match (event, entry) {
            (Event::Started, Entry::Vacant(vacant)) => {
                vacant.insert(State::None);
            }
            (Event::Deployed(deployed), Entry::Occupied(mut state)) => {
                state.get_mut().transition_to_deployed(deployed)
            }
            (Event::Funded(funded), Entry::Occupied(mut state)) => {
                state.get_mut().transition_to_funded(funded)
            }
            (Event::IncorrectlyFunded(funded), Entry::Occupied(mut state)) => {
                state.get_mut().transition_to_incorrectly_funded(funded)
            }
            (Event::Redeemed(redeemed), Entry::Occupied(mut state)) => {
                state.get_mut().transition_to_redeemed(redeemed)
            }
            (Event::Refunded(refunded), Entry::Occupied(mut state)) => {
                state.get_mut().transition_to_refunded(refunded)
            }
            (Event::Started, Entry::Occupied(_)) => {
                tracing::warn!(
                    "Received Started event for {} although state is already present",
                    key
                );
            }
            (_, Entry::Vacant(_)) => {
                tracing::warn!("State not found for {}", key);
            }
        }
    }
}

/// Represents states that an ERC20 HTLC can be in.
#[derive(Debug, Clone, strum_macros::Display)]
#[allow(clippy::large_enum_variant)]
pub enum State {
    None,
    Deployed(Deployed),
    Funded(Funded),
    IncorrectlyFunded(Funded),
    Redeemed {
        htlc_location: htlc_location::Ethereum,
        deploy_transaction: transaction::Ethereum,
        fund_transaction: transaction::Ethereum,
        redeem_transaction: transaction::Ethereum,
        asset: asset::Erc20,
        secret: Secret,
    },
    Refunded {
        htlc_location: htlc_location::Ethereum,
        deploy_transaction: transaction::Ethereum,
        fund_transaction: transaction::Ethereum,
        refund_transaction: transaction::Ethereum,
        asset: asset::Erc20,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct Identities {
    pub redeem_identity: identity::Ethereum,
    pub refund_identity: identity::Ethereum,
}
