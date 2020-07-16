use crate::{
    asset, ledger,
    network::protocols::orderbook::{MakerId, SwapType, TradingPair},
};
use libp2p::{gossipsub::Topic, PeerId};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Hash, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrderId(Uuid);

impl OrderId {
    pub fn random() -> OrderId {
        OrderId(Uuid::new_v4())
    }
}

impl Display for OrderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for OrderId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let uuid = Uuid::from_str(s)?;
        Ok(OrderId(uuid))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Order {
    pub id: OrderId,
    pub maker: MakerId,
    #[serde(with = "asset::bitcoin::sats_as_string")]
    pub btc_quantity: asset::Bitcoin,
    pub bitcoin_ledger: ledger::Bitcoin,
    pub erc20_quantity: asset::Erc20,
    pub ethereum_ledger: ledger::Ethereum,
    pub alpha_expiry: u32,
    pub beta_expiry: u32,
}

impl Order {
    pub fn topic(&self) -> Topic {
        let peer_id = PeerId::from(self.maker.clone());
        TradingPair {
            buy: SwapType::Hbit,
            sell: SwapType::Herc20,
        }
        .to_topic(&peer_id)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn order_id_serialization_roundtrip() {
        // TODO: Implement order_id_serialization_roundtrip()
    }

    #[test]
    fn order_id_serialization_stability() {
        // TODO: Implement order_id_serialization_stability()
    }

    #[test]
    fn order_serialization_roundtrip() {
        // TODO: Implement order_serialization_roundtrip()
    }

    #[test]
    fn order_serialization_stability() {
        // TODO: Implement order_serialization_stability()
    }
}
