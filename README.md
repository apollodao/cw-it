# CosmWasm Integration Testing

A crate of utils for integration testing CosmWasm smart contracts.

Note: This crate is still in early development and the API is subject to change.

## About

The main idea of this crate is to provide a set of "Test Runners" that you can run the same set of tests against. This is accomplished by providing different structs that all implement the [CwItRunner](src/traits.rs) trait. This trait is based on the `Runner` trait from [test-tube](https://crates.io/crates/test-tube) but adds some additional functionality.

This crate also includes a [TestRobot](src/robot.rs) trait and a set of "Test Robots", which are structs that implement the `TestRobot` trait and help you implement the [robot testing pattern](https://jhandguy.github.io/posts/robot-pattern-ios/). In essence these are structs that have functions that either perform an action or make an assertion, and then return `self` so that you can chain them together.

### Available Features and Test Runners

This crate has the following optional features:

- `osmosis`
  - Exports the [osmosis](src/osmosis/mod.rs) module containing an implementation of the [CwItRunner](src/traits.rs) trait for the [OsmosisTestApp](https://docs.rs/osmosis-test-tube/16.0.0/osmosis_test_tube/struct.OsmosisTestApp.html) struct from [osmosis-test-tube](https://crates.io/crates/osmosis-test-tube).
  - This module also contains the [OsmosisTestRobot](src/osmosis/robot.rs) trait. You can implement this trait on a struct to get access to a set of helper functions for testing against Osmosis pools.
- `astroport`
  - Exports the [astroport](src/astroport/mod.rs) module containing the `AstroportTestRobot` trait. You can implement this trait on a struct to get access to a set of helper functions for testing against Astroport pools.
- `rpc_runner`
  - Exports the [rpc_runner](src/rpc_runner/mod.rs) module containing the [RpcRunner](src/rpc_runner/struct.RpcRunner.html) struct. This struct implements the [CwItRunner](src/traits.rs) trait and allows you to run your tests against an RPC node.
- `multi-test`
  - Exports the [multi_test](src/multi_test/mod.rs) module containing the [MultiTestRunner](src/multi_test/struct.MultiTestRunner.html) struct. This struct implements the [CwItRunner](src/traits.rs) trait and allows you to run your tests against an instance of [apollo-cw-multi-test](https://github.com/pacmanifold/cw-multi-test) (this is a forked version of [cw-multi-test](https://github.com/CosmWasm/cw-multi-test) which contains changes to support CwItRunner). Running tests against `cw-multi-test` rather than `OsmosisTestApp` can be useful if you need to run a debugger or want to check code coverage.
- `astroport-multi-test`
  - Exports some utility functions in the `astroport` module that help you instantiate an instance of Astroport with `cw-multi-test`.
- `chain-download`
  - This feature enables the `ChainCodeId` and `ChainContractAddress` variants on the `Artifact` enum. This lets you download the wasm file of contracts from an RPC node by either supplying the code ID or the contract address. This is useful if you want to run tests locally against a contract that is already deployed on a chain.

## Usage

Add the following to your `Cargo.toml`:

```toml
[dev-dependencies]
cw-it = "0.1.0"
```

Depending on your needs, you should add one or more of the features mentioned above, e.g:

```toml
[dev-dependencies]
cw-it = { version = "0.1.0", features = ["osmosis"] }
```

Then you can write tests that look like this:

```rust

struct TestingRobot<'a>(&'a TestRunner);

impl<'a> OsmosisTestRobot<'a> for TestingRobot<'a> {}

pub const TEST_RUNNER: &str = "osmosis-test-app";

#[test]
fn test_my_contract() {
    let runner  = TestRunner::from_str(TEST_RUNNER).unwrap();

    let robot = TestingRobot(&runner);

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
            initial_balance - swap_amount - GAS_AMOUNT,
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
```

Here you can see that we first create a testing robot struct, then implement the relevant traits on it to get some useful helper functions. Then we create a `TestRunner` struct and pass it to the robot. The robot then uses the runner to perform the actions and assertions.
