use std::collections::HashMap;

use config::Config;

use serde::Deserialize;

use thiserror::Error;

use crate::{artifact::Artifact, helpers::get_current_working_dir};

pub const DEFAULT_PROJECTS_FOLDER: &str = "cloned_repos";
#[derive(Clone, Debug, Deserialize)]
pub struct TestConfig {
    pub artifacts: HashMap<String, Artifact>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Contract {
    pub name: String,
    pub artifact: Artifact,
    #[serde(default)]
    pub chain_address: String,
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
}
