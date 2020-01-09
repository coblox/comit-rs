use crate::config::{Bitcoin, Data, Ethereum, Network, Socket};
use config as config_rs;
use log::LevelFilter;
use std::{ffi::OsStr, path::Path};

/// This struct aims to represent the configuration file as it appears on disk.
///
/// Most importantly, optional elements of the configuration file are
/// represented as `Option`s` here. This allows us to create a dedicated step
/// for filling in default values for absent configuration options.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct File {
    pub network: Option<Network>,
    pub http_api: Option<HttpApi>,
    pub data: Option<Data>,
    pub logging: Option<Logging>,
    pub bitcoin: Option<Bitcoin>,
    pub ethereum: Option<Ethereum>,
}

impl File {
    pub fn default() -> Self {
        File {
            network: Option::None,
            http_api: Option::None,
            data: Option::None,
            logging: Option::None,
            bitcoin: Option::None,
            ethereum: Option::None,
        }
    }

    pub fn read<D: AsRef<OsStr>>(config_file: D) -> Result<Self, config_rs::ConfigError> {
        let config_file = Path::new(&config_file);

        let mut config = config_rs::Config::new();
        config.merge(config_rs::File::from(config_file))?;
        config.try_into()
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Logging {
    pub level: Option<LevelFilter>,
    pub structured: Option<bool>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct HttpApi {
    pub socket: Socket,
    pub cors: Option<Cors>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Cors {
    pub allowed_origins: AllowedOrigins,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(untagged)]
pub enum AllowedOrigins {
    All(All),
    None(None),
    Some(Vec<String>),
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum All {
    All,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum None {
    None,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;
    use log::LevelFilter;
    use spectral::prelude::*;
    use std::{
        net::{IpAddr, Ipv4Addr},
        path::PathBuf,
    };

    #[derive(serde::Deserialize, PartialEq, Debug)]
    struct LoggingOnlyConfig {
        logging: Logging,
    }

    #[test]
    fn structured_logging_flag_in_logging_section_is_optional() {
        let file_contents = r#"
        [logging]
        level = "DEBUG"
        "#;

        let config_file = toml::from_str(file_contents);

        assert_that(&config_file).is_ok_containing(LoggingOnlyConfig {
            logging: Logging {
                level: Option::Some(LevelFilter::Debug),
                structured: Option::None,
            },
        });
    }

    #[test]
    fn cors_deserializes_correctly() {
        let file_contents = vec![
            r#"
            allowed_origins = "all"
            "#,
            r#"
             allowed_origins = "none"
            "#,
            r#"
             allowed_origins = ["http://localhost:8000", "https://192.168.1.55:3000"]
            "#,
        ];

        let expected = vec![
            Cors {
                allowed_origins: AllowedOrigins::All(All::All),
            },
            Cors {
                allowed_origins: AllowedOrigins::None(None::None),
            },
            Cors {
                allowed_origins: AllowedOrigins::Some(vec![
                    String::from("http://localhost:8000"),
                    String::from("https://192.168.1.55:3000"),
                ]),
            },
        ];

        let actual = file_contents
            .into_iter()
            .map(toml::from_str)
            .collect::<Result<Vec<Cors>, toml::de::Error>>()
            .unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn full_config_deserializes_correctly() {
        let contents = r#"
[network]
listen = ["/ip4/0.0.0.0/tcp/9939"]

[http_api.socket]
address = "127.0.0.1"
port = 8000

[http_api.cors]
allowed_origins = "all"

[data]
dir = "/tmp/comit/"

[logging]
level = "DEBUG"
structured = false

[bitcoin]
network = "mainnet"
node_url = "http://example.com/"

[ethereum]
node_url = "http://example.com/"
"#;

        let file = File {
            network: Some(Network {
                listen: vec!["/ip4/0.0.0.0/tcp/9939".parse().unwrap()],
            }),
            http_api: Some(HttpApi {
                socket: Socket {
                    address: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                    port: 8000,
                },
                cors: Some(Cors {
                    allowed_origins: AllowedOrigins::All(All::All),
                }),
            }),
            data: Some(Data {
                dir: PathBuf::from("/tmp/comit/"),
            }),
            logging: Some(Logging {
                level: Some(LevelFilter::Debug),
                structured: Some(false),
            }),
            bitcoin: Some(Bitcoin {
                network: bitcoin::Network::Bitcoin,
                node_url: "http://example.com".parse().unwrap(),
            }),
            ethereum: Some(Ethereum {
                node_url: "http://example.com".parse().unwrap(),
            }),
        };

        let config = toml::from_str::<File>(contents);
        assert_that(&config).is_ok().is_equal_to(file);
    }

    #[test]
    fn config_with_defaults_roundtrip() {
        // we start with the default config file
        let default_file = File::default();

        // convert to settings, this populates all empty fields with defaults
        let effective_settings = Settings::from_config_file_and_defaults(default_file).unwrap();

        // write settings back to file
        let file_with_effective_settings = File::from(effective_settings);

        let serialized = toml::to_string(&file_with_effective_settings).unwrap();
        let file = toml::from_str::<File>(&serialized).unwrap();

        assert_eq!(file, file_with_effective_settings)
    }
}
