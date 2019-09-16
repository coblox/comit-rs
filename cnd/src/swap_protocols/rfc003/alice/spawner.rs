use crate::{
    metadata_store::{self, Metadata, MetadataStore, Role},
    network::DialInformation,
    state_store::{self, StateStore},
    swap_protocols::{
        self, create_ledger_events::CreateLedgerEvents, dependencies::LedgerEventDependencies,
    },
};
use comit::{
    asset::Asset,
    rfc003::{alice, messages::ToRequest, Ledger},
    Client, SwapId,
};
use futures::{sync::mpsc, Future, Stream};
use http_api_problem::HttpApiProblem;
use std::sync::Arc;

#[derive(Debug)]
pub enum Error {
    Storage(state_store::Error),
    Metadata(metadata_store::Error),
}

impl From<Error> for HttpApiProblem {
    fn from(e: Error) -> Self {
        use self::Error::*;
        match e {
            Storage(e) => e.into(),
            Metadata(e) => e.into(),
        }
    }
}

pub trait AliceSpawner: Send + Sync + 'static {
    fn spawn<AL: Ledger, BL: Ledger, AA: Asset, BA: Asset>(
        &self,
        id: SwapId,
        bob_dial_info: DialInformation,
        swap_request: Box<dyn ToRequest<AL, BL, AA, BA>>,
    ) -> Result<(), Error>
    where
        LedgerEventDependencies: CreateLedgerEvents<AL, AA> + CreateLedgerEvents<BL, BA>;
}

impl<T: MetadataStore, S: StateStore, C: Client> AliceSpawner
    for swap_protocols::alice::ProtocolDependencies<T, S, C>
{
    fn spawn<AL: Ledger, BL: Ledger, AA: Asset, BA: Asset>(
        &self,
        id: SwapId,
        bob_dial_info: DialInformation,
        partial_swap_request: Box<dyn ToRequest<AL, BL, AA, BA>>,
    ) -> Result<(), Error>
    where
        LedgerEventDependencies: CreateLedgerEvents<AL, AA> + CreateLedgerEvents<BL, BA>,
    {
        let swap_seed = Arc::new(self.seed.swap_seed(id));

        let swap_request = partial_swap_request.to_request(swap_seed.as_ref());
        let alice = alice::State::new(swap_request.clone(), swap_seed);

        let metadata = Metadata::new(
            id,
            swap_request.alpha_ledger.into(),
            swap_request.beta_ledger.into(),
            swap_request.alpha_asset.into(),
            swap_request.beta_asset.into(),
            Role::Alice,
            bob_dial_info.peer_id.to_owned(),
        );

        self.metadata_store
            .insert(metadata)
            .map_err(Error::Metadata)?;

        let (sender, receiver) = mpsc::unbounded();

        let swap_execution = {
            let ledger_events = self.ledger_events.clone();
            alice.new_state_machine(
                ledger_events.create_ledger_events(),
                ledger_events.create_ledger_events(),
                self.client.clone(),
                bob_dial_info,
                Arc::new(sender),
            )
        };

        let state_store = Arc::clone(&self.state_store);
        state_store.insert(id, alice);
        tokio::spawn(receiver.for_each(move |update| {
            state_store.update::<alice::State<AL, BL, AA, BA>>(&id, update);
            Ok(())
        }));

        tokio::spawn(
            swap_execution
                .map(move |outcome| {
                    log::info!("Swap {} finished with {:?}", id, outcome);
                })
                .map_err(move |e| {
                    log::error!("Swap {} failed with {:?}", id, e);
                }),
        );

        Ok(())
    }
}
