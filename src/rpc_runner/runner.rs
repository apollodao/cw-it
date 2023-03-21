use std::num::ParseIntError;

use cosmos_sdk_proto::cosmos::auth::v1beta1::{
    BaseAccount, QueryAccountRequest, QueryAccountResponse,
};
use cosmrs::proto::cosmwasm::wasm::v1::{
    QuerySmartContractStateRequest, QuerySmartContractStateResponse,
};
use cosmwasm_std::{
    from_binary, Coin, ContractResult, Empty, Querier, QuerierResult, QueryRequest, SystemResult,
    WasmQuery,
};
use test_tube::{
    account::FeeSetting, Account, DecodeError, EncodeError, Runner, RunnerError,
    RunnerExecuteResult, RunnerResult, SigningAccount,
};
use testcontainers::clients::Cli;
use testcontainers::images::generic::GenericImage;
use testcontainers::Container;
use thiserror::Error;

use super::chain::Chain;
use crate::application::Application;
use crate::config::TestConfig;

use cosmos_sdk_proto::cosmos::tx::v1beta1::service_client::ServiceClient;
use cosmos_sdk_proto::cosmos::tx::v1beta1::SimulateRequest;
use cosmrs::rpc::endpoint::abci_query::AbciQuery;
use cosmrs::rpc::endpoint::broadcast::tx_commit::Response as TxCommitResponse;
use cosmrs::rpc::Client;
use cosmrs::tx::{self, Raw};
use cosmrs::tx::{Fee, SignerInfo};
use cosmrs::AccountId;
use prost::Message;

#[derive(Debug, Error)]
pub enum RpcRunnerError {
    #[error("{0}")]
    ChainError(#[from] super::chain::ChainError),

    #[error("{0}")]
    Generic(String),
}

#[derive(Debug)]
pub struct RpcRunner<'a> {
    chain: Chain,
    _container: Option<Container<'a, GenericImage>>,
    pub test_config: TestConfig,
}

impl<'a> RpcRunner<'a> {
    pub fn new(mut test_config: TestConfig, docker: &'a Cli) -> Result<Self, RpcRunnerError> {
        // Setup test container
        let container = match &test_config.rpc_runner_config.container {
            Some(container_info) => {
                let container: Container<GenericImage> = docker.run(
                    container_info
                        .get_container_image()
                        .map_err(RpcRunnerError::Generic)?,
                );
                test_config
                    .rpc_runner_config
                    .bind_chain_to_container(&container);
                Some(container)
            }
            None => None,
        };

        // Setup chain and app
        let chain = Chain::new(test_config.rpc_runner_config.chain_config.clone())?;

        Ok(Self {
            chain,
            _container: container,
            test_config,
        })
    }

    pub fn init_accounts(&self, _coins: &[Coin], count: u64) -> Vec<SigningAccount> {
        if count > 10 {
            panic!("cannot create more than 10 accounts");
        }

        let mut accounts = vec![];
        for i in 0..count {
            let account = self
                .test_config
                .rpc_runner_config
                .import_account(&format!("test{}", i))
                .unwrap();
            accounts.push(account);
        }

        accounts
    }
}

impl Querier for RpcRunner<'_> {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        let x = match from_binary::<QueryRequest<Empty>>(&bin_request.into()).unwrap() {
            QueryRequest::Wasm(wasm_query) => match wasm_query {
                WasmQuery::Smart { contract_addr, msg } => self
                    .query::<_, QuerySmartContractStateResponse>(
                        "/cosmwasm.wasm.v1.Query/SmartContractState",
                        &QuerySmartContractStateRequest {
                            address: contract_addr,
                            query_data: msg.into(),
                        },
                    )
                    .unwrap()
                    .data
                    .into(),
                _ => todo!("unsupported WasmQuery variant"),
            },
            _ => todo!("unsupported QueryRequest variant"),
        };

        SystemResult::Ok(ContractResult::Ok(x))
    }
}

impl<'a> Application for RpcRunner<'a> {
    fn create_signed_tx<I>(
        &self,
        msgs: I,
        signer: &SigningAccount,
        fee: Fee,
    ) -> RunnerResult<Vec<u8>>
    where
        I: IntoIterator<Item = cosmrs::Any>,
    {
        // println!("create_signed_tx");
        let account: BaseAccount = self.base_account(signer.account_id())?;
        let tx_body = tx::Body::new(msgs, "MEMO", 0u32);

        // println!("accountId -> {:?}", signer.account_id());
        // println!("account -> {:?}", account);

        let signer_info =
            SignerInfo::single_direct(Some(signer.signing_key().public_key()), account.sequence);
        let auth_info = signer_info.auth_info(fee);
        let sign_doc = tx::SignDoc::new(
            &tx_body,
            &auth_info,
            &self
                .chain
                .chain_cfg()
                .chain_id
                .parse()
                .expect("parse const str of chain id should never fail"),
            account.account_number,
        )
        .map_err(|e| match e.downcast::<prost::EncodeError>() {
            Ok(encode_err) => EncodeError::ProtoEncodeError(encode_err),
            Err(e) => panic!("expect `prost::EncodeError` but got {:?}", e),
        })?;

        let tx_raw: Raw = sign_doc.sign(signer.signing_key())?;

        tx_raw
            .to_bytes()
            .map_err(|e| match e.downcast::<prost::EncodeError>() {
                Ok(encode_err) => EncodeError::ProtoEncodeError(encode_err),
                Err(e) => panic!("expect `prost::EncodeError` but got {:?}", e),
            })
            .map_err(RunnerError::EncodeError)
    }

    #[allow(deprecated)]
    fn simulate_tx<I>(
        &self,
        msgs: I,
        signer: &SigningAccount,
    ) -> RunnerResult<cosmrs::proto::cosmos::base::abci::v1beta1::GasInfo>
    where
        I: IntoIterator<Item = cosmrs::Any>,
    {
        // println!("simulate_tx called");
        let zero_fee = Fee::from_amount_and_gas(
            cosmrs::Coin {
                denom: self.chain.chain_cfg().denom().parse()?,
                amount: (0u8).into(),
            },
            0u64,
        );

        let tx_raw = self.create_signed_tx(msgs, signer, zero_fee)?;
        println!("tx_raw size = {:?}", tx_raw.len());

        let simulate_msg = SimulateRequest {
            tx: None,
            tx_bytes: tx_raw,
        };

        // println!("Init GRpc ServiceClient (port 9090)");

        let gas_info: cosmos_sdk_proto::cosmos::base::abci::v1beta1::GasInfo =
            futures::executor::block_on(async {
                let mut service =
                    ServiceClient::connect(self.chain.chain_cfg().grpc_endpoint.clone())
                        .await
                        .map_err(|e| RunnerError::GenericError(e.to_string()))?;
                service
                    .simulate(simulate_msg)
                    .await
                    .map_err(|e| RunnerError::GenericError(e.to_string()))
            })?
            .into_inner()
            .gas_info
            .ok_or(RunnerError::QueryError {
                msg: "No gas_info returned from simulate".into(),
            })?;

        Ok(cosmrs::proto::cosmos::base::abci::v1beta1::GasInfo {
            gas_wanted: gas_info.gas_wanted,
            gas_used: gas_info.gas_used,
        })
        // let gas_limit = (gas_info.gas_used as f64 * DEFAULT_GAS_ADJUSTMENT).ceil();
        // let amount = Coin {
        //     denom: Denom::from_str(FEE_DENOM).unwrap(),
        //     amount: ((gas_limit * 0.1).ceil() as u64).into(),
        // };

        // Ok(Fee::from_amount_and_gas(amount, gas_limit as u64))
        // unsafe {
        //     let res = Simulate(self.id, base64_tx_bytes);
        //     let res = RawResult::from_non_null_ptr(res).into_result()?;

        //     cosmrs::proto::cosmos::base::abci::v1beta1::GasInfo::decode(res.as_slice())
        //         .map_err(DecodeError::ProtoDecodeError)
        //         .map_err(RunnerError::DecodeError)
        // }
    }

    fn estimate_fee<I>(&self, msgs: I, signer: &SigningAccount) -> RunnerResult<Fee>
    where
        I: IntoIterator<Item = cosmrs::Any>,
    {
        match &signer.fee_setting() {
            FeeSetting::Auto {
                gas_price,
                gas_adjustment,
            } => {
                let gas_info = self.simulate_tx(msgs, signer)?;
                let gas_limit = ((gas_info.gas_used as f64) * gas_adjustment).ceil() as u64;

                let amount = cosmrs::Coin {
                    denom: self.chain.chain_cfg().denom().parse()?,
                    amount: (((gas_limit as f64) * (gas_price.amount.u128() as f64)).ceil() as u64)
                        .into(),
                };

                Ok(Fee::from_amount_and_gas(amount, gas_limit))
            }
            FeeSetting::Custom { .. } => {
                panic!(
                    "estimate fee is a private function and should never be called when fee_setting is Custom"
                );
            }
        }
    }

    fn base_account(&self, account_id: AccountId) -> RunnerResult<BaseAccount> {
        // TODO: find out a race here
        let abci_query = self.abci_query(
            QueryAccountRequest {
                address: account_id.as_ref().into(),
            },
            "/cosmos.auth.v1beta1.Query/Account",
        )?;

        let res = QueryAccountResponse::decode(abci_query.value.as_slice())
            .map_err(DecodeError::ProtoDecodeError)?
            .account
            .ok_or(RunnerError::QueryError {
                msg: "account query failed".to_string(),
            })?;

        let base_account =
            BaseAccount::decode(res.value.as_slice()).map_err(DecodeError::ProtoDecodeError)?;

        Ok(base_account)
    }

    fn abci_query<T: Message>(&self, req: T, path: &str) -> RunnerResult<AbciQuery> {
        let mut buf = Vec::with_capacity(req.encoded_len());
        req.encode(&mut buf)
            .map_err(EncodeError::ProtoEncodeError)?;
        Ok(futures::executor::block_on(
            self.chain
                .client()
                .abci_query(Some(path.parse()?), buf, None, false),
        )?)
    }
}

impl<'a> Runner<'_> for RpcRunner<'a> {
    fn execute_multiple<M, R>(
        &self,
        msgs: &[(M, &str)],
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<R>
    where
        M: ::prost::Message,
        R: ::prost::Message + Default,
    {
        println!("execute_multiple called");
        let encoded_msgs = msgs
            .iter()
            .map(|(msg, type_url)| {
                let mut buf = Vec::new();
                M::encode(msg, &mut buf).map_err(EncodeError::ProtoEncodeError)?;

                Ok(cosmrs::Any {
                    type_url: type_url.to_string(),
                    value: buf,
                })
            })
            .collect::<Result<Vec<cosmrs::Any>, RunnerError>>()?;

        self.execute_multiple_raw(encoded_msgs, signer)
    }

    fn execute_multiple_raw<R>(
        &self,
        msgs: Vec<cosmrs::Any>,
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<R>
    where
        R: prost::Message + Default,
    {
        let _fee = match &signer.fee_setting() {
            FeeSetting::Auto { .. } => self.estimate_fee(msgs.clone(), signer)?,
            FeeSetting::Custom { amount, gas_limit } => Fee::from_amount_and_gas(
                cosmrs::Coin {
                    denom: amount.denom.parse()?,
                    amount: amount
                        .amount
                        .to_string()
                        .parse()
                        .map_err(|e: ParseIntError| RunnerError::GenericError(e.to_string()))?,
                },
                *gas_limit,
            ),
        };

        // TODO: Fix this, sadly estimation goes to the moon and provides no real value
        // there must be a trick somewhere
        let fee = Fee::from_amount_and_gas(
            cosmrs::Coin {
                denom: self.chain.chain_cfg().denom().parse()?,
                amount: 4_000_000,
            },
            25_000_000u64,
        );

        let tx_raw = self.create_signed_tx(msgs, signer, fee)?;

        let tx_commit_response: TxCommitResponse =
            futures::executor::block_on(self.chain.client().broadcast_tx_commit(tx_raw.into()))?;

        if tx_commit_response.check_tx.code.is_err() {
            return Err(RunnerError::ExecuteError {
                msg: tx_commit_response.check_tx.log.value().to_string(),
            });
        }
        if tx_commit_response.deliver_tx.code.is_err() {
            return Err(RunnerError::ExecuteError {
                msg: tx_commit_response.deliver_tx.log.value().to_string(),
            });
        }
        tx_commit_response.try_into()
    }

    fn query<Q, R>(&self, path: &str, msg: &Q) -> RunnerResult<R>
    where
        Q: ::prost::Message,
        R: ::prost::Message + Default,
    {
        let mut base64_query_msg_bytes = Vec::with_capacity(msg.encoded_len());
        msg.encode(&mut base64_query_msg_bytes)
            .map_err(EncodeError::ProtoEncodeError)?;

        let res = futures::executor::block_on(self.chain.client().abci_query(
            Some(path.parse()?),
            base64_query_msg_bytes,
            None,
            false,
        ))?;

        if res.code != cosmrs::tendermint::abci::Code::Ok {
            return Err(RunnerError::QueryError {
                msg: "error".to_string(),
            });
        }

        Ok(R::decode(res.value.as_slice()).map_err(DecodeError::ProtoDecodeError)?)
    }
}
