use anyhow::Error;
use cosmwasm_std::Coin;
use cosmwasm_std::Empty;
use cw_multi_test::Contract;
use test_tube::Runner;
use test_tube::SigningAccount;

use crate::artifact::Artifact;

pub enum ContractType {
    Artifact(Artifact),
    MultiTestContract(Box<dyn Contract<Empty, Empty>>),
}

pub trait CwItRunner<'a>: Runner<'a> {
    fn store_code(&self, code: ContractType, signer: &SigningAccount) -> Result<u64, Error>;

    fn init_account(&self, initial_balance: &[Coin]) -> Result<SigningAccount, Error>;

    fn init_accounts(
        &self,
        initial_balance: &[Coin],
        num_accounts: usize,
    ) -> Result<Vec<SigningAccount>, Error>;
}
