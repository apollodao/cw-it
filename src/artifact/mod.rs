use std::fs;

use serde::Deserialize;
use thiserror::Error;

#[cfg(feature = "chain-download")]
use self::on_chain::{download_wasm_from_code_id, download_wasm_from_contract_address};

#[cfg(feature = "chain-download")]
mod on_chain;

/// Enum to represent the different ways to get a contract artifact
/// - Local: A local file path
/// - Url: A url to download the artifact from
/// - Chain: A chain id to download the artifact from
#[derive(Clone, Debug, Deserialize)]
pub enum Artifact {
    Local(String),
    #[cfg(feature = "url-download")]
    Url(String),
    #[cfg(feature = "chain-download")]
    ChainCodeId {
        rpc_endpoint: String,
        code_id: u64,
    },
    #[cfg(feature = "chain-download")]
    ChainContractAddress {
        rpc_endpoint: String,
        contract_address: String,
    },
    #[cfg(feature = "git")]
    Git {
        url: String,
        branch: String,
        crate_name: String,
    },
}

#[derive(Error, Debug)]
pub enum ArtifactError {
    #[error("{0}")]
    IoError(#[from] std::io::Error),

    #[error("{0}")]
    Generic(String),

    #[cfg(feature = "chain-download")]
    #[error("{0}")]
    DecodeError(#[from] prost::DecodeError),

    #[cfg(feature = "chain-download")]
    #[error("{0}")]
    RpcError(#[from] cosmrs::rpc::error::Error),
}

impl Artifact {
    pub fn get_wasm_byte_code(self) -> Result<Vec<u8>, ArtifactError> {
        match self {
            Artifact::Local(path) => Ok(fs::read(path)?),
            #[cfg(feature = "url-download")]
            Artifact::Url(_url) => todo!(),
            #[cfg(feature = "chain-download")]
            Artifact::ChainCodeId {
                rpc_endpoint,
                code_id,
            } => download_wasm_from_code_id(&rpc_endpoint, code_id),
            #[cfg(feature = "chain-download")]
            Artifact::ChainContractAddress {
                rpc_endpoint,
                contract_address,
            } => download_wasm_from_contract_address(&rpc_endpoint, contract_address),
            #[cfg(feature = "git")]
            Artifact::Git {
                url: _,
                branch: _,
                crate_name: _,
            } => todo!(),
        }
    }
}
