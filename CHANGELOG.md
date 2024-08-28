# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2024-08-28

### Changed

- Update Astroport package to 5.2.0 which includes native lp token implementation.
- Replace Astroport artifacts.
- Update Astroport helpers to use native lp tokens instead of CW20.

## [0.3.1] - 2024-03-07

### Changed

- Bump `astroport-router` to `1.2.0`.

## [0.3.0] - 2024-02-13

### Changed

- Bump dependencies
  - `osmosis-std` to 0.22.0
  - `test-tube` to 0.5.0
  - `osmosis-test-tube` to 22.1.0
  - `prost` to 0.12
  - `cosmrs` to 0.15
  - `cosmwasm-std` to 1.5.0

### Added

- Helper fn `instantiate_astroport` now instantiates the `astroport-incentives` contract.
  - NB: Astroport incentives contract API is only in `astroport` package version >= 3.11 which contains unreleased changes. Be careful when integrating with Astroport as they don't follow SemVer.

## [0.2.3] - 2023-11-01

### Changed

- Bumped `astroport` to `2.9.0` and removed `astroport_v3` dependency since `2.9.0` now includes the `astroport-liquidity-manager` contract.

## [0.2.2] - 2023-10-27

### Changed

- Bumped `astroport` to `2.8.7`.
- Helper fn `instantiate_astroport` now instantiates the `astroport-liquidity-manager` contract and registers the `concentrated` pair type in the factory contract.
  - NB: Astroport liquidity manager API only exists in the `astroport` version >= 3.6.1 which contains unreleased changes. Be careful when integrating with Astroport as they don't follow SemVer.

## [0.2.1] - 2023-09-26

### Changed

- Bumped `osmosis-std` to `0.19.2`.
- Bumped `osmosis-test-tube` to `0.19.0`.
- Bumped `test-tube` to `0.1.7`.
- Renamed `TestRunner` to `OwnedTestRunner` and introduced new enum `TestRunner` that has borrowed fields.
- Introduced [`Unwrap`](src/helpers.rs) enum that helps with unwrapping `Result`s in Robot tests.
