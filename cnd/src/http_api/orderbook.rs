use crate::{
    asset::{self, Dai, Erc20},
    hbit, herc20,
    http_api::problem,
    identity, ledger,
    network::NewOrder,
    storage::{CreatedSwap, Save},
    Facade, LocalSwapId, Role,
};
use chrono::Utc;
use comit::{
    ethereum,
    network::{MakerId, Order, OrderId, Position},
};
use serde::{Deserialize, Serialize};
use warp::{http, http::StatusCode, Rejection, Reply};

pub async fn post_take_order(
    order_id: OrderId,
    body: serde_json::Value,
    mut facade: Facade,
) -> Result<impl Reply, Rejection> {
    tracing::info!("entered take order controller");
    let body = TakeOrderBody::deserialize(&body)
        .map_err(anyhow::Error::new)
        .map_err(problem::from_anyhow)
        .map_err(warp::reject::custom)?;

    let reply = warp::reply::reply();

    let swap_id = LocalSwapId::default();

    let order_id = order_id;
    let order = match facade.get_order(order_id).await {
        Some(order) => order,
        None => panic!("order not found"),
    };

    // TODO: Consider putting the save in the network layer to be uniform with make?
    let start_of_swap = Utc::now().naive_local();

    match order.position {
        Position::Buy => {
            let swap = CreatedSwap {
                swap_id,
                alpha: hbit::CreatedSwap {
                    amount: order.bitcoin_amount,
                    final_identity: body.bitcoin_identity.clone(),
                    network: order.bitcoin_ledger,
                    absolute_expiry: order.bitcoin_absolute_expiry,
                },
                beta: herc20::CreatedSwap {
                    asset: Erc20 {
                        token_contract: order.token_contract,
                        quantity: order.ethereum_amount,
                    },
                    identity: body.ethereum_identity,
                    chain_id: order.ethereum_ledger.chain_id,
                    absolute_expiry: order.ethereum_absolute_expiry,
                },
                peer: order.maker.clone().into(),
                address_hint: None,
                role: Role::Alice,
                start_of_swap,
            };
            facade
                .save(swap)
                .await
                .map_err(problem::from_anyhow)
                .map_err(warp::reject::custom)?;
        }
        Position::Sell => {
            let swap = CreatedSwap {
                swap_id,
                alpha: herc20::CreatedSwap {
                    asset: Erc20 {
                        token_contract: order.token_contract,
                        quantity: order.ethereum_amount,
                    },
                    identity: body.ethereum_identity,
                    chain_id: order.ethereum_ledger.chain_id,
                    absolute_expiry: order.ethereum_absolute_expiry,
                },
                beta: hbit::CreatedSwap {
                    amount: order.bitcoin_amount,
                    final_identity: body.bitcoin_identity.clone(),
                    network: order.bitcoin_ledger,
                    absolute_expiry: order.bitcoin_absolute_expiry,
                },
                peer: order.maker.clone().into(),
                address_hint: None,
                role: Role::Alice,
                start_of_swap,
            };
            facade
                .save(swap)
                .await
                .map_err(problem::from_anyhow)
                .map_err(warp::reject::custom)?;
        }
    }

    tracing::info!("swap created and saved from order: {:?}", order_id);

    facade
        .take_order(
            order_id,
            swap_id,
            body.bitcoin_identity.into(),
            body.ethereum_identity,
        )
        .await
        .map(|_| {
            warp::reply::with_status(
                warp::reply::with_header(reply, "Location", format!("/swaps/{}", swap_id)),
                StatusCode::CREATED,
            )
        })
        // do error handling on in from_anyhow
        .map_err(problem::from_anyhow)
        .map_err(warp::reject::custom)
}

pub async fn post_make_order(
    body: serde_json::Value,
    facade: Facade,
) -> Result<impl Reply, Rejection> {
    tracing::info!("entered make order controller");
    let body = MakeOrderBody::deserialize(&body)
        .map_err(anyhow::Error::new)
        .map_err(problem::from_anyhow)
        .map_err(warp::reject::custom)?;

    let reply = warp::reply::reply();
    let order = NewOrder::from(body.clone());

    order
        .assert_valid_ledger_pair()
        .map_err(problem::from_anyhow)
        .map_err(warp::reject::custom)?;

    let swap_id = LocalSwapId::default();

    facade
        .make_order(
            order,
            swap_id,
            body.ethereum_identity,
            body.bitcoin_identity.into(),
        )
        .await
        .map(|order_id| {
            warp::reply::with_status(
                warp::reply::with_header(reply, "Location", format!("/orders/{}", order_id)),
                StatusCode::CREATED,
            )
        })
        .map_err(problem::from_anyhow)
        .map_err(warp::reject::custom)
}

pub async fn post_limit_order(
    body: serde_json::Value,
    facade: Facade,
) -> Result<impl Reply, Rejection> {
    tracing::info!("entered make order controller");
    let body = LimitOrderBody::deserialize(&body)
        .map_err(anyhow::Error::new)
        .map_err(problem::from_anyhow)
        .map_err(warp::reject::custom)?;

    let reply = warp::reply::reply();
    let order = NewOrder::from(body.clone());

    facade
        .create_limit_order(order, body.ethereum_identity, body.bitcoin_identity.into())
        .await
        .map(|order_id| {
            warp::reply::with_status(
                warp::reply::with_header(reply, "Location", format!("/orders/{}", order_id)),
                StatusCode::CREATED,
            )
        })
        .map_err(problem::from_anyhow)
        .map_err(warp::reject::custom)
}

pub async fn get_order(order_id: OrderId, facade: Facade) -> Result<impl Reply, Rejection> {
    let swap_id = facade
        .storage
        .get_swap_associated_with_order(&order_id)
        .await;

    let entity = match swap_id {
        Some(swap_id) => siren::Entity::default()
            .with_class_member("order")
            .with_link(
                siren::NavigationalLink::new(&["swap"], format!("/swaps/{}", swap_id))
                    .with_title("swap that was created from the order"),
            ),
        None => siren::Entity::default().with_class_member("order"),
    };
    Ok(warp::reply::json(&entity))
}

pub async fn get_orders(facade: Facade) -> Result<impl Reply, Rejection> {
    let orders = facade.get_orders().await;

    let mut entity = siren::Entity::default().with_class_member("orders");

    for order in orders.into_iter() {
        let bitcoin_field = siren::Field {
            name: "bitcoin_identity".to_string(),
            class: vec!["bitcoin".to_string(), "address".to_string()],
            _type: None,
            value: None,
            title: None,
        };

        let ethereum_field = siren::Field {
            name: "ethereum_identity".to_string(),
            class: vec!["ethereum".to_string(), "address".to_string()],
            _type: None,
            value: None,
            title: None,
        };

        let action = siren::Action {
            name: "take".to_string(),
            class: vec![],
            method: Some(http::Method::POST),
            href: format!("/orders/{}/take", order.id),
            title: None,
            _type: Some("application/json".to_string()),
            fields: vec![bitcoin_field, ethereum_field],
        };

        match siren::Entity::default()
            .with_action(action)
            .with_class_member("order")
            .with_properties(OrderResponse::from(order))
        {
            Ok(sub_entity) => {
                entity.push_sub_entity(siren::SubEntity::from_entity(sub_entity, &["item"]))
            }
            Err(_e) => tracing::error!("could not serialise order sub entity"),
        }
    }
    Ok(warp::reply::json(&entity))
}

/// Create a BTC/DAI limit order.
// I don't care what the expiries are, I want cnd to work out good
// defaults for me. I don't want to give over identities here, I just
// want to create the limit order.
#[derive(Clone, Debug, Deserialize)]
struct LimitOrderBody {
    position: Position,
    price: asset::Dai,
    #[serde(with = "asset::bitcoin::sats_as_string")]
    quantity: asset::Bitcoin,
}

impl LimitOrderBody {
    fn dai_vaule() -> asset::Dai {
        // let raw = quantity as sats * price as wei
        // shift it left 8 decimal places and convert back into DAI
        let value = asset::Dai::from_wei_dec_str("9000.00");
        tracing::warn!("careful, limit order is hard coded to be 1 BTC for 9000 DAI");

        value
    }
}

impl From<MakeOrderBody> for NewOrder {
    fn from(body: MakeOrderBody) -> Self {
        // TODO: These should come from cnd startup config.
        let bitcoin_ledger = ledger::Bitcoin::Regtest;
        let ethereum_ledger = ledger::Ethereum::from(1337);

        let (bitcoin_absolute_expiry, ethereum_absolute_expiry) = calculate_expiries(body.position);

        let token_contract = dai_token_contract();

        let bitcoin_amount = body.quantity; // We use indirect quotes.
        let ethereum_amount = body.dai_value();

        NewOrder {
            position: body.position,
            bitcoin_amount,
            bitcoin_ledger,
            bitcoin_absolute_expiry,
            ethereum_amount: body.ethereum_amount,
            token_contract,
            ethereum_ledger,
            ethereum_absolute_expiry,
        }
    }
}

fn dai_token_contract(_ledger: ledger::Ethereum) -> identity::Ethereum {
    let mainnet =
        identity::Ethereum::from_str("0x6b175474e89094c44da98b954eedeac495271d0f").unwrap();

    // TODO: This actually needs to return the token contract for the respective
    // network.

    mainnet
}

// Returns (bitcoin, ethereum) expiries.
fn calculate_expiries(position: Position) -> (u32, u32) {
    let seconds_in_an_hour: u32 = 60 * 60;

    let alpha = 12 * seconds_in_an_hour;
    let beta = 24 * seconds_in_an_hour;

    // Assumes maker of a limit order wants to act in the role of Bob.
    match position {
        Position::Buy => {
            // Alpha ledger must be Bitcoin, beta must be Ethereum.
            (alpha, beta)
        }
        Position::Sell => {
            // Alpha ledger must be Ethereum, beta ledger must be Bitcoin.
            (beta, alpha)
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct MakeOrderBody {
    position: Position,
    #[serde(with = "asset::bitcoin::sats_as_string")]
    bitcoin_amount: asset::Bitcoin,
    bitcoin_ledger: ledger::Bitcoin,
    bitcoin_absolute_expiry: u32,
    ethereum_amount: asset::Erc20Quantity,
    token_contract: identity::Ethereum,
    ethereum_ledger: ledger::Ethereum,
    ethereum_absolute_expiry: u32,
    bitcoin_identity: bitcoin::Address,
    ethereum_identity: identity::Ethereum,
}

impl From<MakeOrderBody> for NewOrder {
    fn from(body: MakeOrderBody) -> Self {
        NewOrder {
            position: body.position,
            bitcoin_amount: body.bitcoin_amount,
            bitcoin_ledger: body.bitcoin_ledger,
            bitcoin_absolute_expiry: body.bitcoin_absolute_expiry,
            ethereum_amount: body.ethereum_amount,
            token_contract: body.token_contract,
            ethereum_ledger: body.ethereum_ledger,
            ethereum_absolute_expiry: body.ethereum_absolute_expiry,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct TakeOrderBody {
    ethereum_identity: identity::Ethereum,
    bitcoin_identity: bitcoin::Address,
}

#[derive(Clone, Debug, Serialize)]
struct OrderResponse {
    id: OrderId,
    maker: MakerId,
    position: Position,
    #[serde(with = "asset::bitcoin::sats_as_string")]
    bitcoin_amount: asset::Bitcoin,
    bitcoin_ledger: ledger::Bitcoin,
    bitcoin_absolute_expiry: u32,
    ethereum_amount: asset::Erc20Quantity,
    token_contract: ethereum::Address,
    ethereum_ledger: ledger::Ethereum,
    ethereum_absolute_expiry: u32,
}

impl From<Order> for OrderResponse {
    fn from(order: Order) -> Self {
        OrderResponse {
            id: order.id,
            maker: order.maker,
            position: order.position,
            bitcoin_amount: order.bitcoin_amount,
            bitcoin_ledger: order.bitcoin_ledger,
            bitcoin_absolute_expiry: order.bitcoin_absolute_expiry,
            ethereum_amount: order.ethereum_amount,
            token_contract: order.token_contract,
            ethereum_ledger: order.ethereum_ledger,
            ethereum_absolute_expiry: order.ethereum_absolute_expiry,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_order_deserialization() {
        let json = r#"
        {
            "position": "sell",
            "bitcoin_amount": "300",
            "bitcoin_ledger": "regtest",
            "bitcoin_absolute_expiry": 600,
            "ethereum_amount": "200",
            "token_contract": "0xB97048628DB6B661D4C2aA833e95Dbe1A905B280",
            "ethereum_ledger": {"chain_id":2},
            "ethereum_absolute_expiry": 600,
            "bitcoin_identity": "1F1tAaz5x1HUXrCNLbtMDqcw6o5GNn4xqX",
            "ethereum_identity": "0x00a329c0648769a73afac7f9381e08fb43dbea72"
        }"#;

        let _body: MakeOrderBody = serde_json::from_str(json).expect("failed to deserialize order");
    }
}
