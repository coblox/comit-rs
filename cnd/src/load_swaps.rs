#![allow(clippy::type_repetition_in_bounds)]
use crate::{
    db::{DetermineTypes, LoadAcceptedSwap, Retrieve},
    init_swap::init_accepted_swap,
};
use comit::{
    asset,
    seed::DeriveSwapSeed,
    swap_protocols::{
        ledger::{self, Ethereum},
        rfc003::{events::HtlcEvents, state_store::StateStore},
    },
};

#[allow(clippy::cognitive_complexity)]
pub async fn load_swaps_from_database<D>(dependencies: D) -> anyhow::Result<()>
where
    D: StateStore
        + Clone
        + DeriveSwapSeed
        + Retrieve
        + DetermineTypes
        + HtlcEvents<ledger::bitcoin::Regtest, asset::Bitcoin>
        + HtlcEvents<ledger::bitcoin::Testnet, asset::Bitcoin>
        + HtlcEvents<ledger::bitcoin::Mainnet, asset::Bitcoin>
        + HtlcEvents<Ethereum, asset::Ether>
        + HtlcEvents<Ethereum, asset::Erc20>
        + LoadAcceptedSwap<ledger::bitcoin::Regtest, Ethereum, asset::Bitcoin, asset::Ether>
        + LoadAcceptedSwap<ledger::bitcoin::Testnet, Ethereum, asset::Bitcoin, asset::Ether>
        + LoadAcceptedSwap<ledger::bitcoin::Mainnet, Ethereum, asset::Bitcoin, asset::Ether>
        + LoadAcceptedSwap<Ethereum, ledger::bitcoin::Regtest, asset::Ether, asset::Bitcoin>
        + LoadAcceptedSwap<Ethereum, ledger::bitcoin::Testnet, asset::Ether, asset::Bitcoin>
        + LoadAcceptedSwap<Ethereum, ledger::bitcoin::Mainnet, asset::Ether, asset::Bitcoin>
        + LoadAcceptedSwap<ledger::bitcoin::Regtest, Ethereum, asset::Bitcoin, asset::Erc20>
        + LoadAcceptedSwap<ledger::bitcoin::Testnet, Ethereum, asset::Bitcoin, asset::Erc20>
        + LoadAcceptedSwap<ledger::bitcoin::Mainnet, Ethereum, asset::Bitcoin, asset::Erc20>
        + LoadAcceptedSwap<Ethereum, ledger::bitcoin::Regtest, asset::Erc20, asset::Bitcoin>
        + LoadAcceptedSwap<Ethereum, ledger::bitcoin::Testnet, asset::Erc20, asset::Bitcoin>
        + LoadAcceptedSwap<Ethereum, ledger::bitcoin::Mainnet, asset::Erc20, asset::Bitcoin>,
{
    tracing::debug!("loading swaps from database ...");

    for swap in Retrieve::all(&dependencies).await?.iter() {
        let swap_id = swap.swap_id;
        tracing::debug!("got swap from database: {}", swap_id);

        let types = DetermineTypes::determine_types(&dependencies, &swap_id).await?;

        with_swap_types!(types, {
            let accepted =
                LoadAcceptedSwap::<AL, BL, AA, BA>::load_accepted_swap(&dependencies, &swap_id)
                    .await;

            match accepted {
                Ok(accepted) => {
                    init_accepted_swap(&dependencies, accepted, types.role)?;
                }
                Err(e) => tracing::error!("failed to load swap: {}, continuing ...", e),
            };
        });
    }
    Ok(())
}
