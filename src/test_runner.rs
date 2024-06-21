use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use crate::{traits::CwItRunner, ContractType};
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
#[derive(strum::EnumVariantNames)]
#[strum(serialize_all = "kebab_case")]
pub enum OwnedTestRunner<'a> {
    // Needed to keep lifetime when rpc-runner and multitest features are off
    PhantomData(&'a ()),
    #[cfg(feature = "osmosis-test-tube")]
    OsmosisTestApp(OsmosisTestApp),
    #[cfg(feature = "rpc-runner")]
    RpcRunner(RpcRunner),
    #[cfg(feature = "multi-test")]
    MultiTest(MultiTestRunner<'a>),
}

/// A version of TestRunner which borrows the runner instead of owning it. This is useful for
/// passing a TestRunner to a function which needs to own it, but we don't want to give up ownership
/// of the runner.
pub enum TestRunner<'a> {
    // Needed to keep lifetime when rpc-runner and multitest features are off
    PhantomData(&'a ()),
    #[cfg(feature = "osmosis-test-tube")]
    OsmosisTestApp(&'a OsmosisTestApp),
    #[cfg(feature = "rpc-runner")]
    RpcRunner(&'a RpcRunner),
    #[cfg(feature = "multi-test")]
    MultiTest(&'a MultiTestRunner<'a>),
}

impl<'a> OwnedTestRunner<'a> {
    pub fn as_ref(&'a self) -> TestRunner<'a> {
        match self {
            Self::PhantomData(_) => unreachable!(),
            #[cfg(feature = "osmosis-test-tube")]
            Self::OsmosisTestApp(app) => TestRunner::OsmosisTestApp(app),
            #[cfg(feature = "rpc-runner")]
            Self::RpcRunner(runner) => TestRunner::RpcRunner(runner),
            #[cfg(feature = "multi-test")]
            Self::MultiTest(runner) => TestRunner::MultiTest(runner),
        }
    }
}

impl FromStr for OwnedTestRunner<'_> {
    type Err = String;

    /// Returns a TestRunner from a string, which is the name of the runner. Useful for deciding
    /// which runner to use base on an env var or similar.
    ///
    /// NB: `MultiTestRunner` will use the "osmo" address prefix.
    /// `RpcRunner` is not supported in this function, as it requires a config file and optional
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

impl Display for OwnedTestRunner<'_> {
    /// Returns the name of the runner.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PhantomData(_) => unreachable!(),
            #[cfg(feature = "osmosis-test-tube")]
            Self::OsmosisTestApp(_) => write!(f, "osmosis-test-app"),
            #[cfg(feature = "rpc-runner")]
            Self::RpcRunner(_) => write!(f, "rpc-runner"),
            #[cfg(feature = "multi-test")]
            Self::MultiTest(_) => write!(f, "multi-test"),
        }
    }
}
impl Display for TestRunner<'_> {
    /// Returns the name of the runner.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PhantomData(_) => unreachable!(),
            #[cfg(feature = "osmosis-test-tube")]
            Self::OsmosisTestApp(_) => write!(f, "osmosis-test-app"),
            #[cfg(feature = "rpc-runner")]
            Self::RpcRunner(_) => write!(f, "rpc-runner"),
            #[cfg(feature = "multi-test")]
            Self::MultiTest(_) => write!(f, "multi-test"),
        }
    }
}

impl OwnedTestRunner<'_> {
    /// Creates a TestRunner instance from the contents of the env var `TEST_RUNNER`. If the env var
    /// is not set, it defaults to `multi-test`. Any string value which `from_str` can parse is valid.
    pub fn from_env_var() -> Result<Self, String> {
        OwnedTestRunner::from_str(
            &std::env::var("TEST_RUNNER").unwrap_or_else(|_| "multi-test".into()),
        )
    }
}

#[cfg(feature = "osmosis-test-tube")]
impl From<OsmosisTestApp> for OwnedTestRunner<'_> {
    fn from(app: OsmosisTestApp) -> Self {
        Self::OsmosisTestApp(app)
    }
}
#[cfg(feature = "osmosis-test-tube")]
impl<'a> From<&'a OsmosisTestApp> for TestRunner<'a> {
    fn from(app: &'a OsmosisTestApp) -> Self {
        Self::OsmosisTestApp(app)
    }
}

#[cfg(feature = "rpc-runner")]
impl<'a> From<RpcRunner> for OwnedTestRunner<'a> {
    fn from(runner: RpcRunner) -> Self {
        Self::RpcRunner(runner)
    }
}
#[cfg(feature = "rpc-runner")]
impl<'a> From<&'a RpcRunner> for TestRunner<'a> {
    fn from(runner: &'a RpcRunner) -> Self {
        Self::RpcRunner(runner)
    }
}

#[cfg(feature = "multi-test")]
impl<'a> From<MultiTestRunner<'a>> for OwnedTestRunner<'a> {
    fn from(runner: MultiTestRunner<'a>) -> Self {
        Self::MultiTest(runner)
    }
}
#[cfg(feature = "multi-test")]
impl<'a> From<&'a MultiTestRunner<'a>> for TestRunner<'a> {
    fn from(runner: &'a MultiTestRunner<'a>) -> Self {
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
            Self::PhantomData(_) => unimplemented!(),
            #[cfg(feature = "osmosis-test-tube")]
            Self::OsmosisTestApp(app) => app.execute_multiple(msgs, signer),
            #[cfg(feature = "rpc-runner")]
            Self::RpcRunner(runner) => runner.execute_multiple(msgs, signer),
            #[cfg(feature = "multi-test")]
            Self::MultiTest(runner) => runner.execute_multiple(msgs, signer),
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
            Self::PhantomData(_) => unimplemented!(),
            #[cfg(feature = "osmosis-test-tube")]
            Self::OsmosisTestApp(app) => app.execute_multiple_raw(msgs, signer),
            #[cfg(feature = "rpc-runner")]
            Self::RpcRunner(runner) => runner.execute_multiple_raw(msgs, signer),
            #[cfg(feature = "multi-test")]
            Self::MultiTest(runner) => runner.execute_multiple_raw(msgs, signer),
        }
    }

    fn query<Q, R>(&self, path: &str, query: &Q) -> test_tube::RunnerResult<R>
    where
        Q: prost::Message,
        R: prost::Message + DeserializeOwned + Default,
    {
        match self {
            Self::PhantomData(_) => unimplemented!(),
            #[cfg(feature = "osmosis-test-tube")]
            Self::OsmosisTestApp(app) => app.query(path, query),
            #[cfg(feature = "rpc-runner")]
            Self::RpcRunner(runner) => runner.query(path, query),
            #[cfg(feature = "multi-test")]
            Self::MultiTest(runner) => runner.query(path, query),
        }
    }

    fn execute_tx(
        &self,
        tx_bytes: &[u8],
    ) -> test_tube::RunnerResult<cosmrs::proto::tendermint::v0_37::abci::ResponseDeliverTx> {
        match self {
            Self::PhantomData(_) => unimplemented!(),
            #[cfg(feature = "osmosis-test-tube")]
            Self::OsmosisTestApp(app) => app.execute_tx(tx_bytes),
            #[cfg(feature = "rpc-runner")]
            Self::RpcRunner(runner) => runner.execute_tx(tx_bytes),
            #[cfg(feature = "multi-test")]
            Self::MultiTest(runner) => runner.execute_tx(tx_bytes),
        }
    }
}
impl Runner<'_> for OwnedTestRunner<'_> {
    fn execute_multiple<M, R>(
        &self,
        msgs: &[(M, &str)],
        signer: &SigningAccount,
    ) -> test_tube::RunnerExecuteResult<R>
    where
        M: prost::Message,
        R: prost::Message + Default,
    {
        self.as_ref().execute_multiple(msgs, signer)
    }

    fn execute_multiple_raw<R>(
        &self,
        msgs: Vec<cosmrs::Any>,
        signer: &SigningAccount,
    ) -> test_tube::RunnerExecuteResult<R>
    where
        R: prost::Message + Default,
    {
        self.as_ref().execute_multiple_raw(msgs, signer)
    }

    fn query<Q, R>(&self, path: &str, query: &Q) -> test_tube::RunnerResult<R>
    where
        Q: prost::Message,
        R: prost::Message + DeserializeOwned + Default,
    {
        self.as_ref().query(path, query)
    }

    fn execute_tx(
        &self,
        tx_bytes: &[u8],
    ) -> test_tube::RunnerResult<cosmrs::proto::tendermint::v0_37::abci::ResponseDeliverTx> {
        self.as_ref().execute_tx(tx_bytes)
    }
}

impl<'a> CwItRunner<'a> for TestRunner<'a> {
    fn store_code(
        &self,
        code: ContractType,
        signer: &SigningAccount,
    ) -> Result<u64, anyhow::Error> {
        match self {
            Self::PhantomData(_) => unimplemented!(),
            #[cfg(feature = "osmosis-test-tube")]
            Self::OsmosisTestApp(app) => app.store_code(code, signer),
            #[cfg(feature = "rpc-runner")]
            Self::RpcRunner(runner) => runner.store_code(code, signer),
            #[cfg(feature = "multi-test")]
            Self::MultiTest(runner) => runner.store_code(code, signer),
        }
    }

    fn init_account(
        &self,
        initial_balance: &[cosmwasm_std::Coin],
    ) -> Result<SigningAccount, anyhow::Error> {
        match self {
            Self::PhantomData(_) => unimplemented!(),
            #[cfg(feature = "osmosis-test-tube")]
            Self::OsmosisTestApp(app) => Ok(app.init_account(initial_balance)?),
            #[cfg(feature = "rpc-runner")]
            Self::RpcRunner(runner) => runner.init_account(initial_balance),
            #[cfg(feature = "multi-test")]
            Self::MultiTest(runner) => runner.init_account(initial_balance),
        }
    }

    fn init_accounts(
        &self,
        initial_balance: &[cosmwasm_std::Coin],
        num_accounts: usize,
    ) -> Result<Vec<SigningAccount>, anyhow::Error> {
        match self {
            Self::PhantomData(_) => unimplemented!(),
            #[cfg(feature = "osmosis-test-tube")]
            Self::OsmosisTestApp(app) => {
                Ok(app.init_accounts(initial_balance, num_accounts as u64)?)
            }
            #[cfg(feature = "rpc-runner")]
            Self::RpcRunner(runner) => runner.init_accounts(initial_balance, num_accounts),
            #[cfg(feature = "multi-test")]
            Self::MultiTest(runner) => runner.init_accounts(initial_balance, num_accounts),
        }
    }

    fn increase_time(&self, seconds: u64) -> Result<(), anyhow::Error> {
        match self {
            Self::PhantomData(_) => unimplemented!(),
            #[cfg(feature = "osmosis-test-tube")]
            Self::OsmosisTestApp(app) => {
                app.increase_time(seconds);
                Ok(())
            }
            #[cfg(feature = "rpc-runner")]
            Self::RpcRunner(runner) => runner.increase_time(seconds),
            #[cfg(feature = "multi-test")]
            Self::MultiTest(runner) => runner.increase_time(seconds),
        }
    }

    fn query_block_time_nanos(&self) -> u64 {
        match self {
            Self::PhantomData(_) => unimplemented!(),
            #[cfg(feature = "osmosis-test-tube")]
            Self::OsmosisTestApp(app) => app.query_block_time_nanos(),
            #[cfg(feature = "rpc-runner")]
            Self::RpcRunner(runner) => unimplemented!(),
            #[cfg(feature = "multi-test")]
            Self::MultiTest(runner) => runner.query_block_time_nanos(),
        }
    }
}
impl CwItRunner<'_> for OwnedTestRunner<'_> {
    fn store_code(
        &self,
        code: ContractType,
        signer: &SigningAccount,
    ) -> Result<u64, anyhow::Error> {
        self.as_ref().store_code(code, signer)
    }

    fn init_account(
        &self,
        initial_balance: &[cosmwasm_std::Coin],
    ) -> Result<SigningAccount, anyhow::Error> {
        self.as_ref().init_account(initial_balance)
    }

    fn init_accounts(
        &self,
        initial_balance: &[cosmwasm_std::Coin],
        num_accounts: usize,
    ) -> Result<Vec<SigningAccount>, anyhow::Error> {
        self.as_ref().init_accounts(initial_balance, num_accounts)
    }

    fn increase_time(&self, seconds: u64) -> Result<(), anyhow::Error> {
        self.as_ref().increase_time(seconds)
    }

    fn query_block_time_nanos(&self) -> u64 {
        self.as_ref().query_block_time_nanos()
    }
}

#[cfg(test)]
mod tests {
    use strum::VariantNames;

    use super::*;

    #[test]
    fn test_runner_from_and_to_str() {
        for str in OwnedTestRunner::VARIANTS {
            match *str {
                "phantom-data" => continue,
                "rpc-runner" => match OwnedTestRunner::from_str(str) {
                    Ok(_) => panic!("RpcRunner from_str should fail"),
                    Err(err) => assert_eq!(err, "RpcRunner requires a config file".to_string()),
                },
                _ => {
                    let runner = OwnedTestRunner::from_str(str).unwrap();
                    assert_eq!(&runner.to_string(), str);
                }
            }
        }
    }
}
