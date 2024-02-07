use config::Config;
use cosmwasm_std::Coin;
use serde::{Deserialize, Serialize};

use super::chain::ChainConfig;
use crate::helpers::get_current_working_dir;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportedAccount {
    pub name: String,
    pub address: String,
    pub mnemonic: String,
    pub pubkey: String,
}

/// This enum exactly matches the `FeeSetting` enum in `test-tube` and is only needed
/// because `test_tube::account::FeeSetting` does not derive `Deserialize`
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum FeeSetting {
    Auto {
        gas_price: Coin,
        gas_adjustment: f64,
    },
    Custom {
        amount: Coin,
        gas_limit: u64,
    },
}

impl From<FeeSetting> for test_tube::account::FeeSetting {
    fn from(value: FeeSetting) -> Self {
        match value {
            FeeSetting::Auto {
                gas_price,
                gas_adjustment,
            } => test_tube::account::FeeSetting::Auto {
                gas_price: gas_price.into(),
                gas_adjustment,
            },
            FeeSetting::Custom { amount, gas_limit } => test_tube::account::FeeSetting::Custom {
                amount: amount.into(),
                gas_limit,
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct RpcRunnerConfig {
    pub chain_config: ChainConfig,
    pub funding_account_mnemonic: String,
    pub fee_setting: Option<FeeSetting>,
}

impl RpcRunnerConfig {
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
