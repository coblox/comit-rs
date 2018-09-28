#![feature(plugin, decl_macro)]
#![plugin(rocket_codegen)]
extern crate bitcoin_rpc_client;
extern crate bitcoin_support;
extern crate comit_node;
extern crate comit_wallet;
extern crate common_types;
extern crate ethereum_support;
extern crate ethereum_wallet;
extern crate hex;
#[macro_use]
extern crate log;
extern crate event_store;
extern crate logging;
extern crate reqwest;
extern crate rocket;
extern crate rocket_contrib;
extern crate secp256k1_support;
extern crate serde;
extern crate serde_json;
extern crate tiny_keccak;
extern crate tokio;
extern crate uuid;
extern crate web3;

use bitcoin_rpc_client::BitcoinRpcApi;
use bitcoin_support::Address as BitcoinAddress;
use comit_node::{
    bitcoin_fee_service::StaticBitcoinFeeService,
    comit_node_api_client::DefaultApiClient as ComitNodeClient,
    comit_server::ComitServer,
    gas_price_service::StaticGasPriceService,
    rocket_factory::create_rocket_instance,
    settings::settings::ComitNodeSettings,
    swap_protocols::rfc003::ledger_htlc_service::{BitcoinService, EthereumService},
};
use comit_wallet::KeyStore;
use ethereum_support::*;
use ethereum_wallet::InMemoryWallet;
use event_store::InMemoryEventStore;
use std::{env::var, sync::Arc};
use web3::{transports::Http, Web3};

// TODO: Make a nice command line interface here (using StructOpt f.e.) see #298
fn main() {
    logging::set_up_logging();
    let settings = load_settings();

    // TODO: Maybe not print settings because of private keys?
    info!("Starting up with {:#?}", settings);

    let event_store = Arc::new(InMemoryEventStore::new());
    let rocket_event_store = event_store.clone();
    let comit_server_event_store = event_store.clone();

    let eth_keypair = settings.ethereum.private_key;

    let address = eth_keypair.public_key().to_ethereum_address();
    let wallet = InMemoryWallet::new(eth_keypair, settings.ethereum.network_id);

    let (event_loop, transport) = Http::new(&settings.ethereum.node_url).unwrap();
    let web3 = Web3::new(transport);

    let nonce = web3.eth().transaction_count(address, None).wait().unwrap();
    info!(
        "ETH address derived from priv key: {}; AddressNonce: {}",
        address, nonce
    );

    let ethereum_service = Arc::new(EthereumService::new(
        Arc::new(wallet),
        Arc::new(StaticGasPriceService::new(settings.ethereum.gas_price)),
        Arc::new((event_loop, web3)),
        nonce,
    ));

    let _eth_refund_address = settings.swap.eth_refund_address;

    let btc_network = settings.bitcoin.network;

    //TODO: Integrate all Ethereum keys in this keystore. See #185/#291
    let bob_key_store = Arc::new(
        KeyStore::new(settings.bitcoin.extended_private_key)
            .expect("Could not HD derive keys from the private key"),
    );

    //TODO: make it dynamically generated every X BTC. Could be done with #296
    let btc_bob_redeem_keypair = bob_key_store.get_new_internal_keypair();
    let btc_bob_redeem_address =
        BitcoinAddress::p2wpkh(btc_bob_redeem_keypair.public_key().into(), btc_network);

    info!("btc_bob_redeem_address: {}", btc_bob_redeem_address);

    let bitcoin_rpc_client = Arc::new(bitcoin_rpc_client::BitcoinCoreClient::new(
        settings.bitcoin.node_url.as_str(),
        settings.bitcoin.node_username.as_str(),
        settings.bitcoin.node_password.as_str(),
    ));

    match bitcoin_rpc_client.get_blockchain_info() {
        Ok(blockchain_info) => {
            info!("Blockchain info:\n{:?}", blockchain_info);
            match bitcoin_rpc_client.validate_address(&bitcoin_rpc_client::Address::from(
                btc_bob_redeem_address.clone(),
            )) {
                Ok(address_validation) => info!("Validation:\n{:?}", address_validation),
                Err(e) => error!("Could not validate BTC_BOB_REDEEM_ADDRESS: {}", e),
            };
        }
        Err(e) => error!("Could not connect to Bitcoin RPC:\n{}", e),
    };

    let satoshi_per_kb = settings.bitcoin.satoshi_per_byte;
    let bitcoin_fee_service = StaticBitcoinFeeService::new(satoshi_per_kb);
    let bitcoin_fee_service = Arc::new(bitcoin_fee_service);
    let bitcoin_service = Arc::new(BitcoinService::new(
        bitcoin_rpc_client.clone(),
        settings.bitcoin.network.clone(),
        bitcoin_fee_service.clone(),
        btc_bob_redeem_address.clone(),
    ));

    {
        let ethereum_service = ethereum_service.clone();
        let bitcoin_service = bitcoin_service.clone();
        let bob_key_store = bob_key_store.clone();
        let http_api_address = settings.http_api.address;
        let http_api_port = settings.http_api.port;
        let http_api_logging = settings.http_api.logging;
        let remote_comit_node_url = settings.comit.remote_comit_node_url;

        std::thread::spawn(move || {
            create_rocket_instance(
                rocket_event_store,
                ethereum_service,
                bitcoin_service,
                bob_key_store,
                btc_network,
                Arc::new(ComitNodeClient::new(remote_comit_node_url)),
                http_api_address.into(),
                http_api_port,
                http_api_logging,
            ).launch();
        });
    }

    let server = ComitServer::new(
        comit_server_event_store,
        bob_key_store,
        ethereum_service.clone(),
        btc_network,
    );

    tokio::run(server.listen(settings.comit.comit_listen).map_err(|e| {
        error!("ComitServer shutdown: {:?}", e);
    }));
}

fn load_settings() -> ComitNodeSettings {
    let comit_config_path = var_or_default("COMIT_NODE_CONFIG_PATH", "~/.config/comit_node".into());
    let run_mode_config = var_or_default("RUN_MODE", "development".into());
    let default_config = format!("{}/{}", comit_config_path.trim(), "default");
    let run_mode_config = format!("{}/{}", comit_config_path.trim(), run_mode_config);

    let settings = ComitNodeSettings::new(default_config, run_mode_config);
    settings.unwrap()
}

fn var_or_default(name: &str, default: String) -> String {
    match var(name) {
        Ok(value) => {
            info!("Set {}={}", name, value);
            value
        }
        Err(_) => {
            eprintln!(
                "{} is not set, falling back to default: '{}' ",
                name, default
            );
            default
        }
    }
}
