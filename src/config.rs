use std::{collections::HashMap, env};

use config::Config;

use serde::{Deserialize, Serialize};

use thiserror::Error;

use crate::{artifact::Artifact, chain::ChainConfig};

#[cfg(feature = "rpc-runner")]
use crate::rpc_runner::container::ContainerInfo;

pub const DEFAULT_PROJECTS_FOLDER: &str = "cloned_repos";
#[derive(Clone, Debug, Deserialize)]
pub struct TestConfig {
    pub contracts: HashMap<String, Contract>,
    pub chain_config: ChainConfig,
    #[cfg(feature = "rpc-runner")]
    pub container: Option<ContainerInfo>,
    #[serde(default)]
    pub folder: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Contract {
    pub name: String,
    pub artifact: Artifact,
    #[serde(default)]
    pub chain_address: String,
}

impl Contract {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportedAccount {
    pub name: String,
    pub address: String,
    pub mnemonic: String,
    pub pubkey: String,
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("query error: {}", .msg)]
    QueryError { msg: String },
    #[error("invalid mnemonic: {}", .msg)]
    InvalidMnemonic { msg: String },
}

impl TestConfig {
    pub fn from_yaml(file: &str) -> Self {
        println!("Working directory [{}]", get_current_working_dir());
        println!("Reading {}", file);
        let settings = Config::builder()
            .add_source(config::File::with_name(file))
            .build()
            .unwrap();
        settings.try_deserialize::<Self>().unwrap()
    }

    pub const fn contracts(&self) -> &HashMap<String, Contract> {
        &self.contracts
    }
}

fn get_current_working_dir() -> String {
    let res = env::current_dir();
    match res {
        Ok(path) => path.into_os_string().into_string().unwrap(),
        Err(_) => "FAILED".to_string(),
    }
}
