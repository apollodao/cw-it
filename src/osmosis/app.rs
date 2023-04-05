use anyhow::{bail, Error};
use cosmwasm_std::Coin;
use osmosis_test_tube::{Module, OsmosisTestApp, SigningAccount, Wasm};

use crate::{traits::CwItRunner, ContractType};

impl CwItRunner<'_> for OsmosisTestApp {
    fn store_code(&self, code: ContractType, signer: &SigningAccount) -> Result<u64, Error> {
        match code {
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
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::Coin;
    use osmosis_test_tube::OsmosisTestApp;

    use crate::artifact::Artifact;

    use crate::test_helpers::*;

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
}
