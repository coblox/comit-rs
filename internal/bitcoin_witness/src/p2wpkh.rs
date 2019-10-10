use crate::witness::{UnlockParameters, Witness};
use bitcoin_support::{Hash160, PubkeyHash, Script};
use secp_wrapper::SecretKey;

/// Utility function to generate the `prev_script` for a p2wpkh adddress.
/// A standard p2wpkh locking script of:
/// 00 14 <public_key_hash>
/// becomes
/// 19 76 a9 14 <public_key_hash> 88 ac
/// in the unlocking script. See BIP 143.
/// This function simply returns the latter as a Script.
fn generate_prev_script(public_key_hash: PubkeyHash) -> Script {
    let public_key_hash: Hash160 = public_key_hash.into();

    let mut prev_script = vec![0x76, 0xa9, 0x14];

    prev_script.append(&mut public_key_hash[..].to_vec());
    prev_script.push(0x88);
    prev_script.push(0xac);

    Script::from(prev_script)
}

pub trait UnlockP2wpkh {
    fn p2wpkh_unlock_parameters(self) -> UnlockParameters;
}

impl UnlockP2wpkh for SecretKey {
    fn p2wpkh_unlock_parameters(self) -> UnlockParameters {
        UnlockParameters {
            witness: vec![
                Witness::Signature(self.clone()),
                Witness::PublicKey(self.clone().public_key()),
            ],
            sequence: super::SEQUENCE_ALLOW_NTIMELOCK_NO_RBF,
            locktime: 0,
            prev_script: generate_prev_script(self.public_key().into()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bitcoin_support::PrivateKey;
    use secp_wrapper::{
        secp256k1::{self, Secp256k1},
        Builder,
    };
    use std::str::FromStr;

    #[test]
    fn correct_prev_script() {
        let secp: Secp256k1<secp256k1::All> = Secp256k1::new();
        let private_key =
            PrivateKey::from_str("L4r4Zn5sy3o5mjiAdezhThkU37mcdN4eGp4aeVM4ZpotGTcnWc6k").unwrap();

        let secret_key = Builder::new(secp)
            .secret_key(private_key.key)
            .build()
            .unwrap();
        let input_parameters = secret_key.p2wpkh_unlock_parameters();
        // Note: You might expect it to be a is_p2wpkh() but it shouldn't be.
        assert!(
            input_parameters.prev_script.is_p2pkh(),
            "prev_script should be a p2pkh"
        );
    }
}
