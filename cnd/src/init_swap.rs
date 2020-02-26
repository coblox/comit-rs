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
pub fn init_accepted_swap<D, AL, BL, AA, BA>(
    dependencies: &D,
    accepted: AcceptedSwap<AL, BL, AA, BA>,
    role: Role,
) -> anyhow::Result<()>
where
    AL: Ledger,
    BL: Ledger,
    AA: Asset,
    BA: Asset,
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
    let seed = dependencies.derive_swap_seed(id);
    tracing::trace!("initialising accepted swap: {}", id);

    match role {
        Role::Alice => {
            let state = alice::State::accepted(request, accept, seed);
            StateStore::insert(dependencies, id, state);

            tokio::task::spawn(create_swap::<D, alice::State<AL, BL, AA, BA>>(
                dependencies.clone(),
                accepted,
            ));
        }
        Role::Bob => {
            let state = bob::State::accepted(request, accept, seed);
            StateStore::insert(dependencies, id, state);

            tokio::task::spawn(create_swap::<D, bob::State<AL, BL, AA, BA>>(
                dependencies.clone(),
                accepted,
            ));
        }
    };

    Ok(())
}
