use std::fs;

use cosmrs::{
    proto::cosmwasm::wasm::v1::{
        QueryCodeRequest, QueryCodeResponse, QueryContractInfoRequest, QueryContractInfoResponse,
    },
    rpc::HttpClient,
};
use cosmwasm_schema::cw_serde;
use prost::Message;
use thiserror::Error;

use crate::helpers::rpc_query;

/// Enum to represent the different ways to get a contract artifact
/// - Local: A local file path
/// - Url: A url to download the artifact from
/// - Chain: A chain id to download the artifact from
#[cw_serde]
pub enum Artifact {
    Local(String),
    Url(String),
    ChainCodeId {
        rpc_endpoint: String,
        code_id: u64,
    },
    ChainContractAddress {
        rpc_endpoint: String,
        contract_address: String,
    },
    Git {
        url: String,
        branch: String,
        crate_name: String,
    },
}

#[derive(Error, Debug)]
pub enum ArtifactError {
    #[error("{0}")]
    RunnerError(#[from] test_tube::RunnerError),

    #[error("{0}")]
    DecodeError(#[from] prost::DecodeError),

    #[error("{0}")]
    RpcError(#[from] cosmrs::rpc::error::Error),

    #[error("{0}")]
    IoError(#[from] std::io::Error),

    #[error("{0}")]
    Generic(String),
}

impl Artifact {
    pub fn get_wasm_byte_code(self) -> Result<Vec<u8>, ArtifactError> {
        match self {
            Artifact::Local(path) => Ok(fs::read(path)?),
            Artifact::Url(_url) => todo!(),
            Artifact::ChainCodeId {
                rpc_endpoint,
                code_id,
            } => {
                let http_client = HttpClient::new(rpc_endpoint.as_str())?;
                download_wasm_from_code_id(&http_client, code_id)
            }
            Artifact::ChainContractAddress {
                rpc_endpoint,
                contract_address,
            } => {
                let http_client = HttpClient::new(rpc_endpoint.as_str())?;
                download_wasm_from_contract_address(&http_client, contract_address)
            }
            Artifact::Git {
                url: _,
                branch: _,
                crate_name: _,
            } => todo!(),
        }
    }
}

fn download_wasm_from_code_id(
    http_client: &HttpClient,
    code_id: u64,
) -> Result<Vec<u8>, ArtifactError> {
    // Query wasm file
    let code_res = QueryCodeResponse::decode(
        rpc_query(
            http_client,
            QueryCodeRequest { code_id },
            "/cosmwasm.wasm.v1.Query/Code",
        )?
        .value
        .as_slice(),
    )?;
    Ok(code_res.data)
}

fn download_wasm_from_contract_address(
    http_client: &HttpClient,
    contract_address: impl Into<String>,
) -> Result<Vec<u8>, ArtifactError> {
    // Query contract info
    let code_id = QueryContractInfoResponse::decode(
        rpc_query(
            http_client,
            QueryContractInfoRequest {
                address: contract_address.into(),
            },
            "/cosmwasm.wasm.v1.Query/ContractInfo",
        )?
        .value
        .as_slice(),
    )?
    .contract_info
    .ok_or(ArtifactError::Generic(
        "failed to query contract info".to_string(),
    ))?
    .code_id;

    download_wasm_from_code_id(http_client, code_id)
}

// #[allow(clippy::expect_fun_call)]
// pub fn clone_repo(
//     &self,
//     contract: &Contract,
//     artifact_folder: &str,
// ) -> Result<(), git2::Error> {
//     let url = contract.url.clone();
//     let git_name = url.split('/').collect::<Vec<&str>>().pop().unwrap();
//     let repo_name = git_name.replace(".git", "");
//     let path = format!(
//         "{}/{}/{}",
//         artifact_folder, DEFAULT_PROJECTS_FOLDER, repo_name
//     );

//     if Path::new(&path).is_dir() {
//         println!("Repository already exist: {}", repo_name);
//         Repository::open(&path).expect(format!("Cant open repo [{}]", repo_name).as_str());
//         Ok(())
//     } else {
//         let mut cb = git2::RemoteCallbacks::new();
//         let git_config = git2::Config::open_default().unwrap();
//         let mut ch = CredentialHandler::new(git_config);
//         cb.credentials(move |url, username, allowed| {
//             ch.try_next_credential(url, username, allowed)
//         });

//         // clone a repository
//         let mut fo = git2::FetchOptions::new();
//         fo.remote_callbacks(cb)
//             .download_tags(git2::AutotagOption::All)
//             .update_fetchhead(true);
//         println!("Cloning repo: {} on: {}", url, path);
//         git2::build::RepoBuilder::new()
//             .branch(&contract.branch)
//             .fetch_options(fo)
//             .clone(&url, path.as_ref())
//             .expect(format!("Cant clone repo [{}]", repo_name).as_str());
//         Ok(())
//     }
// }

// fn wasm_compile(&self, contract: &Contract, artifact_folder: &str) -> Result<(), io::Error> {
//     let url = contract.url.clone();
//     let git_name = url.split('/').collect::<Vec<&str>>().pop().unwrap();
//     let repo_name = git_name.replace(".git", "");
//     let cargo_path = &contract.cargo_path;
//     let path = format!(
//         "{}/{}/{}/{}",
//         artifact_folder, DEFAULT_PROJECTS_FOLDER, repo_name, cargo_path
//     );

//     if Path::new(&path).is_dir() {
//         //let command = format!("cargo cw-optimizoor {}/Cargo.toml", path);
//         // Note: https://github.com/mandrean/cw-optimizoor/blob/87fbbcea67398dfa9cb21165848b7448d98f17c4/src/lib.rs has some problems with workspaces
//         println!(
//             "current dir[{}] project dir[{}]",
//             std::env::current_dir().unwrap().to_str().unwrap(),
//             path
//         );
//         let command = format!(
//             "(cd {}; cargo build --release --locked --target wasm32-unknown-unknown --lib)",
//             path
//         );
//         println!("Command [{}]", command);
//         let status = Command::new("bash")
//             .arg("-c")
//             .arg(command)
//             .stdout(Stdio::inherit())
//             .status()
//             .expect("cargo build failed");

//         println!("process finished with: {:#?}", status);

//         println!(
//             "Artifacts generated on {}/target/wasm32-unknown-unknown/debug/",
//             path
//         );

//         for entry in fs::read_dir(format!(
//             "{}/target/wasm32-unknown-unknown/release/deps/",
//             path
//         ))
//         .unwrap()
//         {
//             let path = entry.as_ref().unwrap().path();
//             if let Some(extension) = path.extension() {
//                 if extension == "wasm" {
//                     let filename = entry.as_ref().unwrap().file_name().into_string().unwrap();
//                     if contract.artifact.eq(&filename) {
//                         rename(
//                             path,
//                             format!(
//                                 "{}/{}",
//                                 artifact_folder,
//                                 entry.as_ref().unwrap().file_name().into_string().unwrap()
//                             ),
//                         )
//                         .expect("Failed renaming wasm files");
//                     }
//                 }
//             }
//         }
//     } else {
//         println!("Path to compile doesn't exist [{}]", path);
//     }
//     Ok(())
// }
