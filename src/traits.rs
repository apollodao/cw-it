use anyhow::Error;
use cosmwasm_std::coin;
use cosmwasm_std::Coin;
use test_tube::Runner;
use test_tube::SigningAccount;

use crate::artifact::ContractType;

// Some very high number smaller than u128::MAX, to allow for receiving some coins without overflow.
pub const DEFAULT_COIN_AMOUNT: u128 = 1_000_000_000_000_000_000_000_000u128;

/// Returns a list of coins to initialize testing accounts.
pub fn initial_coins() -> Vec<cosmwasm_std::Coin> {
    vec![
        coin(DEFAULT_COIN_AMOUNT, "uosmo"),
        coin(DEFAULT_COIN_AMOUNT, "uion"),
        coin(DEFAULT_COIN_AMOUNT, "uatom"),
        coin(DEFAULT_COIN_AMOUNT, "stake"),
        coin(DEFAULT_COIN_AMOUNT, "denom0"),
        coin(DEFAULT_COIN_AMOUNT, "denom1"),
        coin(DEFAULT_COIN_AMOUNT, "denom3"),
        coin(DEFAULT_COIN_AMOUNT, "denom4"),
        coin(DEFAULT_COIN_AMOUNT, "denom5"),
        coin(DEFAULT_COIN_AMOUNT, "denom6"),
        coin(DEFAULT_COIN_AMOUNT, "denom7"),
        coin(DEFAULT_COIN_AMOUNT, "denom8"),
        coin(DEFAULT_COIN_AMOUNT, "denom9"),
        coin(DEFAULT_COIN_AMOUNT, "denom10"),
    ]
}

pub trait CwItRunner<'a>: Runner<'a> {
    /// Store the code on the chain and return the code ID. Takes a ContractType to allow for
    /// both wasm artifacts and multi-test contracts.
    fn store_code(&self, code: ContractType, signer: &SigningAccount) -> Result<u64, Error>;

    /// Initialize 10 accounts with the default balances.
    fn init_default_accounts(&self) -> Result<Vec<SigningAccount>, Error> {
        self.init_accounts(&initial_coins(), 10)
    }

    /// Initialize a single account with the default balances.
    fn init_default_account(&self) -> Result<SigningAccount, Error> {
        self.init_account(&initial_coins())
    }

    /// Initialize a single account with the given balance.
    fn init_account(&self, initial_balance: &[Coin]) -> Result<SigningAccount, Error>;

    /// Initialize the given number of accounts each with the same, specified initial
    /// balance of coins.
    fn init_accounts(
        &self,
        initial_balance: &[Coin],
        num_accounts: usize,
    ) -> Result<Vec<SigningAccount>, Error>;

    /// Increases the time of the blockchain by the given number of seconds.
    fn increase_time(&self, seconds: u64) -> Result<(), Error>;

    /// Returns the current block time in nanoseconds.
    fn query_block_time_nanos(&self) -> u64;
}
