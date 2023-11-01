# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.2] - 2023-10-27

### Changed

- Bumped `astroport` to `2.9.0`.
- Helper fn `instantiate_astroport` now instantiates the `astroport-liquidity-manager` contract and registers the `concentrated` pair type in the factory contract.

## [0.2.1] - 2023-09-26

### Changed

- Bumped `osmosis-std` to `0.19.2`.
- Bumped `osmosis-test-tube` to `0.19.0`.
- Bumped `test-tube` to `0.1.7`.
- Renamed `TestRunner` to `OwnedTestRunner` and introduced new enum `TestRunner` that has borrowed fields.
- Introduced [`Unwrap`](src/helpers.rs) enum that helps with unwrapping `Result`s in Robot tests.
