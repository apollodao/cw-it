use std::{collections::HashMap, str::FromStr};

use cosmrs::proto::cosmos::bank::v1beta1::QueryBalanceRequest;
use cosmwasm_std::{StdError, StdResult, Uint128};
use osmosis_testing::{Bank, Module, Runner, SigningAccount, Wasm};

use crate::config::TestConfig;

pub fn upload_wasm_files<'a, R: Runner<'a>>(
    runner: &'a R,
    signer: &SigningAccount,
    config: TestConfig,
) -> StdResult<HashMap<String, u64>> {
    let wasm = Wasm::new(runner);
    config
        .contracts
        .into_iter()
        .map(|(name, contract)| {
            let wasm_file_path = format!("{}/{}", config.artifacts_folder, contract.artifact);
            println!("Uploading wasm file: {}", wasm_file_path);
            let wasm_byte_code = std::fs::read(wasm_file_path).unwrap();
            let code_id = wasm
                .store_code(&wasm_byte_code, None, signer)
                .map_err(|e| StdError::generic_err(format!("{:?}", e)))?
                .data
                .code_id;
            Ok((name, code_id))
        })
        .collect()
}

pub fn bank_balance_query<'a>(
    runner: &'a impl Runner<'a>,
    address: String,
    denom: String,
) -> StdResult<Uint128> {
    Bank::new(runner)
        .query_balance(&QueryBalanceRequest { address, denom })
        .unwrap()
        .balance
        .map(|c| Uint128::from_str(&c.amount).unwrap())
        .ok_or(StdError::generic_err("Bank balance query failed"))
}
