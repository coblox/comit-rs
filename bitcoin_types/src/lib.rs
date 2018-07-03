extern crate bitcoin;
extern crate secp256k1;

#[macro_use]
extern crate lazy_static;

mod pubkeyhash;

use secp256k1::Secp256k1;

lazy_static! {
    static ref SECP: Secp256k1 = Secp256k1::new();
}

pub use pubkeyhash::*;
