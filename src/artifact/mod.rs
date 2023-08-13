use cosmwasm_schema::cw_serde;
use std::{
    fmt::{Debug, Formatter},
    fs,
};
use thiserror::Error;

#[cfg(feature = "multi-test")]
use {apollo_cw_multi_test::Contract, cosmwasm_std::Empty};

#[cfg(feature = "chain-download")]
use self::on_chain::{download_wasm_from_code_id, download_wasm_from_contract_address};

#[cfg(feature = "chain-download")]
mod on_chain;

/// Enum to represent the different ways to get a contract artifact, i.e a contract wasm file.
#[cw_serde]
pub enum Artifact {
    /// A path to a local wasm file.
    Local(String),
    /// A url to download the wasm file from.
    #[cfg(feature = "url-download")]
    Url(String),
    /// An RPC endpoint to download the artifact from, together with a code id.
    /// Downloads the wasm mapping to the code id from the chain.
    #[cfg(feature = "chain-download")]
    ChainCodeId { rpc_endpoint: String, code_id: u64 },
    /// An RPC endpoint to download the artifact from, together with a contract address.
    /// Downloads the wasm mapping to the contract address from the chain.
    #[cfg(feature = "chain-download")]
    ChainContractAddress {
        rpc_endpoint: String,
        contract_address: String,
    },
}

/// Enum to represent different ways of representing a contract in tests
pub enum ContractType {
    /// A contract artifact. This is the most common way to represent a contract
    /// and is an abstraction around different ways to obtain a contract wasm file.
    Artifact(Artifact),
    /// A multi-test contract. Since multi-test contracts are not wasm files, but instead
    /// pointers to entry points of the contract, we need to handle them differently.
    #[cfg(feature = "multi-test")]
    MultiTestContract(Box<dyn Contract<Empty, Empty>>),
}

impl Debug for ContractType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ContractType::Artifact(artifact) => write!(f, "Artifact({:?})", artifact),
            #[cfg(feature = "multi-test")]
            ContractType::MultiTestContract(_) => write!(f, "MultiTestContract"),
        }
    }
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
    /// Return the wasm byte code for the artifact.
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_type_debug() {
        let artifact = Artifact::Local("foo".to_string());
        let contract_type = ContractType::Artifact(artifact);
        assert_eq!(format!("{:?}", contract_type), "Artifact(Local(\"foo\"))");
    }

    #[cfg(feature = "multi-test")]
    mod multi_test {
        use apollo_cw_multi_test::ContractWrapper;
        use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};

        use super::*;

        fn execute(
            _deps: DepsMut,
            _env: Env,
            _info: MessageInfo,
            _msg: Empty,
        ) -> StdResult<Response> {
            Ok(Response::default())
        }

        fn query(_deps: Deps, _env: Env, _msg: Empty) -> StdResult<Binary> {
            Ok(Binary::default())
        }

        fn instantiate(
            _deps: DepsMut,
            _env: Env,
            _info: MessageInfo,
            _msg: Empty,
        ) -> StdResult<Response> {
            Ok(Response::default())
        }

        #[test]
        fn contract_type_multi_test() {
            let contract_type = ContractType::MultiTestContract(Box::new(
                ContractWrapper::new_with_empty(execute, instantiate, query),
            ));
            assert_eq!(format!("{:?}", contract_type), "MultiTestContract");
        }
    }
}
