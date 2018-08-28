use bitcoin_htlc;
use bitcoin_rpc_client::TransactionId;
use bitcoin_service;
use bitcoin_support::PubkeyHash;
use common_types::{
    ledger::{bitcoin::Bitcoin, ethereum::Ethereum, Ledger},
    secret::Secret,
};
use ethereum_service;
use ethereum_support;
use event_store::{EventStore, InMemoryEventStore};
use rocket::{response::status::BadRequest, State};
use rocket_contrib::Json;
use std::{str::FromStr, sync::Arc};
use swaps::{
    common::{Error, TradeId},
    events::{ContractDeployed, ContractRedeemed, OfferCreated, OrderTaken, TradeFunded},
};

impl From<bitcoin_service::Error> for Error {
    fn from(e: bitcoin_service::Error) -> Self {
        Error::BitcoinService(e)
    }
}

#[derive(Deserialize, Debug)]
pub struct SellOrderHtlcDeployedNotification {
    contract_address: ethereum_support::Address,
}

#[post(
    "/trades/ETH-BTC/<trade_id>/sell-order-htlc-funded",
    format = "application/json",
    data = "<htlc_identifier>"
)]
pub fn post_orders_funding(
    trade_id: TradeId,
    htlc_identifier: Json<<Ethereum as Ledger>::HtlcId>,
    event_store: State<InMemoryEventStore<TradeId>>,
    bitcoin_service: State<Arc<bitcoin_service::BitcoinService>>,
) -> Result<(), BadRequest<String>> {
    handle_post_orders_funding(
        trade_id,
        htlc_identifier.into_inner(),
        event_store.inner(),
        bitcoin_service.inner(),
    )?;
    Ok(())
}

fn handle_post_orders_funding(
    trade_id: TradeId,
    htlc_identifier: <Ethereum as Ledger>::HtlcId,
    event_store: &InMemoryEventStore<TradeId>,
    bitcoin_service: &Arc<bitcoin_service::BitcoinService>,
) -> Result<(), Error> {
    //get OrderTaken event to verify correct state
    let order_taken = event_store.get_event::<OrderTaken<Bitcoin, Ethereum>>(trade_id.clone())?;

    //create new event
    let trade_funded: TradeFunded<Bitcoin, Ethereum> = TradeFunded::new(trade_id, htlc_identifier);
    event_store.add_event(trade_id.clone(), trade_funded)?;
    let exchange_refund_address = order_taken.exchange_refund_address;
    let exchange_refund_address_pub_key: PubkeyHash = exchange_refund_address.into();
    let client_success_address = order_taken.client_success_address;
    let client_success_address_hash: PubkeyHash = client_success_address.into();

    let htlc = bitcoin_htlc::Htlc::new(
        client_success_address_hash,
        exchange_refund_address_pub_key,
        order_taken.contract_secret_lock.clone(),
        order_taken.exchange_contract_time_lock.into(),
    );

    let offer_created_event =
        event_store.get_event::<OfferCreated<Bitcoin, Ethereum>>(trade_id.clone())?;

    let tx_id = TransactionId::from_str(
        "d54994ece1d11b19785c7248868696250ab195605b469632b7bd68130e880c9a",
    ).unwrap();

    let deployed: ContractDeployed<Bitcoin, Ethereum> =
        ContractDeployed::new(trade_id, tx_id.to_string());

    event_store.add_event(trade_id, deployed)?;

    Ok(())
}

#[derive(Deserialize)]
pub struct RedeemETHNotificationBody {
    pub secret: Secret,
}

#[post(
    "/trades/ETH-BTC/<trade_id>/sell-order-secret-revealed",
    format = "application/json",
    data = "<redeem_eth_notification_body>"
)]
pub fn post_revealed_secret(
    redeem_eth_notification_body: Json<RedeemETHNotificationBody>,
    event_store: State<InMemoryEventStore<TradeId>>,
    trade_id: TradeId,
    ethereum_service: State<Arc<ethereum_service::EthereumService>>,
) -> Result<(), BadRequest<String>> {
    handle_post_revealed_secret(
        redeem_eth_notification_body.into_inner(),
        event_store.inner(),
        trade_id,
        ethereum_service.inner(),
    )?;

    Ok(())
}

fn handle_post_revealed_secret(
    redeem_eth_notification_body: RedeemETHNotificationBody,
    event_store: &InMemoryEventStore<TradeId>,
    trade_id: TradeId,
    ethereum_service: &Arc<ethereum_service::EthereumService>,
) -> Result<(), Error> {
    let trade_funded = event_store.get_event::<TradeFunded<Bitcoin, Ethereum>>(trade_id.clone())?;

    let tx_id = ethereum_service.redeem_htlc(
        redeem_eth_notification_body.secret,
        trade_funded.htlc_identifier,
    )?;
    let deployed: ContractRedeemed<Bitcoin, Ethereum> =
        ContractRedeemed::new(trade_id, tx_id.to_string());

    event_store.add_event(trade_id, deployed)?;
    Ok(())
}
