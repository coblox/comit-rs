use bitcoin_rpc::Address;
use secret::{Secret, SecretHash};
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Symbol(pub String); // Expected format: BTC:LTC

#[derive(Serialize, Deserialize)]
pub struct OfferRequest {
    pub symbol: Symbol,
    sell_amount: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Offer {
    pub uid: Uuid,
    pub symbol: Symbol,
    pub rate: f32,
    pub address: Address,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SwapProposal {
    pub uid: Uuid,
    pub symbol: Symbol,
    pub rate: f32,
    pub address: Address,
    pub secret_hash: SecretHash,
}

pub struct SwapData {
    offer: SwapProposal,
    secret: Secret,
}

impl SwapProposal {
    pub fn new(
        uid: Uuid,
        symbol: Symbol,
        rate: f32,
        address: Address,
        secret_hash: SecretHash,
    ) -> SwapProposal {
        SwapProposal {
            uid,
            symbol,
            rate,
            address,
            secret_hash,
        }
    }

    pub fn from_exchange_offer(exchange_offer: Offer, secret_hash: SecretHash) -> SwapProposal {
        SwapProposal::new(
            exchange_offer.uid,
            exchange_offer.symbol,
            exchange_offer.rate,
            exchange_offer.address,
            secret_hash,
        )
    }
}

impl SwapData {
    pub fn new(offer: SwapProposal, secret: Secret) -> SwapData {
        SwapData { offer, secret }
    }

    pub fn uid(&self) -> Uuid {
        self.offer.uid
    }
}

#[derive(Clone)]
pub struct ExchangeApiUrl(pub String);

pub struct Offers {
    pub all_offers: Mutex<HashMap<Uuid, SwapData>>,
}

impl Offers {
    pub fn new() -> Offers {
        Offers {
            all_offers: Mutex::new(HashMap::new()),
        }
    }
}
