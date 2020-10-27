mod db;
mod http_api;
mod seed;

use crate::{
    asset, hbit, herc20, identity, spawn, storage::db::queries::get_swap_context_by_id,
    LocalSwapId, Role, Side,
};
use async_trait::async_trait;
use std::sync::Arc;

pub use db::*;
pub use seed::*;

/// Load data for a particular swap from the storage layer.
#[async_trait]
pub trait Load<T>: Send + Sync + 'static {
    async fn load(&self, swap_id: LocalSwapId) -> anyhow::Result<T>;
}

/// A facade for the storage layer.
#[derive(Debug, Clone)]
pub struct Storage {
    pub db: Sqlite,
    pub seed: RootSeed,
    pub herc20_states: Arc<herc20::States>,
    pub hbit_states: Arc<hbit::States>,
}

impl Storage {
    pub fn new(db: Sqlite, seed: RootSeed) -> Self {
        Self {
            db,
            seed,
            herc20_states: Arc::new(herc20::States::default()),
            hbit_states: Arc::new(hbit::States::default()),
        }
    }

    /// Transient identity used by the hbit HTLC.
    pub fn derive_transient_identity(
        &self,
        swap_id: LocalSwapId,
        role: Role,
        hbit_side: Side,
    ) -> identity::Bitcoin {
        let swap_seed = self.seed.derive_swap_seed(swap_id);
        let sk = match (role, hbit_side) {
            (Role::Alice, Side::Alpha) | (Role::Bob, Side::Beta) => {
                swap_seed.derive_transient_refund_identity()
            }
            (Role::Alice, Side::Beta) | (Role::Bob, Side::Alpha) => {
                swap_seed.derive_transient_redeem_identity()
            }
        };

        identity::Bitcoin::from_secret_key(&*crate::SECP, &sk)
    }
}

#[cfg(test)]
#[allow(dead_code)]
impl Storage {
    pub fn test() -> Self {
        Self::new(
            Sqlite::test(),
            RootSeed::new_random(&mut rand::thread_rng()).unwrap(),
        )
    }
}

#[async_trait::async_trait]
impl<A, B, TParamsA, TParamsB> Load<spawn::Swap<TParamsA, TParamsB>> for Storage
where
    Sqlite: LoadTables<A, B>,
    TParamsA: IntoParams<ProtocolTable = A> + 'static,
    TParamsB: IntoParams<ProtocolTable = B> + 'static,
    A: 'static,
    B: 'static,
{
    async fn load(&self, id: LocalSwapId) -> anyhow::Result<spawn::Swap<TParamsA, TParamsB>> {
        let tab = self.db.load_tables(id).await?;
        let role = tab.swap.role;
        let secret_hash = derive_or_unwrap_secret_hash(id, self.seed, role, tab.secret_hash)?;

        let alpha = TParamsA::into_params(tab.alpha, id, self.seed, role, secret_hash)?;
        let beta = TParamsB::into_params(tab.beta, id, self.seed, role, secret_hash)?;

        Ok(spawn::Swap {
            role,
            alpha,
            beta,
            start_of_swap: tab.swap.start_of_swap,
        })
    }
}

/// Convert a protocol table, with associated data, into a swap params object.
pub trait IntoParams: Sized {
    type ProtocolTable;

    fn into_params(
        _: Self::ProtocolTable,
        _: LocalSwapId,
        _: RootSeed,
        _: Role,
        _: comit::SecretHash,
    ) -> anyhow::Result<Self>;
}

impl IntoParams for herc20::Params {
    type ProtocolTable = Herc20;

    fn into_params(
        herc20: Self::ProtocolTable,
        id: LocalSwapId,
        _: RootSeed,
        _: Role,
        secret_hash: comit::SecretHash,
    ) -> anyhow::Result<herc20::Params> {
        Ok(herc20::Params {
            asset: asset::Erc20 {
                quantity: herc20.amount.0.into(),
                token_contract: herc20.token_contract.0,
            },
            redeem_identity: herc20
                .redeem_identity
                .ok_or_else(|| NoHerc20RedeemIdentity(id))?
                .0,
            refund_identity: herc20
                .refund_identity
                .ok_or_else(|| NoHerc20RefundIdentity(id))?
                .0,
            expiry: herc20.expiry.0.into(),
            secret_hash,
            chain_id: herc20.chain_id.0.into(),
        })
    }
}

impl IntoParams for hbit::Params {
    type ProtocolTable = Hbit;

    fn into_params(
        hbit: Self::ProtocolTable,
        id: LocalSwapId,
        seed: RootSeed,
        role: Role,
        secret_hash: comit::SecretHash,
    ) -> anyhow::Result<hbit::Params> {
        let (redeem, refund) = match (hbit.side.0, role) {
            (Side::Alpha, Role::Bob) | (Side::Beta, Role::Alice) => {
                let redeem = identity::Bitcoin::from_secret_key(
                    &*crate::SECP,
                    &seed.derive_swap_seed(id).derive_transient_redeem_identity(),
                );
                let refund = hbit.transient_identity.ok_or(NoHbitRefundIdentity(id))?.0;

                (redeem, refund)
            }
            (Side::Alpha, Role::Alice) | (Side::Beta, Role::Bob) => {
                let redeem = hbit.transient_identity.ok_or(NoHbitRedeemIdentity(id))?.0;
                let refund = identity::Bitcoin::from_secret_key(
                    &*crate::SECP,
                    &seed.derive_swap_seed(id).derive_transient_refund_identity(),
                );

                (redeem, refund)
            }
        };

        Ok(hbit::Params {
            network: hbit.network.0,
            asset: hbit.amount.0.into(),
            redeem_identity: redeem,
            refund_identity: refund,
            expiry: hbit.expiry.0.into(),
            secret_hash,
        })
    }
}

#[async_trait::async_trait]
impl Load<SwapContext> for Storage {
    async fn load(&self, swap_id: LocalSwapId) -> anyhow::Result<SwapContext> {
        let context = self
            .db
            .do_in_transaction(|connection| get_swap_context_by_id(connection, swap_id))
            .await?;

        Ok(context)
    }
}

// Whether or not we get the secret hash from the db or derive it is
// based on which role we are.
fn derive_or_unwrap_secret_hash(
    id: LocalSwapId,
    seed: RootSeed,
    role: Role,
    secret_hash: Option<SecretHash>,
) -> anyhow::Result<comit::SecretHash> {
    let secret_hash = match role {
        Role::Alice => {
            let swap_seed = seed.derive_swap_seed(id);
            comit::SecretHash::new(swap_seed.derive_secret())
        }
        Role::Bob => secret_hash.ok_or_else(|| NoSecretHash(id))?.secret_hash.0,
    };
    Ok(secret_hash)
}

#[derive(thiserror::Error, Debug, Clone, Copy)]
#[error("could not derive Bitcoin identity for swap not involving hbit: {0}")]
pub struct HbitNotInvolved(pub LocalSwapId);
