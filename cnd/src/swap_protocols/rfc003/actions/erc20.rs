use crate::{
    ethereum::{Bytes, Erc20Token},
    swap_protocols::{
        actions::ethereum::{CallContract, DeployContract},
        ledger::{ethereum::ChainId, Ethereum},
        rfc003::{create_swap::HtlcParams, Secret},
    },
    timestamp::Timestamp,
};
use blockchain_contracts::ethereum::rfc003::erc20_htlc::Erc20Htlc;

pub fn deploy_action(htlc_params: HtlcParams<Ethereum, Erc20Token>) -> DeployContract {
    htlc_params.into()
}

pub fn fund_action(
    htlc_params: HtlcParams<Ethereum, Erc20Token>,
    to_erc20_contract: crate::ethereum::Address,
    beta_htlc_location: crate::ethereum::Address,
) -> CallContract {
    let chain_id = htlc_params.ledger.chain_id;
    let gas_limit = Erc20Htlc::fund_tx_gas_limit();

    let data =
        Erc20Htlc::transfer_erc20_tx_payload(htlc_params.asset.quantity.0, beta_htlc_location);

    CallContract {
        to: to_erc20_contract,
        data: Some(data),
        gas_limit,
        chain_id,
        min_block_timestamp: None,
    }
}

pub fn refund_action(
    chain_id: ChainId,
    expiry: Timestamp,
    beta_htlc_location: crate::ethereum::Address,
) -> CallContract {
    let data = Bytes::default();
    let gas_limit = Erc20Htlc::tx_gas_limit();

    CallContract {
        to: beta_htlc_location,
        data: Some(data),
        gas_limit,
        chain_id,
        min_block_timestamp: Some(expiry),
    }
}

pub fn redeem_action(
    alpha_htlc_location: crate::ethereum::Address,
    secret: Secret,
    chain_id: ChainId,
) -> CallContract {
    let data = Bytes::from(secret.as_raw_secret().to_vec());
    let gas_limit = Erc20Htlc::tx_gas_limit();

    CallContract {
        to: alpha_htlc_location,
        data: Some(data),
        gas_limit,
        chain_id,
        min_block_timestamp: None,
    }
}
