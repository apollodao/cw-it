# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] - 2023-08-23

### Changed

- Bumped `osmosis-std` to `0.17.0-rc0`.
- Bumped `osmosis-test-tube` to `0.17.0-rc0`.
- Bumped `test-tube` to `0.1.6`.
- Renamed `TestRunner` to `OwnedTestRunner` and introduced new enum `TestRunner` that has borrowed fields.
- Introduced [`Unwrap`](src/helpers.rs) enum that helps with unwrapping `Result`s in Robot tests.