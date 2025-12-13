use std::net::{IpAddr, Ipv4Addr};

use config::Environment;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_address")]
    pub address: IpAddr,

    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_address() -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))
}

fn default_port() -> u16 {
    3030
}

impl Config {
    pub fn new() -> Result<Self, config::ConfigError> {
        config::Config::builder()
            .add_source(Environment::with_prefix("VETER"))
            .build()?
            .try_deserialize()
    }
}
