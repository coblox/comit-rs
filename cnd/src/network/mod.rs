pub mod send_request;

pub use send_request::*;

use crate::{
    db::{Save, SaveMessage, Sqlite, Swap},
    libp2p_comit_ext::{FromHeader, ToHeader},
    seed::Seed,
    swap_protocols::{
        asset::{Asset, AssetKind},
        rfc003::{
            self, bob,
            messages::{Decision, DeclineResponseBody, SwapDeclineReason},
            state_store::{InMemoryStateStore, StateStore},
            Ledger,
        },
        HashFunction, LedgerConnectors, LedgerKind, Role, SwapId, SwapProtocol,
    },
};
use futures::{
    future::Future,
    sync::oneshot::{self, Sender},
};
use libp2p::{
    core::muxing::{StreamMuxer, SubstreamRef},
    mdns::{Mdns, MdnsEvent},
    swarm::NetworkBehaviourEventProcess,
    Multiaddr, NetworkBehaviour, PeerId, Swarm, Transport,
};
use libp2p_comit::{
    frame::{OutboundRequest, Response, ValidatedInboundRequest},
    BehaviourOutEvent, Comit, PendingInboundRequest,
};
use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    io,
    sync::{Arc, Mutex},
};

#[derive(NetworkBehaviour)]
#[allow(missing_debug_implementations)]
pub struct ComitNode<TSubstream> {
    comit: Comit<TSubstream>,
    mdns: Mdns<TSubstream>,

    #[behaviour(ignore)]
    pub ledger_events: LedgerConnectors,
    #[behaviour(ignore)]
    pub state_store: Arc<InMemoryStateStore>,
    #[behaviour(ignore)]
    pub seed: Seed,
    #[behaviour(ignore)]
    pub database: Sqlite,
    #[behaviour(ignore)]
    response_channels: HashMap<SwapId, oneshot::Sender<Response>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DialInformation {
    pub peer_id: PeerId,
    pub address_hint: Option<Multiaddr>,
}

impl Display for DialInformation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match &self.address_hint {
            None => write!(f, "{}", self.peer_id),
            Some(address_hint) => write!(f, "{}@{}", self.peer_id, address_hint),
        }
    }
}

impl<TSubstream> ComitNode<TSubstream> {
    pub fn new(
        ledger_events: LedgerConnectors,
        state_store: Arc<InMemoryStateStore>,
        seed: Seed,
        database: Sqlite,
    ) -> Result<Self, io::Error> {
        let mut swap_headers = HashSet::new();
        swap_headers.insert("id".into());
        swap_headers.insert("alpha_ledger".into());
        swap_headers.insert("beta_ledger".into());
        swap_headers.insert("alpha_asset".into());
        swap_headers.insert("beta_asset".into());
        swap_headers.insert("protocol".into());

        let mut known_headers = HashMap::new();
        known_headers.insert("SWAP".into(), swap_headers);

        Ok(Self {
            comit: Comit::new(known_headers),
            mdns: Mdns::new()?,
            ledger_events,
            state_store,
            seed,
            database,
            response_channels: HashMap::new(),
        })
    }

    pub fn send_request(
        &mut self,
        peer_id: DialInformation,
        request: OutboundRequest,
    ) -> Box<dyn Future<Item = Response, Error = ()> + Send> {
        self.comit
            .send_request((peer_id.peer_id, peer_id.address_hint), request)
    }

    fn handle_request(
        &mut self,
        counterparty: PeerId,
        mut request: ValidatedInboundRequest,
    ) -> Result<SwapId, Response> {
        match request.request_type() {
            "SWAP" => {
                let protocol: SwapProtocol = header!(request
                    .take_header("protocol")
                    .map(SwapProtocol::from_header));
                match protocol {
                    SwapProtocol::Rfc003(hash_function) => {
                        let swap_id = header!(request.take_header("id").map(SwapId::from_header));
                        let alpha_ledger = header!(request
                            .take_header("alpha_ledger")
                            .map(LedgerKind::from_header));
                        let beta_ledger = header!(request
                            .take_header("beta_ledger")
                            .map(LedgerKind::from_header));
                        let alpha_asset = header!(request
                            .take_header("alpha_asset")
                            .map(AssetKind::from_header));
                        let beta_asset = header!(request
                            .take_header("beta_asset")
                            .map(AssetKind::from_header));

                        match (alpha_ledger, beta_ledger, alpha_asset, beta_asset) {
                            (
                                LedgerKind::Bitcoin(alpha_ledger),
                                LedgerKind::Ethereum(beta_ledger),
                                AssetKind::Bitcoin(alpha_asset),
                                AssetKind::Ether(beta_asset),
                            ) => {
                                let request = rfc003_swap_request(
                                    swap_id,
                                    alpha_ledger,
                                    beta_ledger,
                                    alpha_asset,
                                    beta_asset,
                                    hash_function,
                                    body!(request.take_body_as()),
                                );
                                self.insert_state_for_bob(counterparty, request)
                                    .expect("Could not save state to database");

                                Ok(swap_id)
                            }
                            (
                                LedgerKind::Ethereum(alpha_ledger),
                                LedgerKind::Bitcoin(beta_ledger),
                                AssetKind::Ether(alpha_asset),
                                AssetKind::Bitcoin(beta_asset),
                            ) => {
                                let request = rfc003_swap_request(
                                    swap_id,
                                    alpha_ledger,
                                    beta_ledger,
                                    alpha_asset,
                                    beta_asset,
                                    hash_function,
                                    body!(request.take_body_as()),
                                );
                                self.insert_state_for_bob(counterparty, request)
                                    .expect("Could not save state to database");

                                Ok(swap_id)
                            }
                            (
                                LedgerKind::Bitcoin(alpha_ledger),
                                LedgerKind::Ethereum(beta_ledger),
                                AssetKind::Bitcoin(alpha_asset),
                                AssetKind::Erc20(beta_asset),
                            ) => {
                                let request = rfc003_swap_request(
                                    swap_id,
                                    alpha_ledger,
                                    beta_ledger,
                                    alpha_asset,
                                    beta_asset,
                                    hash_function,
                                    body!(request.take_body_as()),
                                );
                                self.insert_state_for_bob(counterparty, request)
                                    .expect("Could not save state to database");

                                Ok(swap_id)
                            }
                            (
                                LedgerKind::Ethereum(alpha_ledger),
                                LedgerKind::Bitcoin(beta_ledger),
                                AssetKind::Erc20(alpha_asset),
                                AssetKind::Bitcoin(beta_asset),
                            ) => {
                                let request = rfc003_swap_request(
                                    swap_id,
                                    alpha_ledger,
                                    beta_ledger,
                                    alpha_asset,
                                    beta_asset,
                                    hash_function,
                                    body!(request.take_body_as()),
                                );
                                self.insert_state_for_bob(counterparty, request)
                                    .expect("Could not save state to database");

                                Ok(swap_id)
                            }
                            (alpha_ledger, beta_ledger, alpha_asset, beta_asset) => {
                                log::warn!(
                                    "swapping {:?} to {:?} from {:?} to {:?} is currently not supported", alpha_asset, beta_asset, alpha_ledger, beta_ledger
                                );

                                let decline_body = DeclineResponseBody {
                                    reason: Some(SwapDeclineReason::UnsupportedSwap),
                                };

                                Err(Response::empty()
                                        .with_header(
                                            "decision",
                                            Decision::Declined
                                                .to_header()
                                                .expect("Decision should not fail to serialize"),
                                        )
                                        .with_body(serde_json::to_value(decline_body).expect(
                                            "decline body should always serialize into serde_json::Value",
                                        )),
                                )
                            }
                        }
                    }
                    SwapProtocol::Unknown(protocol) => {
                        log::warn!("the swap protocol {} is currently not supported", protocol);

                        let decline_body = DeclineResponseBody {
                            reason: Some(SwapDeclineReason::UnsupportedProtocol),
                        };
                        Err(Response::empty()
                            .with_header(
                                "decision",
                                Decision::Declined
                                    .to_header()
                                    .expect("Decision should not fail to serialize"),
                            )
                            .with_body(serde_json::to_value(decline_body).expect(
                                "decline body should always serialize into serde_json::Value",
                            )))
                    }
                }
            }

            // This case is just catered for, because of rust. It can only happen
            // if there is a typo in the request_type within the program. The request
            // type is checked on the messaging layer and will be handled there if
            // an unknown request_type is passed in.
            request_type => {
                log::warn!("request type '{}' is unknown", request_type);

                Err(Response::empty().with_header(
                    "decision",
                    Decision::Declined
                        .to_header()
                        .expect("Decision should not fail to serialize"),
                ))
            }
        }
    }

    #[allow(clippy::type_complexity)]
    fn insert_state_for_bob<AL: Ledger, BL: Ledger, AA: Asset, BA: Asset>(
        &self,
        counterparty: PeerId,
        swap_request: rfc003::Request<AL, BL, AA, BA>,
    ) -> anyhow::Result<()>
    where
        Sqlite: SaveMessage<rfc003::Request<AL, BL, AA, BA>>,
    {
        let id = swap_request.swap_id;
        let seed = self.seed.swap_seed(id);

        self.database.save(Swap::new(id, Role::Bob, counterparty))?;
        self.database.save_message(swap_request.clone())?;

        let state_store = Arc::clone(&self.state_store);
        let state = bob::State::proposed(swap_request.clone(), seed);
        state_store.insert(id, state);

        Ok(())
    }
}

pub trait Network: Send + Sync + 'static {
    fn comit_peers(&self) -> Box<dyn Iterator<Item = (PeerId, Vec<Multiaddr>)> + Send + 'static>;
    fn listen_addresses(&self) -> Vec<Multiaddr>;
    fn pending_request_for(&self, swap: SwapId) -> Option<oneshot::Sender<Response>>;
}

impl<TTransport: Transport + Send + 'static, TMuxer: StreamMuxer + Send + Sync + 'static> Network
    for Mutex<Swarm<TTransport, ComitNode<SubstreamRef<Arc<TMuxer>>>>>
where
    <TMuxer as StreamMuxer>::OutboundSubstream: Send + 'static,
    <TMuxer as StreamMuxer>::Substream: Send + 'static,
    <TTransport as Transport>::Dial: Send,
    <TTransport as Transport>::Error: Send,
    <TTransport as Transport>::Listener: Send,
    <TTransport as Transport>::ListenerUpgrade: Send,
    TTransport: Transport<Output = (PeerId, TMuxer)> + Clone,
{
    fn comit_peers(&self) -> Box<dyn Iterator<Item = (PeerId, Vec<Multiaddr>)> + Send + 'static> {
        let mut swarm = self.lock().unwrap();

        Box::new(swarm.comit.connected_peers())
    }

    fn listen_addresses(&self) -> Vec<Multiaddr> {
        let swarm = self.lock().unwrap();

        Swarm::listeners(&swarm)
            .chain(Swarm::external_addresses(&swarm))
            .cloned()
            .collect()
    }

    fn pending_request_for(&self, swap: SwapId) -> Option<Sender<Response>> {
        let mut swarm = self.lock().unwrap();

        swarm.response_channels.remove(&swap)
    }
}

impl<TSubstream> NetworkBehaviourEventProcess<BehaviourOutEvent> for ComitNode<TSubstream> {
    fn inject_event(&mut self, event: BehaviourOutEvent) {
        match event {
            BehaviourOutEvent::PendingInboundRequest { request, peer_id } => {
                let PendingInboundRequest { request, channel } = request;

                match self.handle_request(peer_id, request) {
                    Ok(id) => {
                        self.response_channels.insert(id, channel);
                    }
                    Err(response) => channel
                        .send(response)
                        .unwrap_or_else(|_| log::debug!("failed to send response through channel")),
                }
            }
        }
    }
}

impl<TSubstream> NetworkBehaviourEventProcess<libp2p::mdns::MdnsEvent> for ComitNode<TSubstream> {
    fn inject_event(&mut self, event: libp2p::mdns::MdnsEvent) {
        match event {
            MdnsEvent::Discovered(addresses) => {
                for (peer, address) in addresses {
                    log::trace!("discovered {} at {}", peer, address)
                }
            }
            MdnsEvent::Expired(addresses) => {
                for (peer, address) in addresses {
                    log::trace!("address {} of peer {} expired", address, peer)
                }
            }
        }
    }
}

fn rfc003_swap_request<AL: rfc003::Ledger, BL: rfc003::Ledger, AA: Asset, BA: Asset>(
    id: SwapId,
    alpha_ledger: AL,
    beta_ledger: BL,
    alpha_asset: AA,
    beta_asset: BA,
    hash_function: HashFunction,
    body: rfc003::messages::RequestBody<AL, BL>,
) -> rfc003::Request<AL, BL, AA, BA> {
    rfc003::Request::<AL, BL, AA, BA> {
        swap_id: id,
        alpha_asset,
        beta_asset,
        alpha_ledger,
        beta_ledger,
        hash_function,
        alpha_ledger_refund_identity: body.alpha_ledger_refund_identity,
        beta_ledger_redeem_identity: body.beta_ledger_redeem_identity,
        alpha_expiry: body.alpha_expiry,
        beta_expiry: body.beta_expiry,
        secret_hash: body.secret_hash,
    }
}
