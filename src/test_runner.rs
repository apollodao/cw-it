use std::str::FromStr;

use crate::{traits::CwItRunner, ContractType};
use cosmwasm_std::coin;
use serde::de::DeserializeOwned;
use test_tube::{Runner, SigningAccount};

#[cfg(feature = "rpc-runner")]
use crate::rpc_runner::RpcRunner;

#[cfg(feature = "multi-test")]
use crate::multi_test::MultiTestRunner;

#[cfg(feature = "osmosis-test-tube")]
use osmosis_test_tube::OsmosisTestApp;

/// An enum with concrete types implementing the Runner trait. We specify these here because the
/// Runner trait is not object safe, and we want to be able to run tests against different types of
/// runners.
pub enum TestRunner<'a> {
    // Needed to keep lifetime when rpc-runner feature is off
    PhantomData(&'a ()),
    #[cfg(feature = "osmosis-test-tube")]
    OsmosisTestApp(OsmosisTestApp),
    #[cfg(feature = "rpc-runner")]
    RpcRunner(RpcRunner<'a>),
    #[cfg(feature = "multi-test")]
    MultiTest(MultiTestRunner<'a>),
}

fn initial_coins() -> Vec<cosmwasm_std::Coin> {
    vec![
        coin(u128::MAX, "uosmo"),
        coin(u128::MAX, "uion"),
        coin(u128::MAX, "uatom"),
        coin(u128::MAX, "stake"),
    ]
}

impl FromStr for TestRunner<'_> {
    type Err = String;

    /// Returns a TestRunner from a string, which is the name of the runner. Useful for deciding
    /// which runner to use base on an env var or similar.
    ///
    /// NB: MultiTestRunner will use the "osmo" address prefix.
    /// RpcRunner is not supported in this function, as it requires a config file and optional
    /// docker Cli instance.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        #[allow(unreachable_code)]
        Ok(match s {
            #[cfg(feature = "osmosis-test-tube")]
            "osmosis-test-app" => Self::OsmosisTestApp(OsmosisTestApp::new()),
            #[cfg(feature = "rpc-runner")]
            "rpc-runner" => return Err("RpcRunner requires a config file".to_string()),
            #[cfg(feature = "multi-test")]
            "multi-test" => Self::MultiTest(MultiTestRunner::new("osmo")),
            _ => return Err(format!("Invalid TestRunner: {}", s)),
        })
    }
}

impl TestRunner<'_> {
    /// Initializes 10 accounts with max balance of uosmo, uion, uatom, and stake.
    ///
    /// NB: For RpcRunner, this will instead just read the mnemonics from the config file.
    pub fn init_accounts(&self) -> Vec<SigningAccount> {
        match self {
            TestRunner::PhantomData(_) => unreachable!(),
            #[cfg(feature = "osmosis-test-tube")]
            TestRunner::OsmosisTestApp(app) => app.init_accounts(&initial_coins(), 10).unwrap(),
            #[cfg(feature = "rpc-runner")]
            TestRunner::RpcRunner(runner) => runner.init_accounts(10).unwrap(),
            #[cfg(feature = "multi-test")]
            TestRunner::MultiTest(runner) => runner.init_accounts(&initial_coins(), 10).unwrap(),
        }
    }
}

#[cfg(feature = "osmosis-test-tube")]
impl From<OsmosisTestApp> for TestRunner<'_> {
    fn from(app: OsmosisTestApp) -> Self {
        Self::OsmosisTestApp(app)
    }
}

#[cfg(feature = "rpc-runner")]
impl<'a> From<RpcRunner<'a>> for TestRunner<'a> {
    fn from(runner: RpcRunner<'a>) -> Self {
        Self::RpcRunner(runner)
    }
}

#[cfg(feature = "multi-test")]
impl<'a> From<MultiTestRunner<'a>> for TestRunner<'a> {
    fn from(runner: MultiTestRunner<'a>) -> Self {
        Self::MultiTest(runner)
    }
}

impl<'a> Runner<'a> for TestRunner<'a> {
    fn execute_multiple<M, R>(
        &self,
        msgs: &[(M, &str)],
        signer: &test_tube::SigningAccount,
    ) -> test_tube::RunnerExecuteResult<R>
    where
        M: prost::Message,
        R: prost::Message + Default,
    {
        match self {
            TestRunner::PhantomData(_) => unimplemented!(),
            #[cfg(feature = "osmosis-test-tube")]
            TestRunner::OsmosisTestApp(app) => app.execute_multiple(msgs, signer),
            #[cfg(feature = "rpc-runner")]
            TestRunner::RpcRunner(runner) => runner.execute_multiple(msgs, signer),
            #[cfg(feature = "multi-test")]
            TestRunner::MultiTest(runner) => runner.execute_multiple(msgs, signer),
        }
    }

    fn execute_multiple_raw<R>(
        &self,
        msgs: Vec<cosmrs::Any>,
        signer: &test_tube::SigningAccount,
    ) -> test_tube::RunnerExecuteResult<R>
    where
        R: prost::Message + Default,
    {
        match self {
            TestRunner::PhantomData(_) => unimplemented!(),
            #[cfg(feature = "osmosis-test-tube")]
            TestRunner::OsmosisTestApp(app) => app.execute_multiple_raw(msgs, signer),
            #[cfg(feature = "rpc-runner")]
            TestRunner::RpcRunner(runner) => runner.execute_multiple_raw(msgs, signer),
            #[cfg(feature = "multi-test")]
            TestRunner::MultiTest(runner) => runner.execute_multiple_raw(msgs, signer),
        }
    }

    fn query<Q, R>(&self, path: &str, query: &Q) -> test_tube::RunnerResult<R>
    where
        Q: prost::Message,
        R: prost::Message + DeserializeOwned + Default,
    {
        match self {
            TestRunner::PhantomData(_) => unimplemented!(),
            #[cfg(feature = "osmosis-test-tube")]
            TestRunner::OsmosisTestApp(app) => app.query(path, query),
            #[cfg(feature = "rpc-runner")]
            TestRunner::RpcRunner(runner) => runner.query(path, query),
            #[cfg(feature = "multi-test")]
            TestRunner::MultiTest(runner) => runner.query(path, query),
        }
    }
}

impl<'a> CwItRunner<'a> for TestRunner<'a> {
    fn store_code(
        &self,
        code: ContractType,
        signer: &SigningAccount,
    ) -> Result<u64, anyhow::Error> {
        match self {
            TestRunner::PhantomData(_) => unimplemented!(),
            #[cfg(feature = "osmosis-test-tube")]
            TestRunner::OsmosisTestApp(app) => app.store_code(code, signer),
            #[cfg(feature = "rpc-runner")]
            TestRunner::RpcRunner(runner) => runner.store_code(code, signer),
            #[cfg(feature = "multi-test")]
            TestRunner::MultiTest(runner) => runner.store_code(code, signer),
        }
    }

    fn init_account(
        &self,
        initial_balance: &[cosmwasm_std::Coin],
    ) -> Result<SigningAccount, anyhow::Error> {
        match self {
            TestRunner::PhantomData(_) => unimplemented!(),
            #[cfg(feature = "osmosis-test-tube")]
            TestRunner::OsmosisTestApp(app) => Ok(app.init_account(initial_balance)?),
            #[cfg(feature = "rpc-runner")]
            TestRunner::RpcRunner(runner) => runner.init_account(0),
            #[cfg(feature = "multi-test")]
            TestRunner::MultiTest(runner) => runner.init_account(initial_balance),
        }
    }

    fn init_accounts(
        &self,
        initial_balance: &[cosmwasm_std::Coin],
        num_accounts: usize,
    ) -> Result<Vec<SigningAccount>, anyhow::Error> {
        match self {
            TestRunner::PhantomData(_) => unimplemented!(),
            #[cfg(feature = "osmosis-test-tube")]
            TestRunner::OsmosisTestApp(app) => {
                Ok(app.init_accounts(initial_balance, num_accounts as u64)?)
            }
            #[cfg(feature = "rpc-runner")]
            TestRunner::RpcRunner(runner) => runner.init_accounts(num_accounts),
            #[cfg(feature = "multi-test")]
            TestRunner::MultiTest(runner) => runner.init_accounts(initial_balance, num_accounts),
        }
    }

    fn increase_time(&self, seconds: u64) -> Result<(), anyhow::Error> {
        match self {
            TestRunner::PhantomData(_) => unimplemented!(),
            #[cfg(feature = "osmosis-test-tube")]
            TestRunner::OsmosisTestApp(app) => {
                app.increase_time(seconds);
                Ok(())
            }
            #[cfg(feature = "rpc-runner")]
            TestRunner::RpcRunner(runner) => runner.increase_time(seconds),
            #[cfg(feature = "multi-test")]
            TestRunner::MultiTest(runner) => runner.increase_time(seconds),
        }
    }

    fn query_block_time_nanos(&self) -> u64 {
        match self {
            TestRunner::PhantomData(_) => unimplemented!(),
            #[cfg(feature = "osmosis-test-tube")]
            TestRunner::OsmosisTestApp(app) => app.query_block_time_nanos(),
            #[cfg(feature = "rpc-runner")]
            TestRunner::RpcRunner(runner) => unimplemented!(),
            #[cfg(feature = "multi-test")]
            TestRunner::MultiTest(runner) => runner.query_block_time_nanos(),
        }
    }
}

mod tests {
    #[test]
    fn test_increase_time() {
        #[cfg(feature = "osmosis-test-tube")]
        {
            use crate::traits::CwItRunner;
            let runner = crate::TestRunner::OsmosisTestApp(
                crate::osmosis_test_tube::OsmosisTestApp::default(),
            );
            let time = runner.query_block_time_nanos();
            runner.increase_time(100).unwrap();
            assert_eq!(runner.query_block_time_nanos(), time + 100_000_000_000);
        }
        #[cfg(feature = "multi-test")]
        {
            use crate::traits::CwItRunner;
            let runner =
                crate::TestRunner::MultiTest(crate::multi_test::MultiTestRunner::new("osmo"));
            let time = runner.query_block_time_nanos();
            runner.increase_time(100).unwrap();
            assert_eq!(runner.query_block_time_nanos(), time + 100_000_000_000);
        }
    }
}
