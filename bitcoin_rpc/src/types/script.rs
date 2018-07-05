use types::*;

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct RedeemScript(String);

from_str!(RedeemScript);

// TODO: Maybe we can get rid of this with a custom (de)serializer that decodes the hex string into the ScriptPubKey struct. Let's leave it like this for now so we don't have a primitive there
#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct EncodedScriptPubKey(String);

from_str!(EncodedScriptPubKey);

impl AsRef<[u8]> for EncodedScriptPubKey {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct ScriptPubKey {
    pub asm: String,
    pub hex: EncodedScriptPubKey,
    #[serde(rename = "reqSigs")]
    pub req_sigs: Option<u32>,
    #[serde(rename = "type")]
    pub script_type: ScriptType,
    pub addresses: Option<Vec<RpcAddress>>,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub enum ScriptType {
    #[serde(rename = "pubkey")]
    PubKey,
    #[serde(rename = "pubkeyhash")]
    PubKeyHash,
    #[serde(rename = "multisig")]
    MultiSig,
    #[serde(rename = "nonstandard")]
    NonStandard,
    #[serde(rename = "witness_v0_keyhash")]
    WitnessPubKeyHash,
    /// Appears for generated transactions
    #[serde(rename = "nulldata")]
    NullData,
    #[serde(rename = "witness_v0_scripthash")]
    WitnessScriptHash,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct DecodedScript {
    asm: String,
    #[serde(rename = "type")]
    script_type: Option<ScriptType>,
    #[serde(rename = "reqSigs")]
    req_sigs: Option<u32>,
    addresses: Option<Vec<RpcAddress>>,
    p2sh: RpcAddress,
}

#[cfg(test)]
mod tests {

    use super::*;
    use serde_json;
    use std::str::FromStr;

    #[test]
    fn can_deserialize_decoded_script_type() {
        let json = r#"
        {
            "asm" : "2 03ede722780d27b05f0b1169efc90fa15a601a32fc6c3295114500c586831b6aaf 02ecd2d250a76d204011de6bc365a56033b9b3a149f679bc17205555d3c2b2854f 022d609d2f0d359e5bc0e5d0ea20ff9f5d3396cb5b1906aa9c56a0e7b5edc0c5d5 3 OP_CHECKMULTISIG",
            "reqSigs" : 2,
            "type" : "multisig",
            "addresses" : [
                "mjbLRSidW1MY8oubvs4SMEnHNFXxCcoehQ",
                "mo1vzGwCzWqteip29vGWWW6MsEBREuzW94",
                "mt17cV37fBqZsnMmrHnGCm9pM28R1kQdMG"
            ],
            "p2sh" : "2MyVxxgNBk5zHRPRY2iVjGRJHYZEp1pMCSq"
        }"#;

        let script: DecodedScript = serde_json::from_str(json).unwrap();

        assert_eq!(script, DecodedScript {
            asm: "2 03ede722780d27b05f0b1169efc90fa15a601a32fc6c3295114500c586831b6aaf 02ecd2d250a76d204011de6bc365a56033b9b3a149f679bc17205555d3c2b2854f 022d609d2f0d359e5bc0e5d0ea20ff9f5d3396cb5b1906aa9c56a0e7b5edc0c5d5 3 OP_CHECKMULTISIG".to_string(),
            script_type: Some(ScriptType::MultiSig),
            req_sigs: Some(2),
            addresses: Some(vec![
                RpcAddress::from_str("mjbLRSidW1MY8oubvs4SMEnHNFXxCcoehQ").unwrap(),
                RpcAddress::from_str("mo1vzGwCzWqteip29vGWWW6MsEBREuzW94").unwrap(),
                RpcAddress::from_str("mt17cV37fBqZsnMmrHnGCm9pM28R1kQdMG").unwrap(),
            ]),
            p2sh: RpcAddress::from_str("2MyVxxgNBk5zHRPRY2iVjGRJHYZEp1pMCSq").unwrap(),
        })
    }

}
