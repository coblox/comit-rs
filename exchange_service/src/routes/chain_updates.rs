use bitcoin_htlc;
use bitcoin_htlc::Network;
use bitcoin_rpc;
use bitcoin_wallet;
use common_types::BitcoinQuantity;
use common_types::secret::Secret;
use event_store::EventStore;
use event_store::TradeId;
use rocket::State;
use rocket::response::status::BadRequest;
use rocket_contrib::Json;
use std::fmt::Debug;
use std::sync::Arc;

//TODO: move back to eth_btc.rs

#[derive(Deserialize)]
pub struct RedeemBTCNotificationBody {
    pub secret: Secret,
}

fn log_error<E: Debug>(msg: &'static str) -> impl Fn(E) -> BadRequest<String> {
    move |e: E| {
        error!("{}: {:?}", msg, e);
        BadRequest(None)
    }
}
#[post("/trades/ETH-BTC/<trade_id>/buy-order-secret-revealed", format = "application/json",
       data = "<redeem_btc_notification_body>")]
pub fn post_revealed_secret(
    redeem_btc_notification_body: Json<RedeemBTCNotificationBody>,
    event_store: State<EventStore>,
    rpc_client: State<Arc<bitcoin_rpc::BitcoinRpcApi>>,
    network: State<Network>,
    trade_id: TradeId,
) -> Result<(), BadRequest<String>> {
    let order_taken_event = event_store.get_order_taken_event(&trade_id)?;

    let secret: Secret = redeem_btc_notification_body.into_inner().secret;

    let orig_secret_hash = order_taken_event.contract_secret_lock();
    let given_secret_hash = secret.hash();
    if given_secret_hash != *orig_secret_hash {
        error!("Secret for trade {} can't be used to redeem htlc locked by {} because it didn't match {}", trade_id, orig_secret_hash, given_secret_hash);
        return Err(BadRequest(Some(
            "the secret didn't match the hash".to_string(),
        )));
    }

    // TODO: Maybe if this fails we keep the secret around anyway and steal money early?
    let trade_funded_event = event_store.get_trade_funded_event(&trade_id)?;

    let htlc_txid = trade_funded_event.transaction_id();
    let vout = trade_funded_event.vout();
    let offer_created_event = event_store.get_offer_created_event(&trade_id)?;
    let input_amount = offer_created_event.btc_amount();
    let fee = BitcoinQuantity::from_satoshi(1000);
    let output_amount = input_amount - fee;

    let exchange_success_address = order_taken_event
        .exchange_success_address()
        .to_bitcoin_address()
        .map_err(log_error("Failed to convert exchange success address"))?;

    debug!("Exchange success address retrieved");

    let client_refund_address = order_taken_event
        .client_refund_address()
        .to_bitcoin_address()
        .map_err(log_error("Failed to convert client refund address"))?;

    debug!("Client refund address retrieved");

    let htlc_script = bitcoin_htlc::Htlc::new(
        exchange_success_address.clone(),
        client_refund_address,
        order_taken_event.contract_secret_lock().clone(),
        order_taken_event.client_contract_time_lock().clone().into(),
        &network,
    ).map_err(log_error("Failed to generate bitcoin HTLC"))?
        .script()
        .clone();

    debug!("HTLC successfully generated");

    let redeem_tx = bitcoin_wallet::generate_p2wsh_htlc_redeem_tx(
        htlc_txid,
        vout,
        input_amount,
        output_amount,
        &htlc_script,
        &secret,
        &order_taken_event.exchange_success_private_key(),
        &exchange_success_address,
    ).map_err(log_error(
        "Unable to generate p2wsh htlc redeem transaction",
    ))?;

    debug!("Redeem transaction successfully generated");

    //TODO: Store above in event prior to doing rpc request

    let rpc_transaction =
        bitcoin_rpc::SerializedRawTransaction::from_bitcoin_transaction(redeem_tx).map_err(
            log_error("Failed to convert the transaction into a serialised raw transaction"),
        )?;

    info!(
        "Attempting to redeem HTLC with txid {} for {}",
        htlc_txid, trade_id
    );
    //TODO: Store successful redeem in event
    let redeem_txid = rpc_client
        .send_raw_transaction(rpc_transaction)
        .map_err(log_error("Failed to send connect to bitcoin RPC"))?
        .into_result()
        .map_err(log_error("Failed to send raw transaction to bitcoin RPC"))?;

    info!(
        "HTLC for {} successfully redeemed with {}",
        trade_id, redeem_txid
    );

    Ok(())
}
