mod makerbook;
mod order_source;

use crate::{
    asset,
    order::SwapProtocol,
    orderpool::{Match, OrderPool},
    BtcDaiOrder, OrderId, Position,
};
use libp2p::{
    swarm::{NetworkBehaviourAction, NetworkBehaviourEventProcess, PollParameters},
    NetworkBehaviour, PeerId,
};
use makerbook::Makerbook;
use order_source::*;
use std::{
    collections::VecDeque,
    task::{Context, Poll},
};

/// The Orderbook libp2p network behaviour.
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "BehaviourOutEvent", poll_method = "my_poll")]
#[allow(missing_debug_implementations)]
pub struct Orderbook {
    makerbook: Makerbook,
    order_source: OrderSource,

    #[behaviour(ignore)]
    events: VecDeque<BehaviourOutEvent>,
    #[behaviour(ignore)]
    orderpool: OrderPool,
}

/// Represents a "form" that contains all data for creating a new order.
#[derive(Debug, Copy, Clone)]
pub struct BtcDaiOrderForm {
    pub position: Position,
    pub quantity: asset::Bitcoin,
    pub price: u64,
    pub swap_protocol: SwapProtocol,
}

impl Orderbook {
    /// Construct a new orderbook for this node using the node's peer ID.
    pub fn new(me: PeerId) -> Orderbook {
        Orderbook {
            makerbook: Makerbook::new(me.clone()),
            order_source: OrderSource::default(),
            events: VecDeque::new(),
            orderpool: OrderPool::new(me),
        }
    }

    /// Declare oneself to the network as a maker.
    pub fn declare_as_maker(&mut self) {
        self.makerbook.login();
    }

    /// Announce retraction of oneself as a maker, undoes `declare_as_maker()`.
    pub fn retract(&mut self) {
        self.makerbook.logout();
        self.orderpool.clear_own_orders();
    }

    /// Make a new order.
    ///
    /// The resulting order will eventually be visible to other peers.
    pub fn publish(&mut self, form: BtcDaiOrderForm) -> OrderId {
        let order = BtcDaiOrder::new(form.position, form.quantity, form.price, form.swap_protocol);
        let id = order.id;

        self.orderpool.publish(order);

        id
    }

    /// Cancel an order we previously published.
    pub fn cancel(&mut self, id: OrderId) {
        self.orderpool.cancel(id);
    }

    pub fn orderpool(&self) -> &OrderPool {
        &self.orderpool
    }

    pub fn orderpool_mut(&mut self) -> &mut OrderPool {
        &mut self.orderpool
    }

    fn my_poll<BIE>(
        &mut self,
        _: &mut Context<'_>,
        _: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<BIE, BehaviourOutEvent>> {
        // first, emit all events
        if let Some(event) = self.events.pop_front() {
            return Poll::Ready(NetworkBehaviourAction::GenerateEvent(event));
        }

        // only if we have no events, try to match again
        for r#match in self.orderpool.matches() {
            self.events
                .push_back(BehaviourOutEvent::OrderMatch(r#match));
        }

        Poll::Pending
    }
}

#[derive(PartialEq, Clone, Copy, Debug, thiserror::Error)]
#[error("order {0} not found in orderbook")]
pub struct OrderNotFound(OrderId);

/// Event emitted by the `Orderbook` behaviour.
#[derive(Debug)]
pub enum BehaviourOutEvent {
    OrderMatch(Match),
}

impl NetworkBehaviourEventProcess<makerbook::BehaviourOutEvent> for Orderbook {
    fn inject_event(&mut self, event: makerbook::BehaviourOutEvent) {
        match event {
            makerbook::BehaviourOutEvent::Logout { peer } => {
                self.order_source.stop_getting_orders_from(&peer);
                self.orderpool.remove_all_from(&peer)
            }
        }
    }
}

impl NetworkBehaviourEventProcess<order_source::BehaviourOutEvent> for Orderbook {
    fn inject_event(&mut self, event: order_source::BehaviourOutEvent) {
        match event {
            order_source::BehaviourOutEvent::GetOrdersRequest { response_handle } => {
                self.order_source
                    .send_orders(response_handle, self.orderpool.ours().cloned().collect());
            }
            order_source::BehaviourOutEvent::RetrievedOrders { maker, orders } => {
                if !orders.is_empty() {
                    self.orderpool.receive(maker, orders);
                }
            }
            order_source::BehaviourOutEvent::MakerIsGone { maker } => {
                self.orderpool.remove_all_from(&maker);
            }
        }
    }
}
