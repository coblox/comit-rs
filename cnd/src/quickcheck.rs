use crate::{
    asset,
    asset::ethereum::FromWei,
    ethereum::{Address, Bytes, ChainId},
    identity, ledger, transaction, Role, SecretHash, Timestamp,
};
use ::bitcoin::{
    hashes::{sha256d, Hash},
    secp256k1,
};
use libp2p::PeerId;
use quickcheck::{Arbitrary, Gen};
use std::ops::Deref;

/// Generic newtype that allows us to implement quickcheck::Arbitrary on foreign
/// types
#[derive(Clone, Debug, Copy)]
pub struct Quickcheck<I>(pub I);

impl<I> Deref for Quickcheck<I> {
    type Target = I;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

macro_rules! impl_arbitrary_for_byte_array {
    ([u8; $length:expr]) => {
        impl Arbitrary for Quickcheck<[u8; $length]> {
            fn arbitrary<G: Gen>(g: &mut G) -> Self {
                let mut bytes = [0u8; $length];
                g.fill_bytes(&mut bytes);

                Quickcheck(bytes)
            }
        }
    };
}

impl_arbitrary_for_byte_array!([u8; 16]);
impl_arbitrary_for_byte_array!([u8; 20]);
impl_arbitrary_for_byte_array!([u8; 32]);

impl Arbitrary for Quickcheck<::bitcoin::Network> {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let network = match g.next_u32() % 3 {
            0 => ::bitcoin::Network::Bitcoin,
            1 => ::bitcoin::Network::Testnet,
            2 => ::bitcoin::Network::Regtest,
            _ => unreachable!(),
        };

        Quickcheck(network)
    }
}

impl Arbitrary for Quickcheck<ChainId> {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let chain_id = ChainId::from(g.next_u32());

        Quickcheck(chain_id)
    }
}

impl Arbitrary for Quickcheck<asset::Bitcoin> {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let amount = asset::Bitcoin::from_sat(g.next_u64());

        Quickcheck(amount)
    }
}

impl Arbitrary for Quickcheck<crate::ethereum::U256> {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let bytes = *Quickcheck::<[u8; 32]>::arbitrary(g);
        let u256 = crate::ethereum::U256::from(bytes);

        Quickcheck(u256)
    }
}

impl Arbitrary for Quickcheck<::bitcoin::BlockHash> {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let bytes = *Quickcheck::<[u8; 32]>::arbitrary(g);

        match sha256d::Hash::from_slice(&bytes) {
            Ok(block_id) => Quickcheck(block_id.into()),
            Err(::bitcoin::hashes::Error::InvalidLength(..)) => {
                panic!("we always generate 32 bytes")
            }
        }
    }
}

impl Arbitrary for Quickcheck<crate::asset::Ether> {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let u256 = *Quickcheck::<crate::ethereum::U256>::arbitrary(g);
        let ether_quantity = crate::asset::Ether::from_wei(u256);

        Quickcheck(ether_quantity)
    }
}

impl Arbitrary for Quickcheck<crate::asset::Erc20Quantity> {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let u256 = *Quickcheck::<crate::ethereum::U256>::arbitrary(g);
        let erc20_quantity = crate::asset::Erc20Quantity::from_wei(u256);

        Quickcheck(erc20_quantity)
    }
}

impl Arbitrary for Quickcheck<crate::asset::Erc20> {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let token_contract = *Quickcheck::<identity::Ethereum>::arbitrary(g);
        let quantity = Quickcheck::<crate::asset::Erc20Quantity>::arbitrary(g).0;
        let erc20_token = crate::asset::Erc20 {
            token_contract,
            quantity,
        };

        Quickcheck(erc20_token)
    }
}

impl Arbitrary for Quickcheck<identity::Bitcoin> {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let bytes = *Quickcheck::<[u8; 32]>::arbitrary(g);
        let secret_key =
            secp256k1::SecretKey::from_slice(&bytes).expect("all bytes are a valid secret key");
        let public_key =
            identity::Bitcoin::from_secret_key(&secp256k1::Secp256k1::signing_only(), &secret_key);

        Quickcheck(public_key)
    }
}

impl Arbitrary for Quickcheck<identity::Ethereum> {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let bytes = *Quickcheck::<[u8; 20]>::arbitrary(g);

        Quickcheck(Address::from(bytes))
    }
}

impl Arbitrary for Quickcheck<crate::ethereum::Hash> {
    fn arbitrary<G: quickcheck::Gen>(g: &mut G) -> Self {
        let bytes = *Quickcheck::<[u8; 32]>::arbitrary(g);

        Quickcheck(crate::ethereum::Hash::from(bytes))
    }
}

impl Arbitrary for Quickcheck<transaction::Ethereum> {
    fn arbitrary<G: quickcheck::Gen>(g: &mut G) -> Self {
        Quickcheck(transaction::Ethereum {
            hash: *Quickcheck::<crate::ethereum::Hash>::arbitrary(g),
            to: Option::<Quickcheck<crate::ethereum::Address>>::arbitrary(g).map(|i| i.0),
            value: *Quickcheck::<crate::ethereum::U256>::arbitrary(g),
            input: Bytes(Arbitrary::arbitrary(g)),
        })
    }
}

impl Arbitrary for Quickcheck<Timestamp> {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let timestamp = Timestamp::from(g.next_u32());

        Quickcheck(timestamp)
    }
}

impl Arbitrary for Quickcheck<SecretHash> {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let bytes = *Quickcheck::<[u8; 32]>::arbitrary(g);

        Quickcheck(SecretHash::from(bytes))
    }
}

impl Arbitrary for Quickcheck<ledger::Bitcoin> {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let ledger = match g.next_u32() % 3 {
            0 => ledger::Bitcoin::Mainnet,
            1 => ledger::Bitcoin::Testnet,
            2 => ledger::Bitcoin::Regtest,
            _ => unreachable!(),
        };

        Quickcheck(ledger)
    }
}

impl Arbitrary for Quickcheck<ledger::Ethereum> {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let chain_id = *Quickcheck::<ChainId>::arbitrary(g);
        let ethereum = ledger::Ethereum { chain_id };

        Quickcheck(ethereum)
    }
}

impl Arbitrary for Quickcheck<Role> {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let role = match g.next_u32() % 2 {
            0 => Role::Alice,
            1 => Role::Bob,
            _ => unreachable!(),
        };

        Quickcheck(role)
    }
}

impl Arbitrary for Quickcheck<PeerId> {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let bytes = *Quickcheck::<[u8; 32]>::arbitrary(g);
        let secret_key = libp2p::identity::secp256k1::SecretKey::from_bytes(bytes)
            .expect("any 32 bytes are a valid secret key");
        let keypair = libp2p::identity::secp256k1::Keypair::from(secret_key);
        let public_key = keypair.public().clone();
        let public_key = libp2p::core::PublicKey::Secp256k1(public_key);
        let peer_id = PeerId::from_public_key(public_key);

        Quickcheck(peer_id)
    }
}
