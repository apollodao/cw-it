use cosmrs::{
    proto::cosmwasm::wasm::v1::{
        QueryCodeRequest, QueryCodeResponse, QueryContractInfoRequest, QueryContractInfoResponse,
    },
    rpc::{endpoint::abci_query::AbciQuery, Client, HttpClient},
};
use prost::Message;

use crate::helpers::block_on;

use super::ArtifactError;

pub fn download_wasm_from_code_id(
    rpc_endpoint: &str,
    code_id: u64,
) -> Result<Vec<u8>, ArtifactError> {
    let http_client = HttpClient::new(rpc_endpoint)?;
    // Query wasm file
    let code_res = QueryCodeResponse::decode(
        rpc_query(
            &http_client,
            QueryCodeRequest { code_id },
            "/cosmwasm.wasm.v1.Query/Code",
        )?
        .value
        .as_slice(),
    )?;
    Ok(code_res.data)
}

pub fn download_wasm_from_contract_address(
    rpc_endpoint: &str,
    contract_address: impl Into<String>,
) -> Result<Vec<u8>, ArtifactError> {
    let http_client = HttpClient::new(rpc_endpoint)?;

    // Query contract info
    let code_id = QueryContractInfoResponse::decode(
        rpc_query(
            &http_client,
            QueryContractInfoRequest {
                address: contract_address.into(),
            },
            "/cosmwasm.wasm.v1.Query/ContractInfo",
        )?
        .value
        .as_slice(),
    )?
    .contract_info
    .ok_or(ArtifactError::Generic(
        "failed to query contract info".to_string(),
    ))?
    .code_id;

    download_wasm_from_code_id(rpc_endpoint, code_id)
}

fn rpc_query<T: Message>(
    client: &HttpClient,
    req: T,
    path: &str,
) -> Result<AbciQuery, ArtifactError> {
    let mut buf = Vec::with_capacity(req.encoded_len());
    req.encode(&mut buf).unwrap();
    Ok(block_on(client.abci_query(
        Some(path.parse().unwrap()),
        buf,
        None,
        false,
    ))?)
}

// Commented out because the RPC node goes down sometimes which breaks CI
// #[test]
// fn test_rpc_query() {
//     let rpc_endpoint = "https://rpc.osmosis.zone/".to_string();
//     let http_client = HttpClient::new(rpc_endpoint.as_str()).unwrap();
//     let req = QueryCodeRequest { code_id: 1 };
//     let res = rpc_query(&http_client, req, "/cosmwasm.wasm.v1.Query/ContractInfo").unwrap();
//     println!("{:?}", res);
// }
