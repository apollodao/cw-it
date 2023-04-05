use cosmwasm_schema::cw_serde;
use std::fs;
use thiserror::Error;

#[cfg(feature = "multi-test")]
use {cosmwasm_std::Empty, cw_multi_test::Contract};

#[cfg(feature = "chain-download")]
use self::on_chain::{download_wasm_from_code_id, download_wasm_from_contract_address};

#[cfg(feature = "chain-download")]
mod on_chain;

/// Enum to represent the different ways to get a contract artifact
/// - Local: A local file path
/// - Url: A url to download the artifact from
/// - Chain: A chain id to download the artifact from
#[cw_serde]
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

/// Enum to represent different ways of representing a contract in tests
pub enum ContractType {
    Artifact(Artifact),
    #[cfg(feature = "multi-test")]
    MultiTestContract(Box<dyn Contract<Empty, Empty>>),
}

/// Convenience type to map contract names to implementations
pub type ContractMap = std::collections::HashMap<String, ContractType>;

/// A const-safe helper enum to specify where to get the a remote wasm file
#[cw_serde]
#[derive(Copy)]
#[cfg(feature = "chain-download")]
pub enum ChainArtifact {
    Addr(&'static str),
    CodeId(u64),
}

#[cfg(feature = "chain-download")]
impl ChainArtifact {
    pub fn into_artifact(self, rpc_endpoint: String) -> Artifact {
        match self {
            ChainArtifact::Addr(addr) => Artifact::ChainContractAddress {
                rpc_endpoint,
                contract_address: addr.to_string(),
            },
            ChainArtifact::CodeId(id) => Artifact::ChainCodeId {
                rpc_endpoint,
                code_id: id,
            },
        }
    }
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
    pub fn get_wasm_byte_code(&self) -> Result<Vec<u8>, ArtifactError> {
        match self {
            Artifact::Local(path) => Ok(fs::read(path)?),
            #[cfg(feature = "url-download")]
            Artifact::Url(_url) => todo!(),
            #[cfg(feature = "chain-download")]
            Artifact::ChainCodeId {
                rpc_endpoint,
                code_id,
            } => download_wasm_from_code_id(rpc_endpoint, *code_id),
            #[cfg(feature = "chain-download")]
            Artifact::ChainContractAddress {
                rpc_endpoint,
                contract_address,
            } => download_wasm_from_contract_address(rpc_endpoint, contract_address),
            #[cfg(feature = "git")]
            Artifact::Git {
                url: _,
                branch: _,
                crate_name: _,
            } => todo!(),
        }
    }
}
