use std::collections::HashMap;

use cosmwasm_std::{StdError, StdResult};
use osmosis_testing::{Module, Runner, SigningAccount, Wasm};

use crate::config::TestConfig;

pub fn upload_wasm_files<'a, R: Runner<'a>>(
    runner: &'a R,
    signer: &SigningAccount,
    config: TestConfig,
) -> StdResult<HashMap<String, u64>> {
    let wasm = Wasm::new(runner);
    config
        .contracts
        .into_iter()
        .map(|(name, contract)| {
            let wasm_file_path = format!("{}/{}", config.artifacts_folder, contract.artifact);
            println!("Uploading wasm file: {}", wasm_file_path);
            let wasm_byte_code = std::fs::read(wasm_file_path).unwrap();
            let code_id = wasm
                .store_code(&wasm_byte_code, None, signer)
                .map_err(|e| StdError::generic_err(format!("{:?}", e)))?
                .data
                .code_id;
            Ok((name, code_id))
        })
        .collect()
}
