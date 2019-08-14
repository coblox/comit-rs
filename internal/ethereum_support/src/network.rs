use serde::{Deserialize, Serialize};

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Deserialize,
    Serialize,
    Hash,
    strum_macros::IntoStaticStr,
    strum_macros::Display,
)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    #[strum(serialize = "mainnet")]
    Mainnet,
    #[strum(serialize = "regtest")]
    Regtest,
    #[strum(serialize = "ropsten")]
    Ropsten,
    #[strum(serialize = "unknown")]
    Unknown,
}

impl Network {
    pub fn from_network_id(s: String) -> Self {
        match s.as_str() {
            "1" => Network::Mainnet,
            "3" => Network::Ropsten,
            "17" => Network::Regtest,
            _ => Network::Unknown,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fmt::Display;

    #[test]
    fn string_serialize() {
        let mainnet: &'static str = Network::Mainnet.into();
        let regtest: &'static str = Network::Regtest.into();
        let ropsten: &'static str = Network::Ropsten.into();

        assert_eq!(mainnet, "mainnet");
        assert_eq!(regtest, "regtest");
        assert_eq!(ropsten, "ropsten");
    }

    #[test]
    fn from_version() {
        assert_eq!(
            Network::from_network_id(String::from("1")),
            Network::Mainnet
        );
        assert_eq!(
            Network::from_network_id(String::from("3")),
            Network::Ropsten
        );
        assert_eq!(
            Network::from_network_id(String::from("17")),
            Network::Regtest
        );
        assert_eq!(
            Network::from_network_id(String::from("-1")),
            Network::Unknown
        );
    }

    fn assert_display<T: Display>(t: T) {}

    #[test]
    fn test_derives_display() {
        assert_display(Network::Regtest);
    }
}
