use cosmwasm_std::{Coin, Uint128};
use test_tube::{Bank, Module, Runner, SigningAccount, Wasm};

use crate::helpers::{bank_balance_query, bank_send};

/// Implements a collection of common interactions with a `Runner`, that are all applicable to any
/// cosmos chain.
pub trait TestRobot<'a, R: Runner<'a> + 'a> {
    fn app(&self) -> &'a R;

    fn wasm(&self) -> Wasm<'a, R> {
        Wasm::new(self.app())
    }

    fn bank(&self) -> Bank<'a, R> {
        Bank::new(self.app())
    }

    fn query_native_token_balance(
        &self,
        account: impl Into<String>,
        denom: impl Into<String>,
    ) -> Uint128 {
        bank_balance_query(self.app(), account.into(), denom.into()).unwrap()
    }

    fn assert_native_token_balance_eq(
        &self,
        account: impl Into<String>,
        denom: impl Into<String>,
        expected: impl Into<Uint128>,
    ) -> &Self {
        let actual = self.query_native_token_balance(account, denom);
        assert_eq!(actual, expected.into());

        self
    }

    fn assert_native_token_balance_gt(
        &self,
        account: impl Into<String>,
        denom: impl Into<String>,
        expected: impl Into<Uint128>,
    ) -> &Self {
        let actual = self.query_native_token_balance(account, denom);
        assert!(actual > expected.into());

        self
    }

    fn assert_native_token_balance_lt(
        &self,
        account: impl Into<String>,
        denom: impl Into<String>,
        expected: impl Into<Uint128>,
    ) -> &Self {
        let actual = self.query_native_token_balance(account, denom);
        assert!(actual < expected.into());

        self
    }

    fn send_native_tokens(
        &self,
        from: &SigningAccount,
        to: impl Into<String>,
        amount: impl Into<Uint128>,
        denom: impl Into<String>,
    ) -> &Self {
        let coin = Coin {
            amount: amount.into(),
            denom: denom.into(),
        };
        bank_send(self.app(), from, &to.into(), vec![coin]).unwrap();

        self
    }
}

// We feature-gate theses tests because they use OsmosisTestApp
#[cfg(feature = "osmosis")]
#[cfg(test)]
mod tests {
    use osmosis_test_tube::{Account, OsmosisTestApp};

    use super::*;

    struct OsmosisTestAppRobot<'a>(&'a OsmosisTestApp);

    impl<'a> TestRobot<'a, OsmosisTestApp> for OsmosisTestAppRobot<'a> {
        fn app(&self) -> &'a OsmosisTestApp {
            self.0
        }
    }

    #[test]
    fn test_query_native_token_balance() {
        let app = OsmosisTestApp::new();
        let robot = OsmosisTestAppRobot(&app);

        let account = app
            .init_account(&[Coin::new(100_000_000_000_000_000u128, "uatom")])
            .unwrap();

        let balance = robot.query_native_token_balance(account.address(), "uatom");
        assert_eq!(balance, Uint128::from(100_000_000_000_000_000u128));

        let balance = robot.query_native_token_balance(account.address(), "uosmo");
        assert_eq!(balance, Uint128::zero());
    }

    #[test]
    fn test_send_native_tokens() {
        let app = OsmosisTestApp::new();
        let robot = OsmosisTestAppRobot(&app);

        let account1 = app
            .init_account(&[Coin::new(100_000_000_000_000_000u128, "uatom")])
            .unwrap();
        let account2 = app.init_account(&[]).unwrap();

        robot
            .send_native_tokens(
                &account1,
                account2.address(),
                1_000_000_000_000_000u128,
                "uatom",
            )
            .assert_native_token_balance_eq(account2.address(), "uatom", 1_000_000_000_000_000u128)
            .assert_native_token_balance_eq(
                account1.address(),
                "uatom",
                99_000_000_000_000_000u128,
            );
    }
}
