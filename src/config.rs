use std::{
    collections::HashMap,
    env,
    fs::{self, rename, File},
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use config::Config;
use cosmwasm_schema::cw_serde;
use git2::Repository;
use git2_credentials::CredentialHandler;
use osmosis_test_tube::{FeeSetting, RunnerResult, SigningAccount};
use prost::Message;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use cosmwasm_std::Coin;

use testcontainers::{core::WaitFor, images::generic::GenericImage, Container};

use cosmrs::{
    bip32::{self, Error},
    proto::cosmwasm::wasm::v1::{
        QueryCodeRequest, QueryCodeResponse, QueryContractInfoRequest, QueryContractInfoResponse,
    },
    rpc::{endpoint::abci_query::AbciQuery, Client, HttpClient},
};

use crate::chain::{tokio_block, ChainConfig};

pub const DEFAULT_PROJECTS_FOLDER: &str = "cloned_repos";
pub const DEFAULT_WAIT: u64 = 30;
#[derive(Clone, Debug, Deserialize)]
pub struct TestConfig {
    pub contracts: HashMap<String, Contract>,
    pub container: Option<ContainerInfo>,
    pub chain_config: ChainConfig,
    pub folder: String,
    pub artifacts_folder: String,
    #[serde(default)]
    pub contract_chain_download_rpc: String,
}

#[cw_serde]
pub enum PreferredSource {
    Url,
    Chain,
}

impl Default for PreferredSource {
    fn default() -> Self {
        Self::Chain
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Contract {
    pub artifact: String,
    #[serde(default)]
    pub preferred_source: PreferredSource,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub cargo_path: String,
    #[serde(default)]
    pub always_fetch: bool,
    #[serde(default)]
    pub chain_address: String,
    #[serde(default)]
    pub chain_code_id: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ContainerInfo {
    pub name: String,
    pub tag: String,
    pub volumes: Vec<(String, String)>,
    pub entrypoint: Option<String>,
    pub ports: Vec<u16>,
}

impl ContainerInfo {
    pub fn get_container_image(&self) -> GenericImage {
        let mut image = GenericImage::new(self.name.clone(), self.tag.clone())
            .with_wait_for(WaitFor::seconds(DEFAULT_WAIT));

        for port in self.ports.iter() {
            image = image.with_exposed_port(*port);
        }
        if let Some(entrypoint) = &self.entrypoint {
            image = image.with_entrypoint(entrypoint);
        }
        let working_dir = get_current_working_dir();
        for (from, dest) in &self.volumes {
            // TODO: Merge paths in better way? Should allow leading dot in `from`...
            let from = format!("{}/{}", working_dir, from);
            image = image.with_volume(from, dest);
        }

        image
    }
}

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

    pub fn build(&self) {
        // TODO: There a race here, we should use tokio async an block the threads.
        // Lets download all contracts
        println!("Working dir [{}]", get_current_working_dir());

        // Get all contracts for which we don't already have the wasm file
        let missing_artifacts: HashMap<String, Contract> = self
            .contracts
            .clone()
            .into_iter()
            .filter(|(_, contract)| {
                let fp = format!("{}/{}", self.artifacts_folder, contract.artifact);
                let already_exists = Path::new(&fp).exists();
                return !already_exists || contract.always_fetch; // Also re-download if always_fetch is true
            })
            .collect();

        let mut chain_download_list: Vec<Contract> = vec![];
        let mut clone_list: Vec<Contract> = vec![];
        let mut compile_list: Vec<Contract> = vec![];

        let mut parse_contract_url = |contract: Contract| {
            let extension = contract
                .url
                .split('.')
                .collect::<Vec<&str>>()
                .pop()
                .unwrap();
            if extension == "git" {
                // URL is a git repo, so we need to clone it
                clone_list.push(contract);
            } else {
                compile_list.push(contract); // TODO: Not sure what behavior Pablo wanted here.
            }
        };

        for (_, contract) in missing_artifacts {
            match contract.preferred_source {
                PreferredSource::Url => {
                    if contract.url == ""
                        && (contract.chain_address != "" || contract.chain_code_id != 0)
                    {
                        // Preferred URL, but no URL available. Use chain instead.
                        chain_download_list.push(contract);
                    } else if contract.url != "" {
                        parse_contract_url(contract);
                    }
                }
                PreferredSource::Chain => {
                    if (contract.chain_address == "" && contract.chain_code_id == 0)
                        && contract.url != ""
                    {
                        // Preferred chain, but no chain address available. Use URL instead.
                        parse_contract_url(contract);
                    } else if contract.chain_address != "" || contract.chain_code_id != 0 {
                        chain_download_list.push(contract);
                    }
                }
            }
        }

        //println!("download_list [{:#?}]", download_list);
        //println!("clone_list [{:#?}]", clone_list);
        //println!("compile_list [{:#?}]", compile_list);

        if !chain_download_list.is_empty() {
            let http_client = HttpClient::new(self.contract_chain_download_rpc.as_str()).unwrap();
            for contract in chain_download_list {
                self.download_contract_from_chain(&http_client, &contract)
            }
        }

        for contract in clone_list {
            self.clone_repo(&contract, &self.artifacts_folder)
                .expect("Error cloning artifact");
            if !contract.artifact.is_empty() {
                self.wasm_compile(&contract, &self.artifacts_folder)
                    .expect("Error compiling artifact");
            }
        }

        for contract in compile_list {
            let url = contract.url.clone();
            let git_name = url.split('/').collect::<Vec<&str>>().pop().unwrap();
            let repo_name = git_name.replace(".git", "");

            let in_dir = PathBuf::from(&contract.url);
            let out_dir = PathBuf::from(format!(
                "{}/{}/{}",
                self.artifacts_folder, DEFAULT_PROJECTS_FOLDER, repo_name
            ));
            let _ = Self::copy_dir_all(in_dir, out_dir);
            self.wasm_compile(&contract, &self.artifacts_folder)
                .expect("Error compiling artifact");
        }
    }

    pub fn bind_chain_to_container(&mut self, container: &Container<GenericImage>) {
        // We inject here the endpoint since containers have a life time
        self.chain_config.rpc_endpoint =
            format!("http://localhost:{}/", container.get_host_port_ipv4(26657));
        self.chain_config.grpc_endpoint =
            format!("http://localhost:{}/", container.get_host_port_ipv4(9090));
    }

    pub const fn contracts(&self) -> &HashMap<String, Contract> {
        &self.contracts
    }

    pub fn import_account(&self, name: &str) -> Result<SigningAccount, ConfigError> {
        //println!("get_account [{}]", name);
        let path = format!("{}/{}/accounts.json", self.folder, self.chain_config.name);
        println!("Reading accounts from [{}]", path);
        let bytes = fs::read(path).unwrap();
        let accounts: Vec<ImportedAccount> = serde_json::from_slice(&bytes).unwrap();
        let imported_account = accounts.iter().find(|e| e.name.contains(name));
        imported_account.map_or_else(
            || {
                Err(ConfigError::QueryError {
                    msg: format!("Account not found [{}]", name),
                })
            },
            |ia| {
                let signing_key =
                    Self::mnemonic_to_signing_key(&ia.mnemonic, &self.chain_config).unwrap();
                //println!("Generated key [{:?}]", signging_key.public_key());
                Ok(SigningAccount::new(
                    self.chain_config.prefix().to_string(),
                    signing_key,
                    FeeSetting::Auto {
                        gas_price: Coin::new(
                            self.chain_config.gas_price.into(),
                            self.chain_config.denom(),
                        ),
                        gas_adjustment: self.chain_config.gas_adjustment,
                    },
                ))
            },
        )
    }

    pub fn import_all_accounts(&self) -> HashMap<String, SigningAccount> {
        let mut accounts = HashMap::<String, SigningAccount>::new();
        (1..10).for_each(|n| {
            let name = format!("test{}", n);
            let signing_account = self.import_account(&name).unwrap();
            accounts.insert(name, signing_account);
        });

        // accounts.insert(
        //     "validator".to_string(),
        //     self.import_account("validator").unwrap(),
        // );
        accounts
    }

    // pub fn get_accounts_map(chain_cfg: &ChainCfg) -> HashMap<String, SigningAccount> {
    //    // println!("get_accounts_map");
    //     let path = format!("../configs/{}/accounts.json", chain_cfg.name);
    //     let bytes = fs::read(path).unwrap();
    //     let accounts: Vec<ImportedAccount> = serde_json::from_slice(&bytes).unwrap();

    //     let mut result = HashMap::new();
    //     for imported_account in accounts {
    //         let amount = Coin::new(1_000_000, chain_cfg.denom());
    //         let gas_limit = 25_000_000;
    //         let signging_key =
    //             Self::mnemonic_to_signing_key(&imported_account.mnemonic, chain_cfg).unwrap();
    //         result.insert(
    //             imported_account.name.clone(),
    //             SigningAccount::new(
    //                 signging_key,
    //                 FeeSetting::Custom { amount, gas_limit },
    //                 chain_cfg.prefix().to_string(),
    //             ),
    //         );
    //     }
    //     result
    // }

    fn mnemonic_to_signing_key(
        mnemonic: &str,
        chain_cfg: &ChainConfig,
    ) -> Result<cosmrs::crypto::secp256k1::SigningKey, Error> {
        let seed = bip32::Mnemonic::new(mnemonic, bip32::Language::English)?.to_seed("");
        cosmrs::crypto::secp256k1::SigningKey::derive_from_path(
            seed,
            &chain_cfg.derivation_path.parse().unwrap(),
        )
    }

    fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
        fs::create_dir_all(&dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            if ty.is_dir() {
                Self::copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
            } else {
                fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
            }
        }
        Ok(())
    }

    fn download_contract_from_chain(&self, http_client: &HttpClient, contract: &Contract) {
        let contract_path = format!("{}/{}", self.artifacts_folder, contract.artifact);
        println!("Downloading {} from chain", contract.artifact);

        // Query contract info
        let code_id = if contract.chain_code_id == 0 {
            let contract_info_res = QueryContractInfoResponse::decode(
                abci_query(
                    http_client,
                    QueryContractInfoRequest {
                        address: contract.chain_address.clone(),
                    },
                    "/cosmwasm.wasm.v1.Query/ContractInfo",
                )
                .unwrap()
                .value
                .as_slice(),
            )
            .unwrap();
            println!("Contract info: {:?}", contract_info_res);
            contract_info_res.contract_info.unwrap().code_id
        } else {
            contract.chain_code_id
        };

        // Query wasm file
        let code_res = QueryCodeResponse::decode(
            abci_query(
                &http_client,
                QueryCodeRequest { code_id },
                "/cosmwasm.wasm.v1.Query/Code",
            )
            .unwrap()
            .value
            .as_slice(),
        )
        .unwrap();
        let wasm = code_res.data;

        // Write wasm file
        println!("Writing wasm file to {}", contract_path);
        let mut file = File::create(&contract_path).unwrap();
        file.write_all(&wasm).unwrap();
        println!("Wrote to disk: {}", contract_path);
    }

    #[allow(clippy::expect_fun_call)]
    pub fn clone_repo(
        &self,
        contract: &Contract,
        artifact_folder: &str,
    ) -> Result<(), git2::Error> {
        let url = contract.url.clone();
        let git_name = url.split('/').collect::<Vec<&str>>().pop().unwrap();
        let repo_name = git_name.replace(".git", "");
        let path = format!(
            "{}/{}/{}",
            artifact_folder, DEFAULT_PROJECTS_FOLDER, repo_name
        );

        if Path::new(&path).is_dir() {
            println!("Repository already exist: {}", repo_name);
            Repository::open(&path).expect(format!("Cant open repo [{}]", repo_name).as_str());
            Ok(())
        } else {
            let mut cb = git2::RemoteCallbacks::new();
            let git_config = git2::Config::open_default().unwrap();
            let mut ch = CredentialHandler::new(git_config);
            cb.credentials(move |url, username, allowed| {
                ch.try_next_credential(url, username, allowed)
            });

            // clone a repository
            let mut fo = git2::FetchOptions::new();
            fo.remote_callbacks(cb)
                .download_tags(git2::AutotagOption::All)
                .update_fetchhead(true);
            println!("Cloning repo: {} on: {}", url, path);
            git2::build::RepoBuilder::new()
                .branch(&contract.branch)
                .fetch_options(fo)
                .clone(&url, path.as_ref())
                .expect(format!("Cant clone repo [{}]", repo_name).as_str());
            Ok(())
        }
    }

    fn wasm_compile(&self, contract: &Contract, artifact_folder: &str) -> Result<(), io::Error> {
        let url = contract.url.clone();
        let git_name = url.split('/').collect::<Vec<&str>>().pop().unwrap();
        let repo_name = git_name.replace(".git", "");
        let cargo_path = &contract.cargo_path;
        let path = format!(
            "{}/{}/{}/{}",
            artifact_folder, DEFAULT_PROJECTS_FOLDER, repo_name, cargo_path
        );

        if Path::new(&path).is_dir() {
            //let command = format!("cargo cw-optimizoor {}/Cargo.toml", path);
            // Note: https://github.com/mandrean/cw-optimizoor/blob/87fbbcea67398dfa9cb21165848b7448d98f17c4/src/lib.rs has some problems with workspaces
            println!(
                "current dir[{}] project dir[{}]",
                std::env::current_dir().unwrap().to_str().unwrap(),
                path
            );
            let command = format!(
                "(cd {}; cargo build --release --locked --target wasm32-unknown-unknown --lib)",
                path
            );
            println!("Command [{}]", command);
            let status = Command::new("bash")
                .arg("-c")
                .arg(command)
                .stdout(Stdio::inherit())
                .status()
                .expect("cargo build failed");

            println!("process finished with: {:#?}", status);

            println!(
                "Artifacts generated on {}/target/wasm32-unknown-unknown/debug/",
                path
            );

            for entry in fs::read_dir(format!(
                "{}/target/wasm32-unknown-unknown/release/deps/",
                path
            ))
            .unwrap()
            {
                let path = entry.as_ref().unwrap().path();
                if let Some(extension) = path.extension() {
                    if extension == "wasm" {
                        let filename = entry.as_ref().unwrap().file_name().into_string().unwrap();
                        if contract.artifact.eq(&filename) {
                            rename(
                                path,
                                format!(
                                    "{}/{}",
                                    artifact_folder,
                                    entry.as_ref().unwrap().file_name().into_string().unwrap()
                                ),
                            )
                            .expect("Failed renaming wasm files");
                        }
                    }
                }
            }
        } else {
            println!("Path to compile doesn't exist [{}]", path);
        }
        Ok(())
    }
}

fn get_current_working_dir() -> String {
    let res = env::current_dir();
    match res {
        Ok(path) => path.into_os_string().into_string().unwrap(),
        Err(_) => "FAILED".to_string(),
    }
}

fn abci_query<T: Message>(client: &HttpClient, req: T, path: &str) -> RunnerResult<AbciQuery> {
    let mut buf = Vec::with_capacity(req.encoded_len());
    req.encode(&mut buf).unwrap();
    Ok(tokio_block(client.abci_query(
        Some(path.parse().unwrap()),
        buf,
        None,
        false,
    ))??)
}
