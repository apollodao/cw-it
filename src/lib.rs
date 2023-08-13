pub mod application;
pub mod artifact;
pub mod const_coin;
pub mod error;
pub mod helpers;
pub mod robot;
pub mod traits;

/// This module contains the implementation of the `CwItRunner` trait for the `OsmosisTestApp` struct.
#[cfg(feature = "osmosis-test-tube")]
pub mod osmosis_test_app;
#[cfg(feature = "osmosis-test-tube")]
pub use osmosis_test_app::WhitelistForceUnlock;

#[cfg(feature = "multi-test")]
#[cfg(test)]
mod test_helpers;

#[cfg(feature = "multi-test")]
pub mod multi_test;

#[cfg(feature = "rpc-runner")]
#[cfg_attr(docsrs, doc(cfg(feature = "rpc-runner")))]
pub mod rpc_runner;

#[cfg(feature = "osmosis")]
#[cfg_attr(docsrs, doc(cfg(feature = "osmosis")))]
pub mod osmosis;

#[cfg(feature = "astroport")]
#[cfg_attr(docsrs, doc(cfg(feature = "astroport")))]
pub mod astroport;

// We apply these attributes to this module since we get warnings when no features have been selected
#[allow(unused_variables)]
#[allow(dead_code)]
pub mod test_runner;

pub use artifact::*;
pub use test_runner::TestRunner;

// Re-exports for convenience
pub use cosmrs;
pub use osmosis_std;
pub use test_tube;

#[cfg(feature = "osmosis-test-tube")]
pub use osmosis_test_tube;

#[cfg(feature = "multi-test")]
pub use apollo_cw_multi_test as cw_multi_test;

#[cfg(feature = "rpc-runner")]
pub use testcontainers::clients::Cli;
