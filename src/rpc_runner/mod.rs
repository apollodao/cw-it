//! # RPC Runner
//! This module contains a runner that can be used to interact with a blockchain via an RPC node.
//! It implements the `CwItRunner` trait to be compatible with all the other runners in this crate.
//! There are some fundamental differences between this runner and the others.
//!
//! 1. Since we are only interacting with the blockchain via RPC and are not controlling the entire chain.
//! This means that to initialize accounts with the `init_account` method a funding account is required.
//! This funding account is used to send tokens to the account that is being initialized, which means that the
//! funding account must have enough tokens to initialize all the accounts that are being initialized.
//!
//! 2. The `increase_time` function will panic as it is not implementable via RPC.
pub mod chain;
pub mod config;
pub mod error;
mod helpers;
mod runner;

pub use runner::*;
