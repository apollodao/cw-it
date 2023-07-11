use anyhow::Error;
use cosmwasm_std::Coin;
use osmosis_std::{shim::Any, types::osmosis::lockup};
use osmosis_test_tube::{Module, OsmosisTestApp, SigningAccount, Wasm};
use prost::Message;

use crate::{traits::CwItRunner, ContractType};

#[cfg(feature = "multi-test")]
use anyhow::bail;

impl CwItRunner<'_> for OsmosisTestApp {
    fn store_code(&self, code: ContractType, signer: &SigningAccount) -> Result<u64, Error> {
        match code {
            #[cfg(feature = "multi-test")]
            ContractType::MultiTestContract(_) => {
                bail!("MultiTestContract not supported for OsmosisTestApp")
            }
            ContractType::Artifact(artifact) => {
                let bytes = artifact.get_wasm_byte_code()?;
                let wasm = Wasm::new(self);
                let code_id = wasm.store_code(&bytes, None, signer)?.data.code_id;
                Ok(code_id)
            }
        }
    }

    fn init_account(&self, initial_balance: &[Coin]) -> Result<SigningAccount, Error> {
        Ok(self.init_account(initial_balance)?)
    }

    fn init_accounts(
        &self,
        initial_balance: &[Coin],
        num_accounts: usize,
    ) -> Result<Vec<SigningAccount>, Error> {
        Ok(self.init_accounts(initial_balance, num_accounts as u64)?)
    }

    fn increase_time(&self, seconds: u64) -> Result<(), Error> {
        OsmosisTestApp::increase_time(self, seconds);
        Ok(())
    }

    fn query_block_time_nanos(&self) -> u64 {
        self.get_block_time_nanos() as u64
    }
}

/// A trait for enabling the functionality of whitelisting an address for force unlock of a locked
/// LP position on Osmosis.
pub trait WhitelistForceUnlock {
    /// Whitelists the given address for force unlock of locked LP positions.
    fn whitelist_address_for_force_unlock(&self, addr: &str) -> Result<(), Error>;
}

impl WhitelistForceUnlock for OsmosisTestApp {
    fn whitelist_address_for_force_unlock(&self, addr: &str) -> Result<(), Error> {
        Ok(self.set_param_set(
            "lockup",
            Any {
                type_url: lockup::Params::TYPE_URL.to_string(),
                value: lockup::Params {
                    force_unlock_allowed_addresses: vec![addr.to_string()],
                }
                .encode_to_vec(),
            },
        )?)
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::Coin;
    use osmosis_std::types::{
        cosmos::bank::v1beta1::QueryAllBalancesResponse,
        osmosis::{
            gamm::v1beta1::QueryTotalSharesRequest,
            lockup::{
                MsgForceUnlock, MsgForceUnlockResponse, MsgLockTokens, MsgLockTokensResponse,
            },
        },
    };
    use osmosis_test_tube::{Gamm, OsmosisTestApp};
    use test_tube::{Account, Runner, RunnerError};

    use crate::artifact::Artifact;

    use super::*;

    const TEST_ARTIFACT: &str = "artifacts/counter.wasm";

    #[test]
    fn osmosis_test_app_store_code() {
        let app = OsmosisTestApp::new();
        let admin = app
            .init_account(&[Coin::new(1000000000000, "uosmo")])
            .unwrap();
        let code_id = app
            .store_code(
                ContractType::Artifact(Artifact::Local(TEST_ARTIFACT.to_string())),
                &admin,
            )
            .unwrap();

        assert_eq!(code_id, 1);
    }

    #[test]
    #[should_panic]
    #[cfg(feature = "multi-test")]
    fn osmosis_test_app_store_code_multi_test_contract() {
        use crate::test_helpers::test_contract;

        let app = OsmosisTestApp::new();
        let admin = app
            .init_account(&[Coin::new(1000000000000, "uosmo")])
            .unwrap();
        app.store_code(
            ContractType::MultiTestContract(test_contract::contract()),
            &admin,
        )
        .unwrap();
    }

    #[test]
    fn test_increase_time() {
        let app = OsmosisTestApp::new();

        let time = app.get_block_time_nanos();
        CwItRunner::increase_time(&app, 69).unwrap();
        assert_eq!(app.get_block_time_nanos(), time + 69000000000);
    }

    #[test]
    fn whitelist_address_for_force_unlock_works() {
        let app = OsmosisTestApp::new();

        let balances = vec![
            Coin::new(1_000_000_000_000, "uosmo"),
            Coin::new(1_000_000_000_000, "uion"),
        ];
        let whitelisted_user = app.init_account(&balances).unwrap();

        // create pool
        let gamm = Gamm::new(&app);
        let pool_id = gamm
            .create_basic_pool(
                &[Coin::new(1_000_000, "uosmo"), Coin::new(1_000_000, "uion")],
                &whitelisted_user,
            )
            .unwrap()
            .data
            .pool_id;

        // query shares
        let shares = app
            .query::<QueryTotalSharesRequest, QueryAllBalancesResponse>(
                "/osmosis.gamm.v1beta1.Query/TotalShares",
                &QueryTotalSharesRequest { pool_id },
            )
            .unwrap()
            .balances;

        // lock all shares
        app.execute::<_, MsgLockTokensResponse>(
            MsgLockTokens {
                owner: whitelisted_user.address(),
                duration: Some(osmosis_std::shim::Duration {
                    seconds: 1000000000,
                    nanos: 0,
                }),
                coins: shares,
            },
            MsgLockTokens::TYPE_URL,
            &whitelisted_user,
        )
        .unwrap();

        // try to unlock
        let err = app
            .execute::<_, MsgForceUnlockResponse>(
                MsgForceUnlock {
                    owner: whitelisted_user.address(),
                    id: pool_id,
                    coins: vec![], // all
                },
                MsgForceUnlock::TYPE_URL,
                &whitelisted_user,
            )
            .unwrap_err();

        // should fail
        assert_eq!(err,  RunnerError::ExecuteError {
            msg: format!("failed to execute message; message index: 0: Sender ({}) not allowed to force unlock: unauthorized", whitelisted_user.address()),
        });

        // add whitelisted user to param set
        app.whitelist_address_for_force_unlock(&whitelisted_user.address())
            .unwrap();

        // unlock again after adding whitelisted user
        let res = app
            .execute::<_, MsgForceUnlockResponse>(
                MsgForceUnlock {
                    owner: whitelisted_user.address(),
                    id: pool_id,
                    coins: vec![], // all
                },
                MsgForceUnlock::TYPE_URL,
                &whitelisted_user,
            )
            .unwrap();

        // should succeed
        assert!(res.data.success);
    }
}
