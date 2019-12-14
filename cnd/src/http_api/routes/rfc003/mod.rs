pub mod accept;
pub mod decline;
pub mod handlers;
mod swap_state;

use crate::{
    db::{DetermineTypes, Retrieve, Save, Swap},
    ethereum::{Erc20Token, EtherQuantity},
    http_api::{
        action::ActionExecutionParameters,
        route_factory::swap_path,
        routes::{
            into_rejection,
            rfc003::handlers::{handle_action, handle_get_swap, handle_post_swap},
        },
    },
    network::Network,
    seed::SwapSeed,
    swap_protocols::{
        ledger::{Bitcoin, Ethereum},
        rfc003::{actions::ActionKind, events::HtlcEvents, state_store::StateStore},
        SwapId,
    },
};
use bitcoin::Amount;
use futures::Future;
use futures_core::future::{FutureExt, TryFutureExt};
use warp::{
    http::{self, header},
    Rejection, Reply,
};

pub use self::swap_state::{LedgerState, SwapCommunication, SwapCommunicationState, SwapState};
use crate::{db::Saver, http_api::problem};
use tokio::executor::Executor;

#[allow(clippy::needless_pass_by_value)]
pub fn post_swap<
    D: Clone
        + Network
        + StateStore
        + Executor
        + Save<Swap>
        + SwapSeed
        + Saver
        + HtlcEvents<Bitcoin, Amount>
        + HtlcEvents<Ethereum, EtherQuantity>
        + HtlcEvents<Ethereum, Erc20Token>,
>(
    dependencies: D,
    body: serde_json::Value,
) -> impl Future<Item = impl Reply, Error = Rejection> {
    handle_post_swap(dependencies, body)
        .boxed()
        .compat()
        .map(|swap_created| {
            let body = warp::reply::json(&swap_created);
            let response =
                warp::reply::with_header(body, header::LOCATION, swap_path(swap_created.id));
            warp::reply::with_status(response, warp::http::StatusCode::CREATED)
        })
        .map_err(problem::from_anyhow)
        .map_err(into_rejection)
}

#[allow(clippy::needless_pass_by_value)]
pub fn get_swap<D: DetermineTypes + Retrieve + StateStore>(
    dependencies: D,
    id: SwapId,
) -> impl Future<Item = impl Reply, Error = Rejection> {
    handle_get_swap(dependencies, id)
        .boxed()
        .compat()
        .map(|swap_resource| warp::reply::json(&swap_resource))
        .map_err(problem::from_anyhow)
        .map_err(into_rejection)
}

#[allow(clippy::needless_pass_by_value)]
pub fn action<
    D: DetermineTypes
        + Retrieve
        + StateStore
        + Executor
        + Clone
        + Network
        + SwapSeed
        + Saver
        + HtlcEvents<Bitcoin, Amount>
        + HtlcEvents<Ethereum, EtherQuantity>
        + HtlcEvents<Ethereum, Erc20Token>,
>(
    method: http::Method,
    id: SwapId,
    action_kind: ActionKind,
    query_params: ActionExecutionParameters,
    dependencies: D,
    body: serde_json::Value,
) -> impl Future<Item = impl Reply, Error = Rejection> {
    handle_action(method, id, action_kind, body, query_params, dependencies)
        .boxed()
        .compat()
        .map(|body| warp::reply::json(&body))
        .map_err(problem::from_anyhow)
        .map_err(into_rejection)
}
