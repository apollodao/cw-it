use std::env;
use std::fmt::Debug;
use std::{collections::HashMap, str::FromStr};

use cosmwasm_std::{Coin, StdError, StdResult, Uint128};
use osmosis_std::types::cosmos::bank::v1beta1::{
    MsgSend, MsgSendResponse, QueryAllBalancesRequest, QueryAllBalancesResponse,
    QueryBalanceRequest,
};
use osmosis_std::types::cosmos::base::query::v1beta1::PageRequest;
use osmosis_std::types::cosmos::base::v1beta1::Coin as ProtoCoin;
use serde::Serialize;
use test_tube::{Account, Module, Runner, RunnerExecuteResult, RunnerResult, SigningAccount};
use test_tube::{Bank, Wasm};

use crate::error::CwItError;
use crate::traits::CwItRunner;
use crate::{ArtifactError, ContractType};

#[cfg(feature = "tokio")]
use std::future::Future;
#[cfg(feature = "tokio")]
pub fn block_on<F: Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
}

pub fn upload_wasm_files<'a, R: CwItRunner<'a>>(
    runner: &'a R,
    signer: &SigningAccount,
    contracts: HashMap<String, ContractType>,
) -> Result<HashMap<String, u64>, CwItError> {
    contracts
        .into_iter()
        .map(|(name, contract)| {
            let code_id = upload_wasm_file(runner, signer, contract)?;
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

/// Uploads a wasm file to the chain and returns the code_id
pub fn upload_wasm_file<'a, R: CwItRunner<'a>>(
    runner: &'a R,
    signer: &SigningAccount,
    contract: ContractType,
) -> Result<u64, CwItError> {
    let error_msg = format!("Failed to upload wasm file: {:?}", contract);
    runner.store_code(contract, signer).map_err(|e| {
        CwItError::ArtifactError(ArtifactError::Generic(format!(
            "{:?}. Error: {:?}",
            error_msg, e
        )))
    })
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
        .ok_or_else(|| StdError::generic_err("Bank balance query failed"))
}

pub fn bank_all_balances_query<'a>(
    runner: &'a impl Runner<'a>,
    address: String,
    pagination: Option<PageRequest>,
) -> StdResult<QueryAllBalancesResponse> {
    Bank::new(runner)
        .query_all_balances(&QueryAllBalancesRequest {
            address,
            pagination,
        })
        .map_err(|_| StdError::generic_err("Bank all balances query failed"))
}

pub fn bank_send<'a>(
    runner: &'a impl Runner<'a>,
    sender: &SigningAccount,
    recipient: &str,
    coins: Vec<Coin>,
) -> RunnerExecuteResult<MsgSendResponse> {
    let bank = Bank::new(runner);
    bank.send(
        MsgSend {
            from_address: sender.address(),
            to_address: recipient.to_string(),
            amount: coins
                .iter()
                .map(|c| ProtoCoin {
                    denom: c.denom.clone(),
                    amount: c.amount.to_string(),
                })
                .collect(),
        },
        sender,
    )
}

pub fn get_current_working_dir() -> String {
    let res = env::current_dir();
    match res {
        Ok(path) => path.into_os_string().into_string().unwrap(),
        Err(_) => "FAILED".to_string(),
    }
}

/// An enum to choose which type of unwrap to use. When using `Unwrap::Err`, the
/// result must be an `Err` or the test will panic. If the result contains an
/// `Err`, the test will pass only if the error message contains the provided
/// string.
pub enum Unwrap {
    Ok,
    Err(&'static str),
}

impl Unwrap {
    pub fn unwrap<T: Debug, E: Debug>(self, result: Result<T, E>) -> Option<T> {
        match self {
            Unwrap::Ok => {
                let res = result.unwrap();
                Some(res)
            }
            Unwrap::Err(s) => {
                let err = result.unwrap_err();
                assert!(
                    format!("{:?}", err).contains(s),
                    "Expected error message to contain {:?}, got {:?}",
                    s,
                    err
                );
                None
            }
        }
    }
}

#[test]
fn test_unwrap() {
    let res: Result<u32, &str> = Ok(5);
    assert_eq!(Unwrap::Ok.unwrap(res), Some(5));

    let res: Result<u32, &str> = Err("test");
    assert_eq!(Unwrap::Err("test").unwrap(res), None);

    let res: Result<u32, &str> = Err("test2");
    assert_eq!(Unwrap::Err("test").unwrap(res), None);
}

#[test]
#[should_panic(expected = "Expected error message to contain \"test\", got \"random\"")]
fn test_unwrap_panic() {
    let res: Result<u32, &str> = Err("random");
    Unwrap::Err("test").unwrap(res);
}
