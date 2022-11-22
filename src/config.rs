use std::{
    collections::HashMap,
    env,
    fs::{self, rename},
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use config::Config;
use downloader::Downloader;
use git2::Repository;
use git2_credentials::CredentialHandler;
use osmosis_testing::{FeeSetting, SigningAccount};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use cosmwasm_std::Coin;

use testcontainers::{images::generic::GenericImage, Container};

use cosmrs::bip32::{self, Error};

pub const DEFAULT_PROJECTS_FOLDER: &str = "cloned_repos";

#[derive(Clone, Debug, Deserialize)]
pub struct TestConfig {
    pub contracts: HashMap<String, Contract>,
    pub container: ContainerInfo,
    pub chain_cfg: ChainCfg,
    pub folder: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Contract {
    pub url: String,
    pub branch: String,
    pub cargo_path: String,
    pub artifacts: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ContainerInfo {
    pub name: String,
    pub tag: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportedAccount {
    pub name: String,
    pub address: String,
    pub mnemonic: String,
    pub pubkey: String,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, Deserialize)]
pub struct ChainCfg {
    pub name: String,
    denom: String,
    prefix: String,
    pub chain_id: String,
    pub gas_price: u64,
    pub gas_adjustment: f64,
    pub derivation_path: String,
    pub rpc_endpoint: String,
    pub grpc_endpoint: String,
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("query error: {}", .msg)]
    QueryError { msg: String },
    #[error("invalid mnemonic: {}", .msg)]
    InvalidMnemonic { msg: String },
}

impl ChainCfg {
    pub fn denom(&self) -> &str {
        &self.denom
    }

    pub fn prefix(&self) -> &str {
        &self.prefix
    }
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

    pub fn build(&self, artifact_folder: &str) {
        // TODO: There a race here, we should use tokio async an block the threads.
        // Lets download all contracts
        println!("Working dir [{}]", get_current_working_dir());

        // Check if artifacts already has been compiled
        let artifacts: HashMap<String, Contract> = self
            .contracts
            .clone()
            .into_iter()
            .flat_map(|i| {
                match i.1.artifacts.iter().find(|&contract_name| {
                    let fp = format!("{}/{}", artifact_folder, contract_name);
                    !Path::new(&fp).exists()
                }) {
                    Some(_) => {
                        println!("Processing [{}]", i.0);
                        Some(i)
                    }
                    None => {
                        println!("Files already exist, skipping [{}]", i.0);
                        None
                    }
                }
            })
            .collect();

        //println!("{:?}", artifacts);

        let download_list: Vec<Contract> = artifacts
            .values()
            .filter(|c| c.url.contains('.'))
            .filter(|c| {
                let extension = c.url.split('.').collect::<Vec<&str>>().pop().unwrap();
                extension == "wasm"
            })
            .cloned()
            .collect();

        let clone_list: Vec<Contract> = artifacts
            .values()
            .filter(|c| c.url.contains('.'))
            .filter(|c| {
                let extension = c.url.split('.').collect::<Vec<&str>>().pop().unwrap();
                extension == "git"
            })
            .cloned()
            .collect();

        // compile list
        let compile_list: Vec<Contract> = artifacts
            .values()
            .filter(|c| !c.url.contains("https"))
            .cloned()
            .collect();

        //println!("download_list [{:#?}]", download_list);
        //println!("clone_list [{:#?}]", clone_list);
        //println!("compile_list [{:#?}]", compile_list);

        for contract in download_list {
            self.download_contract(&contract, artifact_folder);
        }

        for contract in clone_list {
            self.clone_repo(&contract, artifact_folder)
                .expect("Error cloning artifact");
            if !contract.artifacts.is_empty() {
                self.wasm_compile(&contract, artifact_folder)
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
                artifact_folder, DEFAULT_PROJECTS_FOLDER, repo_name
            ));
            let _ = Self::copy_dir_all(in_dir, out_dir);
            self.wasm_compile(&contract, artifact_folder)
                .expect("Error compiling artifact");
        }
    }

    pub fn bind_chain_to_container(&mut self, container: &Container<GenericImage>) {
        // We inject here the endpoint since containers have a life time
        self.chain_cfg.rpc_endpoint =
            format!("http://localhost:{}/", container.get_host_port_ipv4(26657));
        self.chain_cfg.grpc_endpoint =
            format!("http://localhost:{}/", container.get_host_port_ipv4(9090));
    }

    pub const fn contracts(&self) -> &HashMap<String, Contract> {
        &self.contracts
    }

    pub fn import_account(&self, name: &str) -> Result<SigningAccount, ConfigError> {
        //println!("get_account [{}]", name);
        let path = format!("{}/{}/accounts.json", self.folder, self.chain_cfg.name);
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
                    Self::mnemonic_to_signing_key(&ia.mnemonic, &self.chain_cfg).unwrap();
                //println!("Generated key [{:?}]", signging_key.public_key());
                Ok(SigningAccount::new(
                    signing_key,
                    FeeSetting::Auto {
                        gas_price: Coin::new(
                            self.chain_cfg.gas_price.into(),
                            self.chain_cfg.denom(),
                        ),
                        gas_adjustment: self.chain_cfg.gas_adjustment,
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

        accounts.insert(
            "validator".to_string(),
            self.import_account("validator").unwrap(),
        );
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
        chain_cfg: &ChainCfg,
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
    fn download_contract(&self, contract: &Contract, artifact_folder: &str) {
        let mut downloader = Downloader::builder()
            .download_folder(std::path::Path::new(&artifact_folder))
            .parallel_requests(32)
            .build()
            .unwrap();
        downloader
            .download(&[downloader::Download::new(&contract.url)])
            .unwrap();
        //let fp = format!("{}/{}", DEFAULT_ARTIFACTS_FOLDER, contract.artifacts[0]);
        // if !Path::new(&fp).exists() {
        //     println!("File to download [{}]", contract.url);
        //     downloader.download(&[downloader::Download::new(&contract.url)]).unwrap();
        // } else {
        //     println!("File already exist [{}]", fp);
        // }
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
                        if contract.artifacts.contains(&filename) {
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
