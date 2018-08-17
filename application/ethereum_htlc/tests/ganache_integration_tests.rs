extern crate env_logger;
extern crate ethereum_htlc;
extern crate ethereum_support;
extern crate ganache_rust_web3;
extern crate hex;
extern crate trufflesuite_ganachecli;
#[macro_use]
extern crate log;
extern crate common_types;
extern crate testcontainers;

mod common;
use common::GanacheClient;
use common_types::secret::Secret;
use ethereum_support::*;
use std::time::Duration;

const SECRET: &[u8; 32] = b"hello world, you are beautiful!!";
const ONE_HOUR: Duration = Duration::from_secs(60 * 60);

#[test]
fn given_deployed_htlc_when_redeemed_with_secret_then_money_is_transferred() {
    let _ = env_logger::try_init();

    let refund_address: Address = "147ba99ef89c152f8004e91999fee87bda6cbc3e".into();
    let success_address: Address = "96984c3e77f38ed01d1c3d98f4bd7c8b11d51d7e".into();

    let secret = Secret::from(SECRET.clone());

    let htlc = ethereum_htlc::Htlc::new(
        ONE_HOUR * 12,
        refund_address,
        success_address,
        secret.hash(),
    );

    let mut client = GanacheClient::new();

    client.take_snapshot();

    let contract_address = client.deploy(refund_address, htlc, 10);

    let refund_balance_before_htlc = client.get_balance(refund_address);
    let success_balance_before_htlc = client.get_balance(success_address);

    let gas_used = client.send_data(
        refund_address,
        contract_address,
        Some(Bytes(SECRET.to_vec())),
    );

    let refund_balance_after_htlc = client.get_balance(refund_address);
    let success_balance_after_htlc = client.get_balance(success_address);

    client.restore_snapshot();

    assert_eq!(
        success_balance_after_htlc.checked_sub(success_balance_before_htlc),
        Some(U256::from(10))
    );
    assert_eq!(
        refund_balance_before_htlc - gas_used,
        refund_balance_after_htlc
    );
}

#[test]
fn given_deployed_htlc_when_refunded_after_timeout_then_money_is_refunded() {
    let _ = env_logger::try_init();

    let refund_address: Address = "03744e31a6b9e6c6f604ff5d8ce1caef1c7bb58c".into();
    let success_address: Address = "25818640c330b071acf5fc836fe0b762a769523d".into();

    let secret = Secret::from(SECRET.clone());

    let htlc = ethereum_htlc::Htlc::new(ONE_HOUR, refund_address, success_address, secret.hash());

    let mut client = GanacheClient::new();

    client.take_snapshot();

    let contract_address = client.deploy(refund_address, htlc, 10);

    let refund_balance_before_htlc = client.get_balance(refund_address);
    let success_balance_before_htlc = client.get_balance(success_address);

    client.activate_flux_capacitor(2);

    let gas_used = client.send_data(refund_address, contract_address, None);

    let refund_balance_after_htlc = client.get_balance(refund_address);
    let success_balance_after_htlc = client.get_balance(success_address);

    client.restore_snapshot();

    assert_eq!(success_balance_after_htlc, success_balance_before_htlc);
    assert_eq!(
        refund_balance_before_htlc - gas_used + U256::from(10),
        refund_balance_after_htlc
    );
}

#[test]
fn given_advanced_timestamp_when_deployed_contract_cannot_yet_be_refunded() {
    let _ = env_logger::try_init();

    let refund_address: Address = "03744e31a6b9e6c6f604ff5d8ce1caef1c7bb58c".into();
    let success_address: Address = "25818640c330b071acf5fc836fe0b762a769523d".into();

    let secret = Secret::from(SECRET.clone());

    let htlc = ethereum_htlc::Htlc::new(ONE_HOUR, refund_address, success_address, secret.hash());

    let mut client = GanacheClient::new();

    client.activate_flux_capacitor(2);

    client.take_snapshot();

    let contract_address = client.deploy(refund_address, htlc, 10);

    let refund_balance_before_htlc = client.get_balance(refund_address);
    let success_balance_before_htlc = client.get_balance(success_address);

    let gas_used = client.send_data(refund_address, contract_address, None);

    let refund_balance_after_htlc = client.get_balance(refund_address);
    let success_balance_after_htlc = client.get_balance(success_address);

    client.restore_snapshot();

    assert_eq!(success_balance_after_htlc, success_balance_before_htlc);
    assert_eq!(
        refund_balance_before_htlc - gas_used,
        refund_balance_after_htlc
    );
}

#[test]
fn given_deployed_htlc_when_timeout_not_yet_reached_and_wrong_secret_then_nothing_happens() {
    let _ = env_logger::try_init();

    let refund_address: Address = "f507e0b7cb47e06bb725b605d463a56cef2c057d".into();
    let success_address: Address = "70485b398676fa6c83fa600efd3e63a75e6ac5c2".into();

    let secret = Secret::from(SECRET.clone());

    // FIXME Unfortunately, evm_increaseTime cannot be undone at the moment, so we have to add 2 hours for the increase of the last test.
    // As soon as a version of ganache-cli with ganache-core > v2.1.0 is released (https://github.com/trufflesuite/ganache-core/releases),
    // we can remove this because then https://github.com/trufflesuite/ganache-core/pull/2 is included in the release.
    let stupid_offset = 2;

    let htlc = ethereum_htlc::Htlc::new(
        ONE_HOUR * (1 + stupid_offset),
        refund_address,
        success_address,
        secret.hash(),
    );

    let mut client = GanacheClient::new();

    client.take_snapshot();

    let contract_address = client.deploy(refund_address, htlc, 10);

    let refund_balance_before_htlc = client.get_balance(refund_address);
    let success_balance_before_htlc = client.get_balance(success_address);

    let gas_used = client.send_data(refund_address, contract_address, None);

    let refund_balance_after_htlc = client.get_balance(refund_address);
    let success_balance_after_htlc = client.get_balance(success_address);

    client.restore_snapshot();

    assert_eq!(success_balance_after_htlc, success_balance_before_htlc);
    assert_eq!(
        refund_balance_before_htlc - gas_used,
        refund_balance_after_htlc
    );
}
