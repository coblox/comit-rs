extern crate bitcoin_htlc;
extern crate bitcoin_rpc_client;
extern crate bitcoin_support;
extern crate comit_node;
extern crate common_types;
extern crate ethereum_support;
extern crate ethereum_wallet;
extern crate event_store;
extern crate hex;
extern crate pretty_env_logger;
extern crate rocket;
extern crate rocket_contrib;
extern crate secp256k1_support;
extern crate serde;
extern crate tc_parity_parity;
#[macro_use]
extern crate log;
extern crate reqwest;
extern crate serde_json;
extern crate spectral;
extern crate tc_web3_client;
extern crate testcontainers;
extern crate uuid;
#[macro_use]
extern crate lazy_static;
extern crate web3;

mod htlc_harness;
mod parity_client;

use ethereum_support::{Bytes, EthereumQuantity, U256};
use htlc_harness::*;
use spectral::prelude::*;
use std::time::Duration;

const SECRET: &[u8; 32] = b"hello world, you are beautiful!!";
const HTLC_TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn given_deployed_erc20_htlc_when_redeemed_with_secret_then_tokens_are_transferred() {
    let (alice, bob, htlc, token_contract, client, _handle, _container) =
        harness(TestHarnessParams {
            alice_initial_ether: EthereumQuantity::from_eth(1.0),
            htlc_type: HtlcType::Erc20 {
                alice_initial_tokens: U256::from(1000),
                htlc_token_value: U256::from(400),
            },
            htlc_timeout: HTLC_TIMEOUT,
            htlc_secret: SECRET.clone(),
        });
    let token = token_contract.unwrap();

    let htlc = assert_that(&htlc).is_ok().subject.clone();

    assert_eq!(client.token_balance_of(token, bob), U256::from(0));
    assert_eq!(client.token_balance_of(token, alice), U256::from(600));
    assert_eq!(client.token_balance_of(token, htlc), U256::from(400));

    // Send correct secret to contract
    let _ = client.send_data(htlc, Some(Bytes(SECRET.to_vec())));

    assert_eq!(client.token_balance_of(token, bob), U256::from(400));
    assert_eq!(client.token_balance_of(token, alice), U256::from(600));
    assert_eq!(client.token_balance_of(token, htlc), U256::from(0));
}

#[test]
fn given_deployed_erc20_htlc_when_refunded_after_timeout_then_tokens_are_refunded() {
    let (alice, bob, htlc, token_contract, client, _handle, _container) =
        harness(TestHarnessParams {
            alice_initial_ether: EthereumQuantity::from_eth(1.0),
            htlc_type: HtlcType::Erc20 {
                alice_initial_tokens: U256::from(1000),
                htlc_token_value: U256::from(400),
            },
            htlc_timeout: HTLC_TIMEOUT,
            htlc_secret: SECRET.clone(),
        });
    let token = token_contract.unwrap();

    let htlc = assert_that(&htlc).is_ok().subject.clone();

    assert_eq!(client.token_balance_of(token, bob), U256::from(0));
    assert_eq!(client.token_balance_of(token, alice), U256::from(600));
    assert_eq!(client.token_balance_of(token, htlc), U256::from(400));

    // Wait for the contract to expire
    ::std::thread::sleep(HTLC_TIMEOUT);
    ::std::thread::sleep(HTLC_TIMEOUT);
    let _ = client.send_data(htlc, None);

    assert_eq!(client.token_balance_of(token, bob), U256::from(0));
    assert_eq!(client.token_balance_of(token, alice), U256::from(1000));
    assert_eq!(client.token_balance_of(token, htlc), U256::from(0));
}

#[test]
fn given_deployed_erc20_htlc_when_timeout_not_yet_reached_and_wrong_secret_then_nothing_happens() {
    let (alice, bob, htlc, token_contract, client, _handle, _container) =
        harness(TestHarnessParams {
            alice_initial_ether: EthereumQuantity::from_eth(1.0),
            htlc_type: HtlcType::Erc20 {
                alice_initial_tokens: U256::from(1000),
                htlc_token_value: U256::from(400),
            },
            htlc_timeout: HTLC_TIMEOUT,
            htlc_secret: SECRET.clone(),
        });
    let token = token_contract.unwrap();

    let htlc = assert_that(&htlc).is_ok().subject.clone();

    assert_eq!(client.token_balance_of(token, bob), U256::from(0));
    assert_eq!(client.token_balance_of(token, alice), U256::from(600));
    assert_eq!(client.token_balance_of(token, htlc), U256::from(400));

    // Don't wait for the timeout and don't send a secret
    let _ = client.send_data(htlc, None);

    assert_eq!(client.token_balance_of(token, bob), U256::from(0));
    assert_eq!(client.token_balance_of(token, alice), U256::from(600));
    assert_eq!(client.token_balance_of(token, htlc), U256::from(400));
}

#[test]
fn given_no_enough_tokens_token_balances_dont_change_and_contract_is_not_deployed() {
    let (alice, bob, htlc, token_contract, client, _handle, _container) =
        harness(TestHarnessParams {
            alice_initial_ether: EthereumQuantity::from_eth(1.0),
            htlc_type: HtlcType::Erc20 {
                alice_initial_tokens: U256::from(200),
                htlc_token_value: U256::from(400),
            },
            htlc_timeout: HTLC_TIMEOUT,
            htlc_secret: SECRET.clone(),
        });
    let token = token_contract.unwrap();

    let htlc = assert_that(&htlc).is_ok().subject.clone();

    assert_eq!(client.token_balance_of(token, bob), U256::from(0));
    assert_eq!(client.token_balance_of(token, alice), U256::from(200));
    assert_eq!(client.token_balance_of(token, htlc), U256::from(0));
    assert_eq!(client.get_contract_code(htlc), Bytes::default());
}
