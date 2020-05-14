use crate::{
    asset, identity,
    network::{
        oneshot_behaviour,
        protocols::{
            announce,
            announce::{behaviour::Announce, protocol::ReplySubstream, SwapDigest},
            ethereum_identity, finalize, lightning_identity, secret_hash,
        },
        DialInformation,
    },
    seed::{DeriveSwapSeed, RootSeed},
    swap_protocols::{
        ledger::{ethereum::ChainId, lightning, Ethereum},
        rfc003::{create_swap::HtlcParams, DeriveSecret, Secret, SecretHash},
        Herc20HalightBitcoinCreateSwapParams, LocalSwapId, Role, SharedSwapId,
    },
    timestamp::{RelativeTime, Timestamp},
};
use digest::Digest;
use futures::AsyncWriteExt;
use libp2p::{
    swarm::{
        NegotiatedSubstream, NetworkBehaviour, NetworkBehaviourAction,
        NetworkBehaviourEventProcess, PollParameters,
    },
    NetworkBehaviour, PeerId,
};
use std::{
    collections::{HashMap, VecDeque},
    fmt,
    task::{Context, Poll},
};
use swaps::Swaps;

/// Setting it at 5 minutes
const PENDING_SWAP_EXPIRY_SECS: u32 = 5 * 60;

mod swaps;

/// Event emitted  by the `ComitLn` behaviour.
#[derive(Debug)]
pub enum BehaviourOutEvent {
    SwapFinalized {
        local_swap_id: LocalSwapId,
        swap_params: Herc20HalightBitcoinCreateSwapParams,
        secret_hash: SecretHash,
        ethereum_identity: identity::Ethereum,
    },
}

#[derive(NetworkBehaviour, Debug)]
#[behaviour(out_event = "BehaviourOutEvent", poll_method = "poll")]
pub struct ComitLN {
    announce: Announce,
    secret_hash: oneshot_behaviour::Behaviour<secret_hash::Message>,
    ethereum_identity: oneshot_behaviour::Behaviour<ethereum_identity::Message>,
    lightning_identity: oneshot_behaviour::Behaviour<lightning_identity::Message>,
    finalize: oneshot_behaviour::Behaviour<finalize::Message>,

    #[behaviour(ignore)]
    events: VecDeque<BehaviourOutEvent>,
    #[behaviour(ignore)]
    swaps: Swaps<ReplySubstream<NegotiatedSubstream>>,
    #[behaviour(ignore)]
    ethereum_identities: HashMap<SharedSwapId, identity::Ethereum>,
    #[behaviour(ignore)]
    lightning_identities: HashMap<SharedSwapId, identity::Lightning>,
    #[behaviour(ignore)]
    communication_state: HashMap<SharedSwapId, CommunicationState>,
    #[behaviour(ignore)]
    secret_hashes: HashMap<SharedSwapId, SecretHash>,

    #[behaviour(ignore)]
    pub seed: RootSeed,
}

#[derive(Debug, Default)]
struct CommunicationState {
    ethereum_identity_sent: bool,
    lightning_identity_sent: bool,
    received_finalized: bool,
    sent_finalized: bool,
    secret_hash_sent_or_received: bool,
}

impl ComitLN {
    pub fn new(seed: RootSeed) -> Self {
        ComitLN {
            announce: Default::default(),
            secret_hash: Default::default(),
            ethereum_identity: Default::default(),
            lightning_identity: Default::default(),
            finalize: Default::default(),
            events: VecDeque::new(),
            swaps: Default::default(),
            ethereum_identities: Default::default(),
            lightning_identities: Default::default(),
            communication_state: Default::default(),
            secret_hashes: Default::default(),
            seed,
        }
    }

    pub fn initiate_communication(
        &mut self,
        local_swap_id: LocalSwapId,
        dial_info: DialInformation,
        role: Role, // TODO: This can be deduced by the presence of shared_local_id
        digest: SwapDigest,
        data: Data,
    ) -> anyhow::Result<()> {
        tracing::trace!("Swap creation request received: {}", digest);

        match role {
            Role::Alice => {
                tracing::info!("Starting announcement for swap: {}", digest);
                self.announce
                    .start_announce_protocol(digest.clone(), dial_info.clone());
                self.swaps
                    .create_as_pending_confirmation(digest, local_swap_id, data)?;
            }
            Role::Bob => {
                if let Ok((shared_swap_id, peer, io)) = self
                    .swaps
                    .move_pending_creation_to_communicate(&digest, local_swap_id, data.clone())
                {
                    tracing::info!("Confirm & communicate for swap: {}", digest);
                    self.bob_communicate(peer, io, shared_swap_id, data)
                } else {
                    self.swaps.create_as_pending_announcement(
                        digest.clone(),
                        local_swap_id,
                        data,
                    )?;
                    tracing::debug!("Swap {} waiting for announcement", digest);
                }
            }
        }

        Ok(())
    }

    pub fn get_created_swap(
        &self,
        swap_id: &LocalSwapId,
    ) -> Option<Herc20HalightBitcoinCreateSwapParams> {
        self.swaps.get_created_swap(swap_id)
    }

    pub fn get_finalized_swap(&self, swap_id: LocalSwapId) -> Option<FinalizedSwap> {
        let (id, create_swap_params) = match self.swaps.get_announced_swap(&swap_id) {
            Some(swap) => swap,
            None => return None,
        };

        let secret = match create_swap_params.role {
            Role::Alice => Some(self.seed.derive_swap_seed(swap_id).derive_secret()),
            Role::Bob => None,
        };

        let alpha_ledger_redeem_identity = match create_swap_params.role {
            Role::Alice => match self.ethereum_identities.get(&id).copied() {
                Some(identity) => identity,
                None => return None,
            },
            Role::Bob => create_swap_params.ethereum_identity.into(),
        };
        let alpha_ledger_refund_identity = match create_swap_params.role {
            Role::Alice => create_swap_params.ethereum_identity.into(),
            Role::Bob => match self.ethereum_identities.get(&id).copied() {
                Some(identity) => identity,
                None => return None,
            },
        };
        let beta_ledger_redeem_identity = match create_swap_params.role {
            Role::Alice => create_swap_params.lightning_identity,
            Role::Bob => match self.lightning_identities.get(&id).copied() {
                Some(identity) => identity,
                None => return None,
            },
        };
        let beta_ledger_refund_identity = match create_swap_params.role {
            Role::Alice => match self.lightning_identities.get(&id).copied() {
                Some(identity) => identity,
                None => return None,
            },
            Role::Bob => create_swap_params.lightning_identity,
        };

        let erc20 = asset::Erc20 {
            token_contract: create_swap_params.token_contract.into(),
            quantity: create_swap_params.ethereum_amount,
        };

        Some(FinalizedSwap {
            alpha_ledger: Ethereum::new(ChainId::regtest()),
            beta_ledger: lightning::Regtest,
            alpha_asset: erc20,
            beta_asset: create_swap_params.lightning_amount,
            alpha_ledger_redeem_identity,
            alpha_ledger_refund_identity,
            beta_ledger_redeem_identity,
            beta_ledger_refund_identity,
            alpha_expiry: create_swap_params.ethereum_absolute_expiry,
            beta_expiry: create_swap_params.lightning_cltv_expiry,
            swap_id,
            secret,
            secret_hash: match self.secret_hashes.get(&id).copied() {
                Some(secret_hash) => secret_hash,
                None => return None,
            },
            role: create_swap_params.role,
        })
    }

    /// Once confirmation is received, exchange the information to then finalize
    fn alice_communicate(
        &mut self,
        peer: PeerId,
        swap_id: SharedSwapId,
        local_swap_id: LocalSwapId,
        create_swap_params: Herc20HalightBitcoinCreateSwapParams,
    ) {
        let addresses = self.announce.addresses_of_peer(&peer);
        self.secret_hash
            .register_addresses(peer.clone(), addresses.clone());
        self.ethereum_identity
            .register_addresses(peer.clone(), addresses.clone());
        self.lightning_identity
            .register_addresses(peer.clone(), addresses.clone());
        self.finalize.register_addresses(peer.clone(), addresses);

        self.ethereum_identity.send(
            peer.clone(),
            ethereum_identity::Message::new(swap_id, create_swap_params.ethereum_identity.into()),
        );
        self.lightning_identity.send(
            peer.clone(),
            lightning_identity::Message::new(swap_id, create_swap_params.lightning_identity),
        );

        let seed = self.seed.derive_swap_seed(local_swap_id);
        let secret_hash = seed.derive_secret().hash();

        self.secret_hashes.insert(swap_id, secret_hash);
        self.secret_hash
            .send(peer, secret_hash::Message::new(swap_id, secret_hash));

        self.communication_state
            .insert(swap_id, CommunicationState::default());
    }

    /// After announcement, confirm and then exchange information for the swap,
    /// once done, go finalize
    fn bob_communicate(
        &mut self,
        peer: libp2p::PeerId,
        io: ReplySubstream<NegotiatedSubstream>,
        data: Data,
    ) {
        let shared_swap_id = data.shared_swap_id.unwrap();

        // TODO: Should this be merged with alice_communicate?
        // Confirm
        tokio::task::spawn(io.send(shared_swap_id));

        let addresses = self.announce.addresses_of_peer(&peer);
        self.secret_hash
            .register_addresses(peer.clone(), addresses.clone());
        self.finalize.register_addresses(peer.clone(), addresses);

        // Communicate
        if let Some(ethereum_identity) = data.local_ethereum_identity {
            self.ethereum_identity
                .register_addresses(peer.clone(), addresses.clone());
            self.ethereum_identity.send(
                peer.clone(),
                ethereum_identity::Message::new(shared_swap_id, ethereum_identity.into()),
            );
        }
        if let Some(lightning_identity) = data.local_lightning_identity {
            self.lightning_identity
                .register_addresses(peer.clone(), addresses.clone());
            self.lightning_identity.send(
                peer,
                lightning_identity::Message::new(shared_swap_id, lightning_identity.into()),
            );
        }

        self.communication_state
            .insert(shared_swap_id, CommunicationState::default());
    }

    fn poll<BIE>(
        &mut self,
        _cx: &mut Context<'_>,
        _params: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<BIE, BehaviourOutEvent>> {
        let time_limit = Timestamp::now().minus(PENDING_SWAP_EXPIRY_SECS);
        self.swaps.clean_up_pending_swaps(time_limit);

        if let Some(event) = self.events.pop_front() {
            return Poll::Ready(NetworkBehaviourAction::GenerateEvent(event));
        }

        // We trust in libp2p to poll us.
        Poll::Pending
    }
}

#[derive(thiserror::Error, Clone, Copy, Debug)]
pub struct SwapExists;

impl fmt::Display for SwapExists {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // This impl is required to build but want to use a static string for
        // this when returning it via the REST API.
        write!(f, "")
    }
}

#[derive(Clone, Debug)]
pub struct FinalizedSwap {
    pub alpha_ledger: Ethereum,
    pub beta_ledger: lightning::Regtest,
    pub alpha_asset: asset::Erc20,
    pub beta_asset: asset::Bitcoin,
    pub alpha_ledger_refund_identity: identity::Ethereum,
    pub alpha_ledger_redeem_identity: identity::Ethereum,
    pub beta_ledger_refund_identity: identity::Lightning,
    pub beta_ledger_redeem_identity: identity::Lightning,
    pub alpha_expiry: Timestamp,
    pub beta_expiry: RelativeTime,
    pub swap_id: LocalSwapId,
    pub secret_hash: SecretHash,
    pub secret: Option<Secret>,
    pub role: Role,
}

impl FinalizedSwap {
    pub fn herc20_params(&self) -> HtlcParams<Ethereum, asset::Erc20, identity::Ethereum> {
        HtlcParams {
            asset: self.alpha_asset.clone(),
            ledger: Ethereum::new(ChainId::regtest()),
            redeem_identity: self.alpha_ledger_redeem_identity,
            refund_identity: self.alpha_ledger_refund_identity,
            expiry: self.alpha_expiry,
            secret_hash: self.secret_hash,
        }
    }
}

impl NetworkBehaviourEventProcess<oneshot_behaviour::OutEvent<secret_hash::Message>> for ComitLN {
    fn inject_event(&mut self, event: oneshot_behaviour::OutEvent<secret_hash::Message>) {
        let (peer, swap_id) = match event {
            oneshot_behaviour::OutEvent::Received {
                peer,
                message:
                    secret_hash::Message {
                        swap_id,
                        secret_hash,
                    },
            } => {
                self.secret_hashes
                    .insert(swap_id, SecretHash::from(secret_hash));

                let state = self
                    .communication_state
                    .get_mut(&swap_id)
                    .expect("must exist");

                state.secret_hash_sent_or_received = true;

                (peer, swap_id)
            }
            oneshot_behaviour::OutEvent::Sent {
                peer,
                message:
                    secret_hash::Message {
                        swap_id,
                        secret_hash,
                    },
            } => {
                self.secret_hashes
                    .insert(swap_id, SecretHash::from(secret_hash));

                let state = self
                    .communication_state
                    .get_mut(&swap_id)
                    .expect("should exist");

                state.secret_hash_sent_or_received = true;

                (peer, swap_id)
            }
        };

        let state = self.communication_state.get(&swap_id).unwrap();

        // check if we are done
        if self.ethereum_identities.contains_key(&swap_id)
            && self.lightning_identities.contains_key(&swap_id)
            && state.lightning_identity_sent
            && state.ethereum_identity_sent
            && state.secret_hash_sent_or_received
        {
            self.finalize.send(peer, finalize::Message::new(swap_id));
        }
    }
}

// It is already split in smaller functions
#[allow(clippy::cognitive_complexity)]
impl NetworkBehaviourEventProcess<announce::behaviour::BehaviourOutEvent> for ComitLN {
    fn inject_event(&mut self, event: announce::behaviour::BehaviourOutEvent) {
        match event {
            announce::behaviour::BehaviourOutEvent::ReceivedAnnouncement { peer, io } => {
                tracing::info!("Peer {} announced a swap ({})", peer, io.swap_digest);
                let span =
                    tracing::trace_span!("swap", digest = format_args!("{}", io.swap_digest));
                let _enter = span.enter();
                match self
                    .swaps
                    .move_pending_announcement_to_communicate(&io.swap_digest, &peer)
                {
                    Ok((shared_swap_id, create_params)) => {
                        tracing::debug!("Swap confirmation and communication has started.");
                        self.bob_communicate(peer, *io, shared_swap_id, create_params);
                    }
                    Err(swaps::Error::NotFound) => {
                        tracing::debug!("Swap has not been created yet, parking it.");
                        let _ = self
                            .swaps
                            .insert_pending_creation((&io.swap_digest).clone(), peer, *io)
                            .map_err(|_| {
                                tracing::error!(
                                    "Swap already known, Alice appeared to have sent it twice."
                                )
                            });
                    }
                    Err(err) => tracing::warn!(
                        "Announcement for {} was not processed due to {}",
                        io.swap_digest,
                        err
                    ),
                }
            }
            announce::behaviour::BehaviourOutEvent::ReceivedConfirmation {
                peer,
                swap_digest,
                swap_id: shared_swap_id,
            } => {
                let (local_swap_id, create_params) = self
                    .swaps
                    .move_pending_confirmation_to_communicate(&swap_digest, shared_swap_id)
                    .expect("we must know about this digest");

                self.alice_communicate(peer, shared_swap_id, local_swap_id, create_params);
            }
            announce::behaviour::BehaviourOutEvent::Error { peer, error } => {
                tracing::warn!(
                    "failed to complete announce protocol with {} because {:?}",
                    peer,
                    error
                );
            }
        }
    }
}

impl NetworkBehaviourEventProcess<oneshot_behaviour::OutEvent<ethereum_identity::Message>>
    for ComitLN
{
    fn inject_event(&mut self, event: oneshot_behaviour::OutEvent<ethereum_identity::Message>) {
        let (peer, swap_id) = match event {
            oneshot_behaviour::OutEvent::Received {
                peer,
                message: ethereum_identity::Message { swap_id, address },
            } => {
                self.ethereum_identities
                    .insert(swap_id, identity::Ethereum::from(address));

                (peer, swap_id)
            }
            oneshot_behaviour::OutEvent::Sent {
                peer,
                message: ethereum_identity::Message { swap_id, .. },
            } => {
                let state = self
                    .communication_state
                    .get_mut(&swap_id)
                    .expect("this should exist");

                state.ethereum_identity_sent = true;

                (peer, swap_id)
            }
        };

        let state = self.communication_state.get(&swap_id).unwrap();

        // check if we are done
        if self.ethereum_identities.contains_key(&swap_id)
            && self.lightning_identities.contains_key(&swap_id)
            && state.lightning_identity_sent
            && state.ethereum_identity_sent
            && state.secret_hash_sent_or_received
        {
            self.finalize.send(peer, finalize::Message::new(swap_id));
        }
    }
}

impl NetworkBehaviourEventProcess<oneshot_behaviour::OutEvent<lightning_identity::Message>>
    for ComitLN
{
    fn inject_event(&mut self, event: oneshot_behaviour::OutEvent<lightning_identity::Message>) {
        let (peer, swap_id) = match event {
            oneshot_behaviour::OutEvent::Received {
                peer,
                message: lightning_identity::Message { swap_id, pubkey },
            } => {
                self.lightning_identities.insert(
                    swap_id,
                    bitcoin::PublicKey::from_slice(&pubkey).unwrap().into(),
                );

                (peer, swap_id)
            }
            oneshot_behaviour::OutEvent::Sent {
                peer,
                message: lightning_identity::Message { swap_id, .. },
            } => {
                let state = self
                    .communication_state
                    .get_mut(&swap_id)
                    .expect("this should exist");

                state.lightning_identity_sent = true;

                (peer, swap_id)
            }
        };

        let state = self.communication_state.get(&swap_id).unwrap();

        // check if we are done
        if self.ethereum_identities.contains_key(&swap_id)
            && self.lightning_identities.contains_key(&swap_id)
            && state.lightning_identity_sent
            && state.ethereum_identity_sent
            && state.secret_hash_sent_or_received
        {
            self.finalize.send(peer, finalize::Message::new(swap_id));
        }
    }
}

impl NetworkBehaviourEventProcess<oneshot_behaviour::OutEvent<finalize::Message>> for ComitLN {
    fn inject_event(&mut self, event: oneshot_behaviour::OutEvent<finalize::Message>) {
        let (_, swap_id) = match event {
            oneshot_behaviour::OutEvent::Received {
                peer,
                message: finalize::Message { swap_id },
            } => {
                let state = self
                    .communication_state
                    .get_mut(&swap_id)
                    .expect("this should exist");

                state.received_finalized = true;

                (peer, swap_id)
            }
            oneshot_behaviour::OutEvent::Sent {
                peer,
                message: finalize::Message { swap_id },
            } => {
                let state = self
                    .communication_state
                    .get_mut(&swap_id)
                    .expect("this should exist");

                state.sent_finalized = true;

                (peer, swap_id)
            }
        };

        let state = self
            .communication_state
            .get_mut(&swap_id)
            .expect("this should exist");

        if state.sent_finalized && state.received_finalized {
            tracing::info!("Swap {} is finalized.", swap_id);
            let (local_swap_id, create_swap_params) = self
                .swaps
                .finalize_swap(&swap_id)
                .expect("Swap should be known");

            let secret_hash = self
                .secret_hashes
                .get(&swap_id)
                .copied()
                .expect("must exist");

            let ethereum_identity = self.ethereum_identities.get(&swap_id).copied().unwrap();

            self.events.push_back(BehaviourOutEvent::SwapFinalized {
                local_swap_id,
                swap_params: create_swap_params,
                secret_hash,
                ethereum_identity,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        asset::{ethereum::FromWei, Erc20Quantity},
        lightning,
        network::{test_swarm, DialInformation},
        swap_protocols::EthereumIdentity,
    };
    use digest::Digest;
    use futures::future;
    use libp2p::{multiaddr::Multiaddr, PeerId};
    use rand::thread_rng;

    fn make_alice_swap_params(
        bob_peer_id: PeerId,
        bob_addr: Multiaddr,
        erc20: asset::Erc20,
        lnbtc: asset::Bitcoin,
        ethereum_absolute_expiry: Timestamp,
        lightning_cltv_expiry: RelativeTime,
    ) -> Herc20HalightBitcoinCreateSwapParams {
        Herc20HalightBitcoinCreateSwapParams {
            role: Role::Alice,
            peer: DialInformation {
                peer_id: bob_peer_id,
                address_hint: Some(bob_addr),
            },
            ethereum_identity: EthereumIdentity::from(identity::Ethereum::random()),
            ethereum_absolute_expiry,
            ethereum_amount: erc20.quantity,
            lightning_identity: lightning::PublicKey::random(),
            lightning_cltv_expiry,
            lightning_amount: lnbtc,
            token_contract: erc20.token_contract.into(),
        }
    }

    fn make_bob_swap_params(
        alice_peer_id: PeerId,
        erc20: asset::Erc20,
        lnbtc: asset::Bitcoin,
        ethereum_absolute_expiry: Timestamp,
        lightning_cltv_expiry: RelativeTime,
    ) -> Herc20HalightBitcoinCreateSwapParams {
        Herc20HalightBitcoinCreateSwapParams {
            role: Role::Bob,
            peer: DialInformation {
                peer_id: alice_peer_id,
                address_hint: None,
            },
            ethereum_identity: EthereumIdentity::from(identity::Ethereum::random()),
            ethereum_absolute_expiry,
            ethereum_amount: erc20.quantity,
            lightning_identity: lightning::PublicKey::random(),
            lightning_cltv_expiry,
            lightning_amount: lnbtc,
            token_contract: erc20.token_contract.into(),
        }
    }

    #[tokio::test]
    async fn finalize_lightning_ethereum_swap_success() {
        // arrange
        let (mut alice_swarm, _, alice_peer_id) =
            test_swarm::new(ComitLN::new(RootSeed::new_random(thread_rng()).unwrap()));
        let (mut bob_swarm, bob_addr, bob_peer_id) =
            test_swarm::new(ComitLN::new(RootSeed::new_random(thread_rng()).unwrap()));

        let erc20 = asset::Erc20 {
            token_contract: Default::default(),
            quantity: Erc20Quantity::from_wei(9_001_000_000_000_000_000_000u128),
        };

        let lnbtc = asset::Bitcoin::from_sat(42);
        let ethereum_expiry = Timestamp::from(100);
        let lightning_expiry = RelativeTime::from(200);

        alice_swarm
            .initiate_communication(
                LocalSwapId::default(),
                make_alice_swap_params(
                    bob_peer_id,
                    bob_addr,
                    erc20.clone(),
                    lnbtc,
                    ethereum_expiry,
                    lightning_expiry,
                ),
            )
            .expect("initiate communication for alice");
        bob_swarm
            .initiate_communication(
                LocalSwapId::default(),
                make_bob_swap_params(
                    alice_peer_id,
                    erc20,
                    lnbtc,
                    ethereum_expiry,
                    lightning_expiry,
                ),
            )
            .expect("initiate communication for bob");

        // act
        let (alice_event, bob_event) = future::join(alice_swarm.next(), bob_swarm.next()).await;

        // assert
        match (alice_event, bob_event) {
            (
                BehaviourOutEvent::SwapFinalized {
                    local_swap_id: _alice_local_swap_id,
                    swap_params: alice_swap_params,
                    secret_hash: _alice_secret_hash,
                    ethereum_identity: _alice_eth_id,
                },
                BehaviourOutEvent::SwapFinalized {
                    local_swap_id: _bob_local_swap_id,
                    swap_params: bob_swap_params,
                    secret_hash: _bob_secret_hash,
                    ethereum_identity: _bob_eth_id,
                },
            ) => {
                assert_eq!(bob_swap_params.digest(), alice_swap_params.digest());
            }
        }
    }
}

/// All possible data to be exchanged between two nodes
/// to execute a swap
#[derive(Clone, Debug, PartialEq)]
pub struct Data {
    pub secret_hash: Option<SecretHash>,
    pub shared_swap_id: Option<SharedSwapId>,
    pub local_ethereum_identity: Option<identity::Ethereum>,
    pub remote_ethereum_identity: Option<identity::Ethereum>,
    pub local_lightning_identity: Option<identity::Lightning>,
    pub remote_lightning_identity: Option<identity::Lightning>,
}
