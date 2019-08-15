use bitcoin_rpc_test_helpers::RegtestHelperClient;
use bitcoin_support::{serialize_hex, Address, BitcoinQuantity, PrivateKey};
use bitcoin_witness::{PrimedInput, PrimedTransaction, UnlockP2wpkh};
use bitcoincore_rpc::RpcApi;
use secp256k1_support::KeyPair;
use std::str::FromStr;
use testcontainers::{clients::Cli, images::coblox_bitcoincore::BitcoinCore, Docker};

#[test]
fn sign_with_rate() {
    let _ = env_logger::try_init();
    let docker = Cli::default();

    let container = docker.run(BitcoinCore::default());
    let client = tc_bitcoincore_client::new(&container);
    client.mine_bitcoins();
    let input_amount = BitcoinQuantity::from_satoshi(100_000_001);
    let private_key =
        PrivateKey::from_str("L4nZrdzNnawCtaEcYGWuPqagQA3dJxVPgN8ARTXaMLCxiYCy89wm").unwrap();
    let keypair: KeyPair = private_key.key.clone().into();

    let (_, outpoint) = client.create_p2wpkh_vout_at(keypair.public_key().clone(), input_amount);

    let alice_addr: Address = client.get_new_address(None, None).unwrap().into();

    let rate = 42;

    let primed_tx = PrimedTransaction {
        inputs: vec![PrimedInput::new(
            outpoint,
            input_amount,
            keypair.p2wpkh_unlock_parameters(),
        )],
        output_address: alice_addr.clone(),
    };

    let redeem_tx = primed_tx.sign_with_rate(rate).unwrap();

    let redeem_tx_hex = serialize_hex(&redeem_tx);

    let rpc_redeem_txid = client.send_raw_transaction(redeem_tx_hex).unwrap();

    client.generate(1, None).unwrap();

    assert!(client
        .find_utxo_at_tx_for_address(&rpc_redeem_txid, &alice_addr)
        .is_some())
}
