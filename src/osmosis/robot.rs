use crate::osmosis::utils::is_osmosis_lp_token;
use cosmwasm_std::Coin;
use osmosis_std::{
    shim::Duration,
    types::osmosis::{
        gamm::v1beta1::{
            MsgJoinSwapExternAmountIn, MsgJoinSwapExternAmountInResponse, MsgSwapExactAmountIn,
            MsgSwapExactAmountInResponse, SwapAmountInRoute,
        },
        lockup::{MsgLockTokens, MsgLockTokensResponse},
    },
};
use osmosis_test_tube::{Account, OsmosisTestApp, Runner, SigningAccount};

use crate::robot::TestRobot;

/// Implements a collection of common interactions with the `OsmosisTestApp`, that are
/// specific to the osmosis chain
pub trait OsmosisTestRobot<'a>: TestRobot<'a, OsmosisTestApp> {
    /// Increases the block time by the given number of seconds
    ///
    /// ## Args:
    ///   - `seconds`: The number of seconds to increase the block time by
    fn increase_time(&self, seconds: u64) -> &Self {
        self.app().increase_time(seconds);
        self
    }

    /// Whitelists an address for force unlocking
    /// ## Args:
    ///   - `address`: The address to whitelist
    fn whitelist_address_for_force_unlock(&self, address: impl Into<String>) -> &Self {
        self.app()
            .whitelist_address_for_force_unlock(&address.into());
        self
    }

    /// Whitelists a denom as a superfluid LP share
    /// ## Args:
    ///   - `denom`: The LP share denom to whitelist
    fn whitelist_superfluid_lp_share(&self, denom: impl Into<String>) -> &Self {
        let denom = denom.into();
        if !is_osmosis_lp_token(&denom) {
            panic!("Denom must be an LP share to be whitelisted as a superfluid LP share");
        }
        self.app().add_superfluid_lp_share(&denom);
        self
    }

    /// Provide single sided liquidity to a pool. If the resulting number of LP shares is less than
    /// `min_out`, the transaction will fail. If `min_out` is `None`, the minimum amount of LP shares
    /// is set to 1. This is required by Osmosis which does not allow min_out to be non-positive.
    /// ## Args:
    ///   - `pool_id`: The pool ID
    ///   - `coins`: The coins to provide
    ///   - `min_out`: The minimum amount of LP shares to receive
    ///   - `signer`: The account to provide liquidity from
    fn join_pool_swap_extern_amount_in(
        &self,
        signer: &SigningAccount,
        pool_id: u64,
        coin: Coin,
        min_out: Option<u128>,
    ) -> &Self {
        // Min out must be at least 1 on osmosis.
        let min_out = min_out.unwrap_or(1);
        let msg = MsgJoinSwapExternAmountIn {
            pool_id,
            token_in: Some(coin.into()),
            sender: signer.address(),
            share_out_min_amount: format!("{min_out}"),
        };

        self.app()
            .execute::<_, MsgJoinSwapExternAmountInResponse>(
                msg,
                MsgJoinSwapExternAmountIn::TYPE_URL,
                signer,
            )
            .unwrap();
        self
    }

    /// Swap an exact amount of tokens for another token in a given pool. If the resulting amount of tokens is less
    /// than `min_out`, the transaction will fail. If `min_out` is `None`, the minimum amount of
    /// tokens is set to 1. This is required by Osmosis which does not allow min_out to be non-positive.
    /// If either of the tokens are not in the pool, the transaction will fail.
    /// ## Args:
    ///  - `signer`: The account to swap from
    ///  - `pool_id`: The pool ID of the pool to swap in
    ///  - `token_in`: The token to swap in
    ///  - `token_out_denom`: The denom of token to swap to
    fn swap_exact_amount_in(
        &self,
        signer: &SigningAccount,
        pool_id: u64,
        token_in: Coin,
        token_out_denom: impl Into<String>,
        min_out: Option<u128>,
    ) -> &Self {
        // Min out must be at least 1 on osmosis.
        let min_out = min_out.unwrap_or(1);

        let msg = MsgSwapExactAmountIn {
            routes: vec![SwapAmountInRoute {
                pool_id,
                token_out_denom: token_out_denom.into(),
            }],
            token_in: Some(token_in.into()),
            sender: signer.address(),
            token_out_min_amount: format!("{min_out}"),
        };

        self.app()
            .execute::<_, MsgSwapExactAmountInResponse>(msg, MsgSwapExactAmountIn::TYPE_URL, signer)
            .unwrap();

        self
    }

    /// Locks LP shares for a given duration in the osmosis lockup module
    fn lock_tokens(&self, signer: &SigningAccount, coin: Coin, duration: u32) -> &Self {
        if !is_osmosis_lp_token(&coin.denom) {
            panic!("Only LP shares can be locked");
        }

        let msg = MsgLockTokens {
            coins: vec![coin.into()],
            duration: Some(Duration {
                seconds: duration as i64,
                nanos: 0,
            }),
            owner: signer.address(),
        };

        self.app()
            .execute::<_, MsgLockTokensResponse>(msg, MsgLockTokens::TYPE_URL, signer)
            .unwrap();
        self
    }
}

#[cfg(test)]
mod tests {
    use apollo_utils::iterators::IntoElementwise;
    use cosmwasm_std::testing::mock_env;
    use cosmwasm_std::{Coin, Uint128};
    use osmosis_test_tube::{Gamm, Module, OsmosisTestApp};

    use crate::const_coin::ConstCoin;

    use super::*;

    struct TestingRobot<'a>(&'a OsmosisTestApp);

    impl<'a> TestRobot<'a, OsmosisTestApp> for TestingRobot<'a> {
        fn app(&self) -> &'a OsmosisTestApp {
            &self.0
        }
    }

    impl<'a> OsmosisTestRobot<'a> for TestingRobot<'a> {}

    const INITIAL_BALANCES: &[ConstCoin] = &[
        ConstCoin::new(100_000_000_000_000_000u128, "uatom"),
        ConstCoin::new(100_000_000_000_000_000u128, "uosmo"),
    ];

    #[test]
    fn test_join_pool_swap_extern_amount_in() {
        let app = OsmosisTestApp::new();
        let account = app
            .init_account(&INITIAL_BALANCES.into_elementwise())
            .unwrap();
        let gamm = Gamm::new(&app);
        let pool_id = gamm
            .create_basic_pool(
                &[
                    Coin::new(1_000_000_000, "uosmo"),
                    Coin::new(1_000_000_000, "uatom"),
                ],
                &account,
            )
            .unwrap()
            .data
            .pool_id;

        let robot = TestingRobot(&app);
        robot
            .join_pool_swap_extern_amount_in(
                &account,
                pool_id,
                Coin::new(1_000_000_000, "uosmo"),
                None,
            )
            .assert_native_token_balance_gt(
                account.address(),
                format!("gamm/pool/{pool_id}"),
                0u128,
            );
    }

    #[test]
    fn test_swap_exact_amount_in() {
        let app = OsmosisTestApp::new();
        let account1 = app
            .init_account(&INITIAL_BALANCES.into_elementwise())
            .unwrap();
        let account2 = app
            .init_account(&INITIAL_BALANCES.into_elementwise())
            .unwrap();

        let initial_balance = INITIAL_BALANCES
            .iter()
            .find(|c| c.denom == "uatom")
            .unwrap()
            .amount
            .u128();

        let gamm = Gamm::new(&app);
        let pool_id = gamm
            .create_basic_pool(
                &[
                    Coin::new(1_000_000_000, "uosmo"),
                    Coin::new(1_000_000_000, "uatom"),
                ],
                &account1,
            )
            .unwrap()
            .data
            .pool_id;

        let swap_amount = 1_000_000_000u128;

        let robot = TestingRobot(&app);
        robot
            .swap_exact_amount_in(
                &account2,
                pool_id,
                Coin::new(swap_amount, "uosmo"),
                "uatom",
                None,
            )
            .assert_native_token_balance_eq(
                // We should have swapped swap_amount of our uosmo
                account2.address(),
                "uosmo",
                initial_balance - swap_amount,
            )
            .assert_native_token_balance_gt(
                // We should have more than the initial balance
                account2.address(),
                "uatom",
                initial_balance,
            )
            .assert_native_token_balance_lt(
                // But less than the initial balance + swap amount due to slippage and a balanced pool
                account2.address(),
                "uatom",
                initial_balance + swap_amount,
            );
    }
}
