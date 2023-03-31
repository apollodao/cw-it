#[cfg(feature = "rpc-runner")]
use crate::rpc_runner::RpcRunner;

use osmosis_test_tube::{OsmosisTestApp, Runner};

/// An enum with concrete types implementing the Runner trait. We specify these here because the
/// Runner trait is not object safe, and we want to be able to run tests against different types of
/// runners.
pub enum TestRunner<'a> {
    OsmosisTestApp(OsmosisTestApp),
    // Needed to keep lifetime when rpc-runner feature is off
    PhantomData(&'a ()),
    #[cfg(feature = "rpc-runner")]
    RpcRunner(RpcRunner<'a>),
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
            TestRunner::PhantomData(_) => unimplemented!(),
        }
    }

    fn query<Q, R>(&self, path: &str, query: &Q) -> osmosis_test_tube::RunnerResult<R>
    where
        Q: prost::Message,
        R: prost::Message + Default,
    {
        match self {
            TestRunner::OsmosisTestApp(app) => app.query(path, query),
            #[cfg(feature = "rpc-runner")]
            TestRunner::RpcRunner(runner) => runner.query(path, query),
            TestRunner::PhantomData(_) => unimplemented!(),
        }
    }
}
