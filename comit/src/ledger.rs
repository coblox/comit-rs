use crate::ethereum::ChainId;
use fmt::Display;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// The ledger network kind. We define this as a cross blockchain domain term.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Kind {
    /// The main public ledger network.
    Mainnet,
    /// A public test network.
    Testnet,
    /// A private test network.
    Devnet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bitcoin {
    Mainnet,
    Testnet,
    Regtest,
}

pub trait LedgerKind {
    fn ledger_kind(&self) -> Kind;
}

impl LedgerKind for Bitcoin {
    fn ledger_kind(&self) -> Kind {
        match self {
            Bitcoin::Mainnet => Kind::Mainnet,
            Bitcoin::Testnet => Kind::Testnet,
            Bitcoin::Regtest => Kind::Devnet,
        }
    }
}

pub fn is_valid_ledger_pair<A, B>(a: A, b: B) -> bool
where
    A: LedgerKind,
    B: LedgerKind,
{
    a.ledger_kind() == b.ledger_kind()
}

impl Default for Bitcoin {
    fn default() -> Self {
        Self::Regtest
    }
}

impl Display for Bitcoin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Bitcoin::Mainnet => "mainnet",
            Bitcoin::Testnet => "testnet",
            Bitcoin::Regtest => "regtest",
        };

        write!(f, "{}", s)
    }
}

impl From<Bitcoin> for ::bitcoin::Network {
    fn from(bitcoin: Bitcoin) -> ::bitcoin::Network {
        match bitcoin {
            Bitcoin::Mainnet => ::bitcoin::Network::Bitcoin,
            Bitcoin::Testnet => ::bitcoin::Network::Testnet,
            Bitcoin::Regtest => ::bitcoin::Network::Regtest,
        }
    }
}

impl From<::bitcoin::Network> for Bitcoin {
    fn from(network: ::bitcoin::Network) -> Self {
        match network {
            bitcoin::Network::Bitcoin => Bitcoin::Mainnet,
            bitcoin::Network::Testnet => Bitcoin::Testnet,
            bitcoin::Network::Regtest => Bitcoin::Regtest,
        }
    }
}

impl Serialize for Bitcoin {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let str = match self {
            Bitcoin::Mainnet => "mainnet",
            Bitcoin::Testnet => "testnet",
            Bitcoin::Regtest => "regtest",
        };

        serializer.serialize_str(str)
    }
}

impl<'de> Deserialize<'de> for Bitcoin {
    fn deserialize<D>(deserializer: D) -> Result<Bitcoin, D::Error>
    where
        D: Deserializer<'de>,
    {
        let network = match String::deserialize(deserializer)?.as_str() {
            "mainnet" => Bitcoin::Mainnet,
            "testnet" => Bitcoin::Testnet,
            "regtest" => Bitcoin::Regtest,

            network => {
                return Err(<D as Deserializer<'de>>::Error::custom(format!(
                    "not regtest: {}",
                    network
                )))
            }
        };

        Ok(network)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ethereum {
    pub chain_id: ChainId,
}

impl Ethereum {
    pub fn new(chain: ChainId) -> Self {
        Ethereum { chain_id: chain }
    }
}

impl Display for Ethereum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let chain_id = u32::from(self.chain_id);
        let s = match chain_id {
            1 => "Mainnet",
            3 => "Ropsten",
            4 => "Rinkeby",
            5 => "Goerli",
            42 => "Kovan",
            _ => "Devnet",
        };

        write!(f, "{}", s)
    }
}

impl LedgerKind for Ethereum {
    fn ledger_kind(&self) -> Kind {
        let chain_id = u32::from(self.chain_id);
        match chain_id {
            1 => Kind::Mainnet,
            3 => Kind::Testnet,  // Ropsten
            4 => Kind::Testnet,  // Rinkeby
            5 => Kind::Testnet,  // Goerli
            42 => Kind::Testnet, // Kovan
            _ => Kind::Devnet,
        }
    }
}

impl From<u32> for Ethereum {
    fn from(chain_id: u32) -> Self {
        Ethereum::new(chain_id.into())
    }
}

impl Default for Ethereum {
    fn default() -> Self {
        Ethereum {
            chain_id: ChainId::REGTEST,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spectral::prelude::*;

    #[test]
    fn valid_ledger_pair() {
        let a = Ethereum::from(1);
        let b = Bitcoin::Mainnet;

        assert!(is_valid_ledger_pair(a, b))
    }

    #[test]
    fn bitcoin_serializes_as_expected() {
        let ledger = Bitcoin::Mainnet;
        let want = r#""mainnet""#.to_string();
        let got = serde_json::to_string(&ledger).expect("failed to serialize");

        assert_that(&got).is_equal_to(&want);
    }

    #[test]
    fn bitcoin_serialization_roundtrip() {
        let ledger = Bitcoin::Mainnet;
        let json = serde_json::to_string(&ledger).expect("failed to serialize");
        let rinsed: Bitcoin = serde_json::from_str(&json).expect("failed to deserialize");

        assert_eq!(ledger, rinsed);
    }

    #[test]
    fn ethereum_serializes_as_expected() {
        let ledger = Ethereum::from(1);
        let want = r#"{"chain_id":1}"#.to_string();
        let got = serde_json::to_string(&ledger).expect("failed to serialize");

        assert_that(&got).is_equal_to(&want);
    }

    #[test]
    fn ethereum_serialization_roundtrip() {
        let ledger = Ethereum::from(1);
        let json = serde_json::to_string(&ledger).expect("failed to serialize");
        let rinsed: Ethereum = serde_json::from_str(&json).expect("failed to deserialize");

        assert_eq!(ledger, rinsed);
    }
}
