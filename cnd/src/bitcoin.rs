//! This module is the home of bitcoin-specific types and functionality that is
//! needed across several places in cnd.
//!
//! This involves:
//!     - Creating wrapper types to allow for more ergonomic APIs of upstream
//!       libraries
//!     - Common functionality that is not (yet) available upstream

use bitcoin::{blockdata::block::Block, secp256k1};
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::fmt::{self, Display};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PublicKey(bitcoin::PublicKey);

impl PublicKey {
    pub fn new(key: secp256k1::PublicKey) -> Self {
        Self(bitcoin::PublicKey {
            compressed: true, // we always want the PublicKey to be serialized in a compressed way
            key,
        })
    }

    pub fn from(key: bitcoin::util::key::PublicKey) -> Self {
        Self(key)
    }

    pub fn from_secret_key<C: secp256k1::Signing>(
        secp: &secp256k1::Secp256k1<C>,
        secret_key: &secp256k1::SecretKey,
    ) -> Self {
        Self::new(secp256k1::PublicKey::from_secret_key(secp, secret_key))
    }

    pub fn into_inner(self) -> bitcoin::PublicKey {
        self.0
    }
}

impl Serialize for PublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for PublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        struct PublicKeyVisitor;

        impl<'de> Visitor<'de> for PublicKeyVisitor {
            type Value = PublicKey;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "a hex-encoded, compressed public key")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                v.parse().map(PublicKey::new).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(PublicKeyVisitor)
    }
}

/// Pretty print bitcoin block.
#[derive(Debug)]
pub struct PrettyBlock<'a>(pub &'a Block);

impl Display for PrettyBlock<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Block: {:?}", self.0)
    }
}
