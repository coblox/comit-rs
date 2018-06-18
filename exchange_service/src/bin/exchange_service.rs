#![feature(plugin, decl_macro)]
#![plugin(rocket_codegen)]
extern crate bitcoin;
extern crate bitcoin_rpc;
extern crate env_logger;
extern crate ethereum_wallet;
extern crate exchange_service;
extern crate hex;
extern crate log;
extern crate reqwest;
extern crate rocket;
extern crate rocket_contrib;
extern crate secp256k1;
extern crate serde;
extern crate serde_derive;
extern crate serde_json;
extern crate tiny_keccak;
extern crate uuid;
extern crate web3;

use ethereum_wallet::InMemoryWallet;
use exchange_service::ethereum_service::EthereumService;
use exchange_service::event_store::EventStore;
use exchange_service::gas_price_service::StaticGasPriceService;
use exchange_service::rocket_factory::create_rocket_instance;
use exchange_service::treasury_api_client::{DefaultApiClient, TreasuryApiUrl};
use hex::FromHex;
use std::env::var;
use std::str::FromStr;
use std::sync::Arc;
use web3::futures::Future;
use web3::types::Address;

// TODO: Make a nice command line interface here (using StructOpt f.e.)
fn main() {
    let _ = env_logger::init();
    let treasury_api_url =
        TreasuryApiUrl(var("TREASURY_SERVICE_URL").expect("Treasury api url not set"));

    let api_client = DefaultApiClient {
        client: reqwest::Client::new(),
        url: treasury_api_url,
    };

    let event_store = EventStore::new();

    let private_key = var("ETHEREUM_PRIVATE_KEY").expect("Private key not set");
    let network_id = var("ETHEREUM_NETWORK_ID").expect("Ethereum network id not set");

    let private_key = <[u8; 32]>::from_hex(private_key).expect("Private key is not hex_encoded");
    let network_id = u8::from_str(network_id.as_ref()).expect("Failed to parse network id");

    let wallet =
        InMemoryWallet::new(private_key, network_id).expect("Failed to create wallet instance");

    let endpoint = var("ETHEREUM_NODE_ENDPOINT").expect("Ethereum node endpoint is not set");

    let (_event_loop, transport) = web3::transports::Http::new(&endpoint).unwrap();

    let web3 = web3::api::Web3::new(transport);

    // TODO: issue opened. The derive is incorrect
    // let address = derive_address_from_private_key(&private_key);

    let address = var("ETHEREUM_EXCHANGE_ADDRESS").expect("ETHEREUM_EXCHANGE_ADDRESS is not set");
    let address = web3::types::Address::from_str(&address.as_str()).unwrap();
    let nonce = web3.eth().transaction_count(address, None).wait().unwrap();
    println!("Nonce: {}", nonce);

    let ethereum_service = EthereumService::new(
        Arc::new(wallet),
        Arc::new(StaticGasPriceService::default()),
        Arc::new(web3),
        nonce,
    );

    let bitcoin_rpc_client = {
        let url = var("BITCOIN_RPC_URL").expect("BITCOIN_RPC_URL not set");
        let username = var("BITCOIN_RPC_USERNAME").expect("BITCOIN_RPC_USERNAME not set");
        let password = var("BITCOIN_RPC_PASSWORD").expect("BITCOIN_RPC_PASSWORD not set");

        bitcoin_rpc::BitcoinCoreClient::new(url.as_str(), username.as_str(), password.as_str())
    };

    let network = match var("BTC_NETWORK") {
        Ok(value) => match value.as_str() {
            "BTC_MAINNET" => panic!("You are not mainnet ready fool!"),
            "BTC_TESTNET" => bitcoin::network::constants::Network::Testnet,
            "BTCORE_REGTEST" => bitcoin::network::constants::Network::BitcoinCoreRegtest,
            _ => panic!(
                "Please set environment variable BTC_NETWORK to one of the following values:\n\
                 - BTC_MAINNET\n- BTC_TESTNET\n- BTCORE_REGTEST"
            ),
        },
        Err(_) => bitcoin::network::constants::Network::BitcoinCoreRegtest,
    };

    create_rocket_instance(
        Arc::new(api_client),
        event_store,
        Arc::new(ethereum_service),
        Arc::new(bitcoin_rpc_client),
        network,
    ).launch();
}

// TODO move this somewhere else (maybe contribute to web3?)
fn derive_address_from_private_key(private_key: &[u8]) -> web3::types::Address {
    let secp256k1 = secp256k1::Secp256k1::new();
    let secret_key = secp256k1::SecretKey::from_slice(&secp256k1, private_key).unwrap();
    let public_key = secp256k1::PublicKey::from_secret_key(&secp256k1, &secret_key).unwrap();

    let serialized = public_key.serialize();

    let hash = tiny_keccak::keccak256(&serialized);

    let mut result = Address::default();
    result.copy_from_slice(&hash[12..]);
    result
}
