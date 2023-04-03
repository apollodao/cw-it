use anyhow::Error;
use cosmwasm_std::Empty;
use cw_multi_test::Contract;
use test_tube::Runner;
use test_tube::SigningAccount;

use crate::artifact::Artifact;

pub enum ContractType {
    Artifact(Artifact),
    MultiTestContract(Box<dyn Contract<Empty, Empty>>),
}

pub trait WasmRunner<'a>: Runner<'a> {
    fn store_code(&self, code: ContractType, signer: &SigningAccount) -> Result<u64, Error>;
}
