use std::{collections::HashMap, str::FromStr};

use cosmrs::proto::cosmos::bank::v1beta1::QueryBalanceRequest;
use cosmwasm_std::{Coin, StdError, StdResult, Uint128};
use osmosis_testing::{Account, Bank, Module, Runner, RunnerResult, SigningAccount, Wasm};
use serde::Serialize;

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
            let wasm_byte_code = std::fs::read(wasm_file_path)?;
            let code_id = wasm
                .store_code(&wasm_byte_code, None, signer)
                .map_err(|e| StdError::generic_err(format!("{:?}", e)))?
                .data
                .code_id;
            Ok((name, code_id))
        })
        .collect()
}

/// Instantiates the liquidity helper contract
pub fn instantiate_contract_with_funds<'a, R, M, S>(
    app: &'a R,
    admin: &SigningAccount,
    code_id: u64,
    instantite_msg: &M,
    funds: &[Coin],
) -> RunnerResult<S>
where
    R: Runner<'a>,
    M: Serialize,
    S: From<String>,
{
    let wasm = Wasm::new(app);

    // Instantiate the contract
    println!("Instantiating contract with code id {}", code_id);
    wasm.instantiate(
        code_id,
        instantite_msg,
        Some(&admin.address()), // contract admin used for migration
        None,
        funds,
        admin, // signer
    )
    .map(|r| r.data.address.into())
}

pub fn instantiate_contract<'a, R, M, S>(
    app: &'a R,
    admin: &SigningAccount,
    code_id: u64,
    instantite_msg: &M,
) -> RunnerResult<S>
where
    R: Runner<'a>,
    M: Serialize,
    S: From<String>,
{
    instantiate_contract_with_funds(app, admin, code_id, instantite_msg, &[])
}

pub fn bank_balance_query<'a>(
    runner: &'a impl Runner<'a>,
    address: String,
    denom: String,
) -> RunnerResult<Uint128> {
    Bank::new(runner)
        .query_balance(&QueryBalanceRequest { address, denom })?
        .balance
        .map(|c| Uint128::from_str(&c.amount).unwrap())
        .ok_or(osmosis_testing::RunnerError::StdError(StdError::generic_err("Bank balance query failed")))
}
