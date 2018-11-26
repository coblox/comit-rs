use comit_client;
use futures::{stream::Stream, sync::mpsc::UnboundedReceiver, Future};
use key_store::KeyStore;
use ledger_query_service::{DefaultLedgerQueryServiceApiClient, FirstMatch, QueryIdCache};
use rand::thread_rng;
use std::{marker::PhantomData, net::SocketAddr, sync::Arc, time::Duration};
use swap_protocols::{
    asset::Asset,
    metadata_store::MetadataStore,
    rfc003::{
        alice::SwapRequestKind,
        events::{AliceToBob, CommunicationEvents, LedgerEvents, LqsEvents, LqsEventsForErc20},
        roles::Alice,
        state_machine::{Context, Start, Swap, SwapStates},
        state_store::StateStore,
        Ledger, Secret,
    },
};
use swaps::common::SwapId;

#[derive(Debug)]
pub struct SwapRequestHandler<
    C: comit_client::Client,
    F: comit_client::ClientFactory<C> + 'static,
    MetadataStore,
    StateStore,
> {
    // new dependencies
    pub receiver: UnboundedReceiver<(SwapId, SwapRequestKind)>,
    pub metadata_store: Arc<MetadataStore>,
    pub key_store: Arc<KeyStore>,
    pub state_store: Arc<StateStore>,
    pub lqs_api_client: Arc<DefaultLedgerQueryServiceApiClient>,
    // legacy code dependencies
    pub client_factory: Arc<F>,
    pub comit_node_addr: SocketAddr,
    pub phantom_data: PhantomData<C>,
    pub bitcoin_poll_interval: Duration,
    pub ethereum_poll_interval: Duration,
}

impl<
        C: comit_client::Client,
        F: comit_client::ClientFactory<C> + 'static,
        M: MetadataStore<SwapId>,
        S: StateStore<SwapId>,
    > SwapRequestHandler<C, F, M, S>
{
    pub fn start(self) -> impl Future<Item = (), Error = ()> {
        let (receiver, metadata_store, bitcoin_poll_interval, ethereum_poll_interval) = (
            self.receiver,
            self.metadata_store,
            self.bitcoin_poll_interval,
            self.ethereum_poll_interval,
        );
        let key_store = Arc::clone(&self.key_store);
        let state_store = Arc::clone(&self.state_store);
        let lqs_api_client = Arc::clone(&self.lqs_api_client);
        let client_factory = Arc::clone(&self.client_factory);
        let comit_node_addr = self.comit_node_addr.clone();

        receiver
            .for_each(move |(id, requests)| {
                match requests {
                    SwapRequestKind::BitcoinEthereumBitcoinQuantityEtherQuantity(request) => {
                        // TODO: Store this somewhere
                        let _alpha_ledger_final_refund_identity =
                            request.alpha_ledger_refund_identity;
                        let alpha_ledger_refund_identity =
                            key_store.get_transient_keypair(&id.into(), b"REFUND");

                        if let Err(e) = metadata_store.insert(id, request.clone()) {
                            error!("Failed to store metadata for swap {} because {:?}", id, e);
                            // Return Ok to keep the loop running
                            return Ok(());
                        }

                        let secret = Secret::generate(&mut thread_rng());

                        let start_state = Start {
                            alpha_ledger_refund_identity,
                            beta_ledger_success_identity: request.beta_ledger_success_identity,
                            alpha_ledger: request.alpha_ledger,
                            beta_ledger: request.beta_ledger,
                            alpha_asset: request.alpha_asset,
                            beta_asset: request.beta_asset,
                            alpha_ledger_lock_duration: request.alpha_ledger_lock_duration,
                            secret,
                        };

                        let comit_client = match client_factory.client_for(comit_node_addr) {
                            Ok(client) => client,
                            Err(e) => {
                                debug!("Couldn't get client for {}: {:?}", comit_node_addr, e);
                                return Ok(());
                            }
                        };

                        spawn_state_machine(
                            id,
                            start_state,
                            state_store.as_ref(),
                            Box::new(LqsEvents::new(
                                QueryIdCache::wrap(Arc::clone(&lqs_api_client)),
                                FirstMatch::new(Arc::clone(&lqs_api_client), bitcoin_poll_interval),
                            )),
                            Box::new(LqsEvents::new(
                                QueryIdCache::wrap(Arc::clone(&lqs_api_client)),
                                FirstMatch::new(
                                    Arc::clone(&lqs_api_client),
                                    ethereum_poll_interval,
                                ),
                            )),
                            Box::new(AliceToBob::new(Arc::clone(&comit_client))),
                        );
                        Ok(())
                    }
                    SwapRequestKind::EthereumBitcoinEtherQuantityBitcoinQuantity(request) => {
                        unimplemented!()
                    }
                    SwapRequestKind::BitcoinEthereumBitcoinQuantityErc20Quantity(request) => {
                        // TODO: Store this somewhere
                        let _alpha_ledger_final_refund_identity =
                            request.alpha_ledger_refund_identity;
                        let alpha_ledger_refund_identity =
                            key_store.get_transient_keypair(&id.into(), b"REFUND");

                        if let Err(e) = metadata_store.insert(id, request.clone()) {
                            error!("Failed to store metadata for swap {} because {:?}", id, e);
                            // Return Ok to keep the loop running
                            return Ok(());
                        }

                        let secret = Secret::generate(&mut thread_rng());

                        let start_state = Start {
                            alpha_ledger_refund_identity,
                            beta_ledger_success_identity: request.beta_ledger_success_identity,
                            alpha_ledger: request.alpha_ledger,
                            beta_ledger: request.beta_ledger,
                            alpha_asset: request.alpha_asset,
                            beta_asset: request.beta_asset,
                            alpha_ledger_lock_duration: request.alpha_ledger_lock_duration,
                            secret,
                        };

                        let comit_client = match client_factory.client_for(comit_node_addr) {
                            Ok(client) => client,
                            Err(e) => {
                                debug!("Couldn't get client for {}: {:?}", comit_node_addr, e);
                                return Ok(());
                            }
                        };

                        spawn_state_machine(
                            id,
                            start_state,
                            state_store.as_ref(),
                            Box::new(LqsEvents::new(
                                QueryIdCache::wrap(Arc::clone(&lqs_api_client)),
                                FirstMatch::new(Arc::clone(&lqs_api_client), bitcoin_poll_interval),
                            )),
                            Box::new(LqsEventsForErc20::new(
                                QueryIdCache::wrap(Arc::clone(&lqs_api_client)),
                                FirstMatch::new(
                                    Arc::clone(&lqs_api_client),
                                    ethereum_poll_interval,
                                ),
                            )),
                            Box::new(AliceToBob::new(Arc::clone(&comit_client))),
                        );
                        Ok(())
                    }
                    SwapRequestKind::EthereumBitcoinErc20QuantityBitcoinQuantity(request) => {
                        unimplemented!()
                    }
                }
            })
            .map_err(|_| ())
    }
}

fn spawn_state_machine<AL: Ledger, BL: Ledger, AA: Asset, BA: Asset, S: StateStore<SwapId>>(
    id: SwapId,
    start_state: Start<Alice<AL, BL, AA, BA>>,
    state_store: &S,
    alpha_ledger_events: Box<LedgerEvents<AL, AA>>,
    beta_ledger_events: Box<LedgerEvents<BL, BA>>,
    communication_events: Box<CommunicationEvents<Alice<AL, BL, AA, BA>>>,
) {
    let state = SwapStates::Start(start_state);
    let state_repo = state_store.insert(id, state.clone()).expect("");

    let context = Context {
        alpha_ledger_events,
        beta_ledger_events,
        communication_events,
        state_repo,
    };

    tokio::spawn(
        Swap::start_in(state, context)
            .map(move |outcome| {
                info!("Swap {} finished with {:?}", id, outcome);
            })
            .map_err(move |e| {
                error!("Swap {} failed with {:?}", id, e);
            }),
    );
}
