use crate::{
    db::{
        schema::{address_hints, halights, herc20s, secret_hashes, shared_swap_ids, swaps},
        wrapper_types::{
            custom_sql_types::{Text, U32},
            BitcoinNetwork, Erc20Amount, EthereumAddress, Satoshis,
        },
        Error, Sqlite,
    },
    identity, lightning,
    swap_protocols::{self, rfc003, HashFunction, LocalSwapId, Role},
};
use diesel::{self, prelude::*, RunQueryDsl};
use libp2p::{Multiaddr, PeerId};

#[derive(Identifiable, Queryable, PartialEq, Debug)]
#[table_name = "swaps"]
pub struct Swap {
    id: i32,
    local_swap_id: Text<LocalSwapId>,
    role: Text<Role>,
    counterparty_peer_id: Text<PeerId>,
}

impl From<Swap> for ISwap {
    fn from(swap: Swap) -> Self {
        ISwap {
            local_swap_id: swap.local_swap_id,
            role: swap.role,
            counterparty_peer_id: swap.counterparty_peer_id,
        }
    }
}

#[derive(Insertable, Debug, Clone)]
#[table_name = "swaps"]
pub struct ISwap {
    local_swap_id: Text<LocalSwapId>,
    role: Text<Role>,
    counterparty_peer_id: Text<PeerId>,
}

impl ISwap {
    pub fn new(swap_id: LocalSwapId, counterparty: PeerId, role: Role) -> Self {
        ISwap {
            local_swap_id: Text(swap_id),
            role: Text(role),
            counterparty_peer_id: Text(counterparty),
        }
    }
}

#[derive(Associations, Clone, Copy, Debug, Identifiable, Queryable, PartialEq)]
#[belongs_to(Swap)]
#[table_name = "secret_hashes"]
pub struct SecretHash {
    id: i32,
    swap_id: i32,
    secret_hash: Text<rfc003::SecretHash>,
}

#[derive(Insertable, Debug, Clone, Copy)]
#[table_name = "secret_hashes"]
pub struct ISecretHash {
    swap_id: i32,
    secret_hash: Text<rfc003::SecretHash>,
}

#[derive(Clone, Debug, Identifiable, Queryable, PartialEq)]
#[table_name = "address_hints"]
pub struct AddressHint {
    id: i32,
    peer_id: Text<PeerId>,
    address_hint: Text<Multiaddr>,
}

#[derive(Insertable, Debug, Clone)]
#[table_name = "address_hints"]
pub struct IAddressHint {
    peer_id: Text<PeerId>,
    address_hint: Text<Multiaddr>,
}

#[derive(Associations, Clone, Copy, Debug, Identifiable, Queryable, PartialEq)]
#[belongs_to(Swap)]
#[table_name = "shared_swap_ids"]
pub struct SharedSwapId {
    id: i32,
    swap_id: i32,
    shared_swap_id: Text<swap_protocols::SharedSwapId>,
}

#[derive(Insertable, Debug, Clone, Copy)]
#[table_name = "shared_swap_ids"]
pub struct ISharedSwapId {
    swap_id: i32,
    shared_swap_id: Text<swap_protocols::SharedSwapId>,
}

#[derive(Associations, Clone, Debug, Identifiable, Queryable, PartialEq)]
#[belongs_to(Swap)]
#[table_name = "herc20s"]
pub struct Herc20 {
    id: i32,
    swap_id: i32,
    amount: Text<Erc20Amount>,
    chain_id: U32,
    expiry: U32,
    hash_function: Text<HashFunction>,
    token_contract: Text<EthereumAddress>,
    redeem_identity: Option<Text<EthereumAddress>>,
    refund_identity: Option<Text<EthereumAddress>>,
    ledger: String,
}

#[derive(Insertable, Debug, Clone)]
#[table_name = "herc20s"]
pub struct IHerc20 {
    pub swap_id: i32,
    pub amount: Text<Erc20Amount>,
    pub chain_id: U32,
    pub expiry: U32,
    pub hash_function: Text<HashFunction>,
    pub token_contract: Text<EthereumAddress>,
    pub redeem_identity: Option<Text<EthereumAddress>>,
    pub refund_identity: Option<Text<EthereumAddress>>,
    pub ledger: String,
}

impl IHerc20 {
    pub fn with_swap_id(&self, swap_id: i32) -> Self {
        IHerc20 {
            swap_id,
            amount: self.amount.clone(),
            chain_id: self.chain_id,
            expiry: self.expiry,
            hash_function: self.hash_function,
            token_contract: self.token_contract,
            redeem_identity: self.redeem_identity,
            refund_identity: self.refund_identity,
            ledger: self.ledger.clone(),
        }
    }
}

#[derive(Associations, Clone, Debug, Identifiable, Queryable, PartialEq)]
#[belongs_to(Swap)]
#[table_name = "halights"]
pub struct Halight {
    id: i32,
    swap_id: i32,
    amount: Text<Satoshis>,
    network: Text<BitcoinNetwork>,
    chain: String,
    cltv_expiry: U32,
    hash_function: Text<HashFunction>,
    redeem_identity: Option<Text<lightning::PublicKey>>,
    refund_identity: Option<Text<lightning::PublicKey>>,
    ledger: String,
}

#[derive(Insertable, Debug, Clone)]
#[table_name = "halights"]
pub struct IHalight {
    pub swap_id: i32,
    pub amount: Text<Satoshis>,
    pub network: Text<BitcoinNetwork>,
    pub chain: String,
    pub cltv_expiry: U32,
    pub hash_function: Text<HashFunction>,
    pub redeem_identity: Option<Text<lightning::PublicKey>>,
    pub refund_identity: Option<Text<lightning::PublicKey>>,
    pub ledger: String,
}

impl IHalight {
    pub fn with_swap_id(&self, swap_id: i32) -> Self {
        IHalight {
            swap_id,
            amount: self.amount,
            network: self.network,
            chain: self.chain.clone(),
            cltv_expiry: self.cltv_expiry,
            hash_function: self.hash_function,
            redeem_identity: self.redeem_identity,
            refund_identity: self.refund_identity,
            ledger: self.ledger.clone(),
        }
    }
}

impl Sqlite {
    pub async fn role(&self, swap_id: LocalSwapId) -> anyhow::Result<Role> {
        let swap = self.load_swap(swap_id).await?;
        Ok(swap.role.0)
    }

    pub async fn save_swap(&self, insertable: &ISwap) -> anyhow::Result<()> {
        self.do_in_transaction(|connection| {
            diesel::insert_into(swaps::dsl::swaps)
                .values(insertable)
                .execute(&*connection)
        })
        .await?;

        Ok(())
    }

    pub async fn load_swap(&self, swap_id: LocalSwapId) -> anyhow::Result<Swap> {
        let record: Swap = self
            .do_in_transaction(|connection| {
                let key = Text(swap_id);

                swaps::table
                    .filter(swaps::local_swap_id.eq(key))
                    .first(connection)
                    .optional()
            })
            .await?
            .ok_or(Error::SwapNotFound)?;

        Ok(record)
    }

    pub async fn save_secret_hash(
        &self,
        swap_id: LocalSwapId,
        secret_hash: rfc003::SecretHash,
    ) -> anyhow::Result<()> {
        self.do_in_transaction(|connection| {
            let key = Text(swap_id);

            let swap: Swap = swaps::table
                .filter(swaps::local_swap_id.eq(key))
                .first(connection)?;

            let insertable = ISecretHash {
                swap_id: swap.id,
                secret_hash: Text(secret_hash),
            };

            diesel::insert_into(secret_hashes::dsl::secret_hashes)
                .values(insertable)
                .execute(&*connection)
        })
        .await?;

        Ok(())
    }

    pub async fn load_secret_hash(
        &self,
        swap_id: LocalSwapId,
    ) -> anyhow::Result<rfc003::SecretHash> {
        let record: SecretHash = self
            .do_in_transaction(|connection| {
                let key = Text(swap_id);

                let swap: Swap = swaps::table
                    .filter(swaps::local_swap_id.eq(key))
                    .first(connection)?;

                SecretHash::belonging_to(&swap).first(connection).optional()
            })
            .await?
            .ok_or(Error::SwapNotFound)?;

        Ok(record.secret_hash.0)
    }

    pub async fn save_shared_swap_id(
        &self,
        swap_id: LocalSwapId,
        shared_swap_id: swap_protocols::SharedSwapId,
    ) -> anyhow::Result<()> {
        self.do_in_transaction(|connection| {
            let key = Text(swap_id);

            let swap: Swap = swaps::table
                .filter(swaps::local_swap_id.eq(key))
                .first(connection)?;

            let insertable = ISharedSwapId {
                swap_id: swap.id,
                shared_swap_id: Text(shared_swap_id),
            };

            diesel::insert_into(shared_swap_ids::dsl::shared_swap_ids)
                .values(insertable)
                .execute(&*connection)
        })
        .await?;

        Ok(())
    }

    pub async fn load_shared_swap_id(
        &self,
        swap_id: LocalSwapId,
    ) -> anyhow::Result<swap_protocols::SharedSwapId> {
        let record: SharedSwapId = self
            .do_in_transaction(|connection| {
                let key = Text(swap_id);

                let swap: Swap = swaps::table
                    .filter(swaps::local_swap_id.eq(key))
                    .first(connection)?;

                SharedSwapId::belonging_to(&swap)
                    .first(connection)
                    .optional()
            })
            .await?
            .ok_or(Error::SwapNotFound)?;

        Ok(record.shared_swap_id.0)
    }

    /// This function is called depending on ones role and which side of the
    /// swap halight is on.
    /// - Called by Alice when halight is the alpha protocol.
    /// - Called by Bob when when halight is the beta protocol.
    pub async fn save_counterparty_halight_redeem_identity(
        &self,
        swap_id: LocalSwapId,
        identity: identity::Lightning,
    ) -> anyhow::Result<()> {
        use crate::db::schema::halights::columns::redeem_identity;

        self.do_in_transaction(|connection| {
            let key = Text(swap_id);

            let swap: Swap = swaps::table
                .filter(swaps::local_swap_id.eq(key))
                .first(connection)?;

            diesel::update(halights::dsl::halights.filter(halights::swap_id.eq(swap.id)))
                .set(redeem_identity.eq(Text(identity)))
                .execute(&*connection)
        })
        .await?;

        Ok(())
    }

    /// This function is called depending on ones role and which side of the
    /// swap halight is on.
    /// - Called by Alice when halight is the beta protocol.
    /// - Called by Bob when when halight is the alpha protocol.
    pub async fn save_counterparty_halight_refund_identity(
        &self,
        swap_id: LocalSwapId,
        identity: identity::Lightning,
    ) -> anyhow::Result<()> {
        use crate::db::schema::halights::columns::refund_identity;

        self.do_in_transaction(|connection| {
            let key = Text(swap_id);

            let swap: Swap = swaps::table
                .filter(swaps::local_swap_id.eq(key))
                .first(connection)?;

            diesel::update(halights::dsl::halights.filter(halights::swap_id.eq(swap.id)))
                .set(refund_identity.eq(Text(identity)))
                .execute(&*connection)
        })
        .await?;

        Ok(())
    }

    /// This function is called depending on ones role and which side of the
    /// swap herc20 is on.
    /// - Called by Alice when herc20 is the alpha protocol.
    /// - Called by Bob when when herc20 is the beta protocol.
    pub async fn save_counterparty_herc20_redeem_identity(
        &self,
        _swap_id_: LocalSwapId,
        _identity: identity::Ethereum,
    ) -> anyhow::Result<()> {
        unimplemented!()
    }

    /// This function is called depending on ones role and which side of the
    /// swap herc20 is on.
    /// - Called by Alice when herc20 is the beta protocol.
    /// - Called by Bob when when herc20 is the alpha protocol.
    pub async fn save_counterparty_herc20_refund_identity(
        &self,
        _swap_id: LocalSwapId,
        _identity: identity::Ethereum,
    ) -> anyhow::Result<()> {
        unimplemented!()
    }

    pub async fn save_address_hint(
        &self,
        peer_id: PeerId,
        address_hint: &libp2p::Multiaddr,
    ) -> anyhow::Result<()> {
        self.do_in_transaction(|connection| {
            let insertable = IAddressHint {
                peer_id: Text(peer_id.clone()),
                address_hint: Text(address_hint.clone()),
            };

            diesel::insert_into(address_hints::dsl::address_hints)
                .values(insertable)
                .execute(&*connection)
        })
        .await?;

        Ok(())
    }

    pub async fn load_address_hint(&self, peer_id: &PeerId) -> anyhow::Result<libp2p::Multiaddr> {
        let record: AddressHint = self
            .do_in_transaction(|connection| {
                let key = Text(peer_id);

                address_hints::table
                    .filter(address_hints::peer_id.eq(key))
                    .first(connection)
                    .optional()
            })
            .await?
            .ok_or(Error::PeerIdNotFound)?;

        Ok(record.address_hint.0)
    }

    pub(crate) async fn save_herc20_swap_detail(
        &self,
        swap_id: LocalSwapId,
        data: &IHerc20,
    ) -> anyhow::Result<()> {
        self.do_in_transaction(|connection| {
            let key = Text(swap_id);

            let swap: Swap = swaps::table
                .filter(swaps::local_swap_id.eq(key))
                .first(connection)?;

            let insertable = data.with_swap_id(swap.id);

            diesel::insert_into(herc20s::dsl::herc20s)
                .values(insertable)
                .execute(&*connection)
        })
        .await?;

        Ok(())
    }

    pub async fn load_herc20_swap_detail(&self, swap_id: LocalSwapId) -> anyhow::Result<Herc20> {
        let record: Herc20 = self
            .do_in_transaction(|connection| {
                let key = Text(swap_id);

                let swap: Swap = swaps::table
                    .filter(swaps::local_swap_id.eq(key))
                    .first(connection)?;

                Herc20::belonging_to(&swap).first(connection).optional()
            })
            .await?
            .ok_or(Error::SwapNotFound)?;

        Ok(record)
    }

    pub async fn save_halight_swap_detail(
        &self,
        swap_id: LocalSwapId,
        data: &IHalight,
    ) -> anyhow::Result<()> {
        self.do_in_transaction(|connection| {
            let key = Text(swap_id);

            let swap: Swap = swaps::table
                .filter(swaps::local_swap_id.eq(key))
                .first(connection)?;

            let insertable = data.with_swap_id(swap.id);

            diesel::insert_into(halights::dsl::halights)
                .values(insertable)
                .execute(&*connection)
        })
        .await?;

        Ok(())
    }

    pub async fn load_halight_swap_detail(&self, swap_id: LocalSwapId) -> anyhow::Result<Halight> {
        let record: Halight = self
            .do_in_transaction(|connection| {
                let key = Text(swap_id);

                let swap: Swap = swaps::table
                    .filter(swaps::local_swap_id.eq(key))
                    .first(connection)?;

                Halight::belonging_to(&swap).first(connection).optional()
            })
            .await?
            .ok_or(Error::SwapNotFound)?;

        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lightning;
    use std::{path::PathBuf, str::FromStr};

    fn temp_db() -> PathBuf {
        let temp_file = tempfile::Builder::new()
            .suffix(".sqlite")
            .tempfile()
            .unwrap();

        temp_file.into_temp_path().to_path_buf()
    }

    fn insertable_swap() -> ISwap {
        let swap_id =
            LocalSwapId::from_str("ad2652ca-ecf2-4cc6-b35c-b4351ac28a34").expect("valid swap id");
        let role = Role::Alice;
        let peer_id = PeerId::from_str("QmfUfpC2frwFvcDzpspnfZitHt5wct6n4kpG5jzgRdsxkY")
            .expect("valid peer id");

        ISwap {
            local_swap_id: Text(swap_id),
            role: Text(role),
            counterparty_peer_id: Text(peer_id),
        }
    }

    impl PartialEq<ISwap> for Swap {
        fn eq(&self, other: &ISwap) -> bool {
            self.local_swap_id == other.local_swap_id
                && self.role == other.role
                && self.counterparty_peer_id == other.counterparty_peer_id
        }
    }

    impl PartialEq<IHerc20> for Herc20 {
        fn eq(&self, other: &IHerc20) -> bool {
            self.amount == other.amount
                && self.chain_id == other.chain_id
                && self.expiry == other.expiry
                && self.hash_function == other.hash_function
                && self.token_contract == other.token_contract
                && self.redeem_identity == other.redeem_identity
                && self.refund_identity == other.refund_identity
                && self.ledger == other.ledger
        }
    }

    impl PartialEq<IHalight> for Halight {
        fn eq(&self, other: &IHalight) -> bool {
            self.amount == other.amount
                && self.network == other.network
                && self.chain == other.chain
                && self.cltv_expiry == other.cltv_expiry
                && self.hash_function == other.hash_function
                && self.redeem_identity == other.redeem_identity
                && self.refund_identity == other.refund_identity
                && self.ledger == other.ledger
        }
    }

    #[tokio::test]
    async fn roundtrip_swap() {
        let path = temp_db();
        let db = Sqlite::new(&path).expect("a new db");

        let given = insertable_swap();
        let swap_id = given.local_swap_id.0;

        db.save_swap(&given)
            .await
            .expect("to be able to save a swap");

        let loaded = db
            .load_swap(swap_id)
            .await
            .expect("to be able to load a previously saved swap");

        assert_eq!(loaded, given)
    }

    #[tokio::test]
    async fn roundtrip_secret_hash() {
        let path = temp_db();
        let db = Sqlite::new(&path).expect("a new db");

        let swap = insertable_swap();
        let swap_id = swap.local_swap_id.0;

        db.save_swap(&swap)
            .await
            .expect("to be able to save a swap");

        let secret_hash = rfc003::SecretHash::from_str(
            "bfbfbfbfbfbfbfbfbfbfbfbfbfbfbfbf\
             bfbfbfbfbfbfbfbfbfbfbfbfbfbfbfbf",
        )
        .expect("valid secret hash");

        db.save_secret_hash(swap_id, secret_hash)
            .await
            .expect("to be able to save secret hash");

        let loaded = db
            .load_secret_hash(swap_id)
            .await
            .expect("to be able to load a previously saved secret hash");

        assert_eq!(loaded, secret_hash)
    }

    #[tokio::test]
    async fn roundtrip_shared_swap_id() {
        let path = temp_db();
        let db = Sqlite::new(&path).expect("a new db");

        let swap = insertable_swap();
        let swap_id = swap.local_swap_id.0;

        db.save_swap(&swap)
            .await
            .expect("to be able to save a swap");

        let shared_swap_id =
            swap_protocols::SharedSwapId::from_str("ad9999ca-ecf2-4cc6-b35c-b4351ac28a34")
                .expect("valid swap id");

        db.save_shared_swap_id(swap_id, shared_swap_id)
            .await
            .expect("to be able to save swap id");

        let loaded = db
            .load_shared_swap_id(swap_id)
            .await
            .expect("to be able to load a previously saved swap id");

        assert_eq!(loaded, shared_swap_id)
    }

    #[tokio::test]
    async fn roundtrip_address_hint() {
        let path = temp_db();
        let db = Sqlite::new(&path).expect("a new db");

        let swap = insertable_swap();

        db.save_swap(&swap)
            .await
            .expect("to be able to save a swap");

        let peer_id = PeerId::from_str("QmfUfpC2frwFvcDzpspnfZitHt5wct6n4kpG5jzgRdsxkY")
            .expect("valid peer id");
        let multi_addr = "/ip4/80.123.90.4/tcp/5432";
        let address_hint: Multiaddr = multi_addr.parse().expect("valid multiaddress");

        db.save_address_hint(peer_id.clone(), &address_hint)
            .await
            .expect("to be able to save address hint");

        let loaded = db
            .load_address_hint(&peer_id)
            .await
            .expect("to be able to load a previously saved address hint");

        assert_eq!(loaded, address_hint)
    }

    #[tokio::test]
    async fn roundtrip_herc20s() {
        let path = temp_db();
        let db = Sqlite::new(&path).expect("a new db");

        let swap = insertable_swap();
        let swap_id = swap.local_swap_id.0;

        db.save_swap(&swap)
            .await
            .expect("to be able to save a swap");

        let amount = Erc20Amount::from_str("12345").expect("valid ERC20 amount");
        let ethereum_identity =
            EthereumAddress::from_str("1111e8be41b21f651a71aaB1A85c6813b8bBcCf8")
                .expect("valid etherum identity");
        let redeem_identity = EthereumAddress::from_str("2222e8be41b21f651a71aaB1A85c6813b8bBcCf8")
            .expect("valid redeem identity");
        let refund_identity = EthereumAddress::from_str("3333e8be41b21f651a71aaB1A85c6813b8bBcCf8")
            .expect("valid refund identity");

        let given = IHerc20 {
            swap_id: 0, // This is set when saving.
            amount: Text(amount),
            chain_id: U32(1337),
            expiry: U32(123),
            hash_function: Text(HashFunction::Sha256),
            token_contract: Text(ethereum_identity),
            redeem_identity: Some(Text(redeem_identity)),
            refund_identity: Some(Text(refund_identity)),
            ledger: "alpha".to_string(),
        };

        db.save_herc20_swap_detail(swap_id, &given)
            .await
            .expect("to be able to save swap details");

        let loaded = db
            .load_herc20_swap_detail(swap_id)
            .await
            .expect("to be able to load a previously saved swap details");

        assert_eq!(loaded, given)
    }

    #[tokio::test]
    async fn roundtrip_halights() {
        let path = temp_db();
        let db = Sqlite::new(&path).expect("a new db");

        let swap = insertable_swap();
        let swap_id = swap.local_swap_id.0;

        db.save_swap(&swap)
            .await
            .expect("to be able to save a swap");

        let amount = Satoshis::from_str("12345").expect("valid ERC20 amount");

        let redeem_identity = lightning::PublicKey::random();
        let refund_identity = lightning::PublicKey::random();

        let given = IHalight {
            swap_id: 0, // This is set when saving.
            amount: Text(amount),
            network: Text(BitcoinNetwork::Testnet),
            chain: "bitcoin".to_string(),
            cltv_expiry: U32(456),
            hash_function: Text(HashFunction::Sha256),
            redeem_identity: Some(Text(redeem_identity)),
            refund_identity: Some(Text(refund_identity)),
            ledger: "beta".to_string(),
        };

        db.save_halight_swap_detail(swap_id, &given)
            .await
            .expect("to be able to save swap details");

        let loaded = db
            .load_halight_swap_detail(swap_id)
            .await
            .expect("to be able to load a previously saved swap details");

        assert_eq!(loaded, given)
    }
}
