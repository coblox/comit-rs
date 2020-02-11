use crate::{
    asset::Asset,
    db::AcceptedSwap,
    seed::DeriveSwapSeed,
    swap_protocols::{
        rfc003::{
            alice, bob, create_swap,
            events::{HtlcDeployed, HtlcFunded, HtlcRedeemed, HtlcRefunded},
            state_store::StateStore,
            Ledger,
        },
        Role,
    },
};

#[allow(clippy::cognitive_complexity)]
pub fn init_accepted_swap<D, AL: Ledger, BL: Ledger, AA: Asset, BA: Asset>(
    facade: &D,
    accepted: AcceptedSwap<AL, BL, AA, BA>,
    role: Role,
) -> anyhow::Result<()>
where
    D: StateStore
        + Clone
        + DeriveSwapSeed
        + HtlcFunded<AL, AA>
        + HtlcFunded<BL, BA>
        + HtlcDeployed<AL, AA>
        + HtlcDeployed<BL, BA>
        + HtlcRedeemed<AL, AA>
        + HtlcRedeemed<BL, BA>
        + HtlcRefunded<AL, AA>
        + HtlcRefunded<BL, BA>,
{
    let (request, accept, _at) = accepted.clone();

    let id = request.swap_id;
    let seed = facade.derive_swap_seed(id);
    tracing::trace!("initialising accepted swap: {}", id);

    match role {
        Role::Alice => {
            let state = alice::State::accepted(request, accept, seed);
            StateStore::insert(facade, id, state);

            tokio::task::spawn(create_swap::<D, alice::State<AL, BL, AA, BA>>(
                facade.clone(),
                accepted,
            ));
        }
        Role::Bob => {
            let state = bob::State::accepted(request, accept, seed);
            StateStore::insert(facade, id, state);

            tokio::task::spawn(create_swap::<D, bob::State<AL, BL, AA, BA>>(
                facade.clone(),
                accepted,
            ));
        }
    };

    Ok(())
}
