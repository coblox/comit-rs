extern crate bitcoin_rpc_client;
extern crate bitcoin_support;
extern crate crypto;
#[macro_use]
extern crate lazy_static;
extern crate secp256k1_support;
extern crate uuid;
#[macro_use]
extern crate log;

use secp256k1_support::{All, Secp256k1};

lazy_static! {
    static ref SECP: Secp256k1<All> = Secp256k1::new();
}

pub mod fake_key_store;
mod key_store;

pub use key_store::*;
