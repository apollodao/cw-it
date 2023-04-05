#[cfg(feature = "rpc-runner")]
use crate::rpc_runner::RpcRunner;

#[cfg(feature = "multi-test")]
use crate::multi_test::MultiTestRunner;

use crate::{traits::CwItRunner, ContractType};

use cosmwasm_std::coin;
use osmosis_test_tube::{OsmosisTestApp, Runner, SigningAccount};
use serde::de::DeserializeOwned;

/// An enum with concrete types implementing the Runner trait. We specify these here because the
/// Runner trait is not object safe, and we want to be able to run tests against different types of
/// runners.
pub enum TestRunner<'a> {
    OsmosisTestApp(OsmosisTestApp),
    // Needed to keep lifetime when rpc-runner feature is off
    PhantomData(&'a ()),
    #[cfg(feature = "rpc-runner")]
    RpcRunner(RpcRunner<'a>),
    #[cfg(feature = "multi-test")]
    MultiTest(MultiTestRunner<'a>),
}

impl Default for TestRunner<'_> {
    fn default() -> Self {
        Self::OsmosisTestApp(OsmosisTestApp::default())
    }
}

fn initial_coins() -> Vec<cosmwasm_std::Coin> {
    vec![
        coin(u128::MAX, "uosmo"),
        coin(u128::MAX, "uion"),
        coin(u128::MAX, "uatom"),
        coin(u128::MAX, "stake"),
    ]
}

impl TestRunner<'_> {
    /// Initializes 10 accounts with max balance of uosmo, uion, uatom, and stake.
    ///
    /// NB: For RpcRunner, this will instead just read the mnemonics from the config file.
    pub fn init_accounts(&self) -> Vec<SigningAccount> {
        match self {
            TestRunner::OsmosisTestApp(app) => app.init_accounts(&initial_coins(), 10).unwrap(),
            #[cfg(feature = "rpc-runner")]
            TestRunner::RpcRunner(runner) => runner.init_accounts(10).unwrap(),
            #[cfg(feature = "multi-test")]
            TestRunner::MultiTest(runner) => runner.init_accounts(&initial_coins(), 10).unwrap(),
            TestRunner::PhantomData(_) => unreachable!(),
        }
    }
}

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

impl<'a> Runner<'a> for TestRunner<'a> {
    fn execute_multiple<M, R>(
        &self,
        msgs: &[(M, &str)],
        signer: &osmosis_test_tube::SigningAccount,
    ) -> osmosis_test_tube::RunnerExecuteResult<R>
    where
        M: prost::Message,
        R: prost::Message + Default,
    {
        match self {
            TestRunner::OsmosisTestApp(app) => app.execute_multiple(msgs, signer),
            #[cfg(feature = "rpc-runner")]
            TestRunner::RpcRunner(runner) => runner.execute_multiple(msgs, signer),
            #[cfg(feature = "multi-test")]
            TestRunner::MultiTest(runner) => runner.execute_multiple(msgs, signer),
            TestRunner::PhantomData(_) => unimplemented!(),
        }
    }

    fn execute_multiple_raw<R>(
        &self,
        msgs: Vec<cosmrs::Any>,
        signer: &osmosis_test_tube::SigningAccount,
    ) -> osmosis_test_tube::RunnerExecuteResult<R>
    where
        R: prost::Message + Default,
    {
        match self {
            TestRunner::OsmosisTestApp(app) => app.execute_multiple_raw(msgs, signer),
            #[cfg(feature = "rpc-runner")]
            TestRunner::RpcRunner(runner) => runner.execute_multiple_raw(msgs, signer),
            #[cfg(feature = "multi-test")]
            TestRunner::MultiTest(runner) => runner.execute_multiple_raw(msgs, signer),
            TestRunner::PhantomData(_) => unimplemented!(),
        }
    }

    fn query<Q, R>(&self, path: &str, query: &Q) -> osmosis_test_tube::RunnerResult<R>
    where
        Q: prost::Message,
        R: prost::Message + DeserializeOwned + Default,
    {
        match self {
            TestRunner::OsmosisTestApp(app) => app.query(path, query),
            #[cfg(feature = "rpc-runner")]
            TestRunner::RpcRunner(runner) => runner.query(path, query),
            #[cfg(feature = "multi-test")]
            TestRunner::MultiTest(runner) => runner.query(path, query),
            TestRunner::PhantomData(_) => unimplemented!(),
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
            TestRunner::OsmosisTestApp(app) => app.store_code(code, signer),
            #[cfg(feature = "rpc-runner")]
            TestRunner::RpcRunner(runner) => runner.store_code(code, signer),
            #[cfg(feature = "multi-test")]
            TestRunner::MultiTest(runner) => runner.store_code(code, signer),
            TestRunner::PhantomData(_) => unimplemented!(),
        }
    }

    fn init_account(
        &self,
        initial_balance: &[cosmwasm_std::Coin],
    ) -> Result<SigningAccount, anyhow::Error> {
        match self {
            TestRunner::OsmosisTestApp(app) => Ok(app.init_account(initial_balance)?),
            #[cfg(feature = "rpc-runner")]
            TestRunner::RpcRunner(runner) => runner.init_account(0),
            #[cfg(feature = "multi-test")]
            TestRunner::MultiTest(runner) => runner.init_account(initial_balance),
            TestRunner::PhantomData(_) => unimplemented!(),
        }
    }

    fn init_accounts(
        &self,
        initial_balance: &[cosmwasm_std::Coin],
        num_accounts: usize,
    ) -> Result<Vec<SigningAccount>, anyhow::Error> {
        match self {
            TestRunner::OsmosisTestApp(app) => {
                Ok(app.init_accounts(initial_balance, num_accounts as u64)?)
            }
            #[cfg(feature = "rpc-runner")]
            TestRunner::RpcRunner(runner) => runner.init_accounts(num_accounts),
            #[cfg(feature = "multi-test")]
            TestRunner::MultiTest(runner) => runner.init_accounts(initial_balance, num_accounts),
            TestRunner::PhantomData(_) => unimplemented!(),
        }
    }
}