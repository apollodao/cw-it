use anyhow::Error;
use cosmwasm_std::Coin;
use test_tube::Runner;
use test_tube::SigningAccount;

use crate::artifact::ContractType;

pub trait CwItRunner<'a>: Runner<'a> {
    fn store_code(&self, code: ContractType, signer: &SigningAccount) -> Result<u64, Error>;

    fn init_account(&self, initial_balance: &[Coin]) -> Result<SigningAccount, Error>;

    fn init_accounts(
        &self,
        initial_balance: &[Coin],
        num_accounts: usize,
    ) -> Result<Vec<SigningAccount>, Error>;

    /// Increases the time of the blockchain by the given number of seconds.
    fn increase_time(&self, seconds: u64) -> Result<(), Error>;
}
