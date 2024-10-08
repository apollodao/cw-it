use std::num::ParseIntError;

use anyhow::bail;

use cosmrs::crypto::secp256k1;
use cosmrs::proto::cosmos::auth::v1beta1::BaseAccount;
use cosmrs::proto::cosmos::auth::v1beta1::{QueryAccountRequest, QueryAccountResponse};
use cosmrs::tendermint::Time;
use cosmwasm_std::{
    from_json, Coin, ContractResult, Empty, Querier, QuerierResult, QueryRequest, SystemResult,
    WasmQuery,
};
use osmosis_std::types::cosmwasm::wasm::v1::{
    QuerySmartContractStateRequest, QuerySmartContractStateResponse,
};
use test_tube::{
    account::FeeSetting, Account, DecodeError, EncodeError, Module, Runner, RunnerError,
    RunnerExecuteResult, RunnerResult, SigningAccount, Wasm,
};

use super::chain::Chain;
use super::config::RpcRunnerConfig;
use super::error::RpcRunnerError;
use super::helpers;
use crate::helpers::{bank_send, block_on};
use crate::traits::CwItRunner;
use crate::ContractType;

use cosmrs::rpc::endpoint::abci_query::AbciQuery;
use cosmrs::rpc::endpoint::broadcast::tx_commit::Response as TxCommitResponse;
use cosmrs::rpc::Client;
use cosmrs::tx::{self, Raw};
use cosmrs::tx::{Fee, SignerInfo};
use cosmrs::AccountId;
use prost::Message;

pub struct RpcRunner {
    chain: Chain,
    funding_account: SigningAccount,
    pub config: RpcRunnerConfig,
}

impl RpcRunner {
    pub fn new(rpc_runner_config: RpcRunnerConfig) -> Result<Self, RpcRunnerError> {
        // Setup chain and app
        let chain = Chain::new(rpc_runner_config.chain_config.clone())?;

        let signing_key = helpers::mnemonic_to_signing_key(
            &rpc_runner_config.funding_account_mnemonic,
            &rpc_runner_config.chain_config.derivation_path.parse()?,
        )?;

        let funding_account = SigningAccount::new(
            rpc_runner_config.chain_config.prefix.clone(),
            signing_key,
            rpc_runner_config
                .fee_setting
                .clone()
                .unwrap_or(chain.chain_cfg().auto_fee_setting())
                .into(),
        );

        Ok(Self {
            chain,
            config: rpc_runner_config,
            funding_account,
        })
    }
}

impl Querier for RpcRunner {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        let x = match from_json::<QueryRequest<Empty>>(&bin_request).unwrap() {
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

impl RpcRunner {
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
        _msgs: I,
        _signer: &SigningAccount,
    ) -> RunnerResult<cosmrs::proto::cosmos::base::abci::v1beta1::GasInfo>
    where
        I: IntoIterator<Item = cosmrs::Any>,
    {
        todo!()
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
        Ok(block_on(self.chain.client().abci_query(
            Some(path.to_string()),
            buf,
            None,
            false,
        ))?)
    }
}

impl Runner<'_> for RpcRunner {
    fn execute_multiple<M, R>(
        &self,
        msgs: &[(M, &str)],
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<R>
    where
        M: ::prost::Message,
        R: ::prost::Message + Default,
    {
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
            block_on(self.chain.client().broadcast_tx_commit(tx_raw))?;

        if tx_commit_response.check_tx.code.is_err() {
            return Err(RunnerError::ExecuteError {
                msg: tx_commit_response.check_tx.log,
            });
        }
        if tx_commit_response.tx_result.code.is_err() {
            return Err(RunnerError::ExecuteError {
                msg: tx_commit_response.tx_result.log,
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

        let res = block_on(self.chain.client().abci_query(
            Some(path.to_string()),
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

    fn execute_tx(
        &self,
        _tx_bytes: &[u8],
    ) -> RunnerResult<cosmrs::proto::tendermint::v0_37::abci::ResponseDeliverTx> {
        todo!()
    }
}

impl<'a> CwItRunner<'a> for RpcRunner {
    fn store_code(
        &self,
        code: ContractType,
        signer: &SigningAccount,
    ) -> Result<u64, anyhow::Error> {
        match code {
            ContractType::Artifact(artifact) => {
                let bytes = artifact.get_wasm_byte_code()?;
                let wasm = Wasm::new(self);
                let code_id = wasm.store_code(&bytes, None, signer)?.data.code_id;
                Ok(code_id)
            }
            _ => bail!("Only ContractType::Artifact is supported for RpcRunner"),
        }
    }

    fn init_account(&self, initial_balance: &[Coin]) -> Result<SigningAccount, anyhow::Error> {
        // Create new random account
        let new_account = SigningAccount::new(
            self.chain.chain_cfg().prefix().to_string(),
            secp256k1::SigningKey::random(),
            self.config
                .fee_setting
                .clone()
                .unwrap_or(self.chain.chain_cfg().auto_fee_setting())
                .into(),
        );

        // Fund account with initial_balance from funding_account
        bank_send(
            self,
            &self.funding_account,
            &new_account.address(),
            initial_balance.to_vec(),
        )
        .map_err(|e| anyhow::anyhow!("Funding of new account failed. Error: {}", e))?;

        Ok(new_account)
    }

    fn init_accounts(
        &self,
        initial_balance: &[Coin],
        num_accounts: usize,
    ) -> Result<Vec<SigningAccount>, anyhow::Error> {
        let mut accounts = Vec::new();
        for _ in 0..num_accounts {
            accounts.push(self.init_account(initial_balance)?);
        }
        Ok(accounts)
    }

    fn increase_time(&self, _seconds: u64) -> Result<(), anyhow::Error> {
        // TODO: Figure out best way to sleep tests until `seconds` has passed.
        todo!("Increase time is unimplemented for RpcRunner")
    }

    fn query_block_time_nanos(&self) -> u64 {
        block_on(self.chain.client().latest_block())
            .unwrap()
            .block
            .header
            .time
            .duration_since(Time::unix_epoch())
            .unwrap()
            .as_nanos() as u64
    }
}

// Commenting out RPC tests so that CI doesn't break randomly when the RPC endpoint is down
// #[cfg(test)]
// mod test {
//     use crate::rpc_runner::{chain::ChainConfig, config::RpcRunnerConfig, RpcRunner};
//     use crate::traits::CwItRunner;

//     #[test]
//     fn test_query_block_time_nanos() {
//         let rpc_runner_config = RpcRunnerConfig {
//             accounts_folder: "".to_string(),
//             chain_config: ChainConfig {
//                 chain_id: "pion-1".to_string(),
//                 derivation_path: "m/44'/1'/0'/0/0".to_string(),
//                 gas_adjustment: 1.5,
//                 gas_price: 0,
//                 grpc_endpoint: "http://grpc-palvus.pion-1.ntrn.tech:80".to_string(),
//                 rpc_endpoint: "https://rpc-palvus.pion-1.ntrn.tech:443".to_string(),
//                 name: "pion-1".to_string(),
//                 denom: "ntrn".to_string(),
//                 prefix: "neutron".to_string(),
//             },
//             container: None,
//         };
//         let rpc_runner = RpcRunner::new(rpc_runner_config, None).unwrap();
//         let block_time_nanos = rpc_runner.query_block_time_nanos();
//         println!("block_time_nanos: {}", block_time_nanos);
//         assert!(block_time_nanos > 1683910796000000000);
//     }
// }
