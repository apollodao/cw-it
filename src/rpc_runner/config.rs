use std::{collections::HashMap, fs};

use cosmrs::bip32;
use cosmwasm_std::Coin;
use serde::{Deserialize, Serialize};
use test_tube::{account::FeeSetting, SigningAccount};
use testcontainers::{images::generic::GenericImage, Container};

use super::chain::ChainConfig;
use crate::config::ConfigError;

use super::container::ContainerInfo;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportedAccount {
    pub name: String,
    pub address: String,
    pub mnemonic: String,
    pub pubkey: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RpcRunnerConfig {
    pub chain_config: ChainConfig,
    pub container: Option<ContainerInfo>,
    pub accounts_folder: String,
}

impl RpcRunnerConfig {
    pub fn bind_chain_to_container(&mut self, container: &Container<GenericImage>) {
        // We inject here the endpoint since containers have a life time
        self.chain_config.rpc_endpoint =
            format!("http://localhost:{}/", container.get_host_port_ipv4(26657));
        self.chain_config.grpc_endpoint =
            format!("http://localhost:{}/", container.get_host_port_ipv4(9090));
    }

    pub fn import_account(&self, name: &str) -> Result<SigningAccount, ConfigError> {
        let path = format!(
            "{}/{}/accounts.json",
            self.accounts_folder, self.chain_config.name
        );
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

    fn mnemonic_to_signing_key(
        mnemonic: &str,
        chain_cfg: &ChainConfig,
    ) -> Result<cosmrs::crypto::secp256k1::SigningKey, bip32::Error> {
        let seed = bip32::Mnemonic::new(mnemonic, bip32::Language::English)?.to_seed("");
        cosmrs::crypto::secp256k1::SigningKey::derive_from_path(
            seed,
            &chain_cfg.derivation_path.parse()?,
        )
    }
}
