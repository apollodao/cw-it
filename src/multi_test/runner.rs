use crate::multi_test::api::MockApiBech32;
use crate::multi_test::test_addresses::MockAddressGenerator;
use crate::{traits::CwItRunner, ContractType};
use anyhow::bail;
use apollo_cw_multi_test::BankKeeper;
use apollo_cw_multi_test::WasmKeeper;
use apollo_cw_multi_test::{BankSudo, BasicAppBuilder};
use cosmrs::{crypto::secp256k1::SigningKey, proto::cosmos::base::abci::v1beta1::GasInfo};
use cosmwasm_std::{
    coin, Addr, BankMsg, Binary, Coin, CosmosMsg, Empty, QueryRequest, StakingMsg, WasmMsg,
};
use osmosis_std::types::{
    cosmos::{
        bank::v1beta1::MsgSend,
        staking::v1beta1::{MsgBeginRedelegate, MsgDelegate, MsgUndelegate},
    },
    cosmwasm::wasm::v1::{
        MsgClearAdmin, MsgExecuteContract, MsgInstantiateContract, MsgMigrateContract,
        MsgUpdateAdmin,
    },
};
use prost::Message;
use serde::de::DeserializeOwned;
use std::str::FromStr;
use test_tube::{
    Account, DecodeError, EncodeError, FeeSetting, Runner, RunnerError, SigningAccount,
};

pub struct MultiTestRunner<'a> {
    pub app: apollo_cw_multi_test::App<BankKeeper, MockApiBech32<'a>>,
    pub address_prefix: &'a str,
}

impl<'a> MultiTestRunner<'a> {
    /// Creates a new instance of a `MultiTestRunner`, wrapping a `cw_multi_test::App`
    /// with the given address prefix.
    pub fn new(address_prefix: &'a str) -> Self {
        // Construct app
        let wasm_keeper: WasmKeeper<Empty, Empty> =
            WasmKeeper::new().with_address_generator(MockAddressGenerator);

        let app = BasicAppBuilder::<Empty, Empty>::new()
            .with_api(MockApiBech32::new(address_prefix))
            .with_wasm(wasm_keeper)
            .build(|_, _, _| {});

        Self {
            app,
            address_prefix,
        }
    }

    /// Creates a new instance of a `MultiTestRunner`, wrapping a `cw_multi_test::App`
    /// with the given address prefix and stargate keeper. This is needed for testing
    /// functionality that requires the Stargate messages or queries.
    pub fn new_with_stargate(
        address_prefix: &'a str,
        stargate_keeper: apollo_cw_multi_test::StargateKeeper<Empty, Empty>,
    ) -> Self {
        // Construct app
        let app = BasicAppBuilder::<Empty, Empty>::new()
            .with_api(MockApiBech32::new(address_prefix))
            .with_stargate(stargate_keeper)
            .build(|_, _, _| {});

        Self {
            app,
            address_prefix,
        }
    }
}

impl Runner<'_> for MultiTestRunner<'_> {
    fn execute_cosmos_msgs<S>(
        &self,
        msgs: &[cosmwasm_std::CosmosMsg],
        signer: &test_tube::SigningAccount,
    ) -> test_tube::RunnerExecuteResult<S>
    where
        S: prost::Message + Default,
    {
        let sender = Addr::unchecked(signer.address());

        // Execute messages with multi test app
        let app_responses = self
            .app
            .execute_multi(sender, msgs.to_vec())
            // NB: Must use this syntax to capture full anyhow message.
            // to_string() will only give the outermost error context.
            .map_err(|e| RunnerError::GenericError(format!("{:#}", e)))?;

        // Construct test_tube::ExecuteResponse from cw_multi_test::AppResponse
        let events = app_responses
            .iter()
            .flat_map(|r| r.events.clone())
            .collect();
        let tmp = app_responses
            .iter()
            .map(|r| r.data.clone())
            .filter(|d| d.is_some())
            .collect::<Vec<_>>();
        let last_data = tmp.last().unwrap_or(&None);
        let data = match last_data {
            Some(d) => S::decode(d.as_slice()).unwrap(),
            None => S::default(),
        };
        let raw_data = data.encode_to_vec();
        let runner_res = test_tube::ExecuteResponse {
            data,
            events,
            raw_data,
            gas_info: GasInfo {
                gas_wanted: 0,
                gas_used: 0,
            },
        };

        Ok(runner_res)
    }

    fn execute_multiple<M, R>(
        &self,
        msgs: &[(M, &str)],
        signer: &test_tube::SigningAccount,
    ) -> test_tube::RunnerExecuteResult<R>
    where
        M: prost::Message,
        R: prost::Message + Default,
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
        signer: &test_tube::SigningAccount,
    ) -> test_tube::RunnerExecuteResult<R>
    where
        R: prost::Message + Default,
    {
        let msgs = msgs
            .iter()
            .map(|msg| match msg.type_url.as_str() {
                // WasmMsg
                MsgExecuteContract::TYPE_URL => {
                    let msg = MsgExecuteContract::decode(msg.value.as_slice())
                        .map_err(DecodeError::ProtoDecodeError)?;
                    Ok(CosmosMsg::<Empty>::Wasm(WasmMsg::Execute {
                        contract_addr: msg.contract,
                        msg: Binary(msg.msg),
                        funds: msg
                            .funds
                            .into_iter()
                            .map(|c| coin(u128::from_str(&c.amount).unwrap(), c.denom))
                            .collect(),
                    }))
                }
                MsgInstantiateContract::TYPE_URL => {
                    let msg = MsgInstantiateContract::decode(msg.value.as_slice())
                        .map_err(DecodeError::ProtoDecodeError)?;
                    Ok(CosmosMsg::<Empty>::Wasm(WasmMsg::Instantiate {
                        code_id: msg.code_id,
                        admin: Some(msg.admin),
                        msg: Binary(msg.msg),
                        funds: msg
                            .funds
                            .into_iter()
                            .map(|c| coin(u128::from_str(&c.amount).unwrap(), c.denom))
                            .collect(),
                        label: msg.label,
                    }))
                }
                MsgMigrateContract::TYPE_URL => {
                    let msg = MsgMigrateContract::decode(msg.value.as_slice())
                        .map_err(DecodeError::ProtoDecodeError)?;
                    Ok(CosmosMsg::<Empty>::Wasm(WasmMsg::Migrate {
                        contract_addr: msg.contract,
                        new_code_id: msg.code_id,
                        msg: Binary(msg.msg),
                    }))
                }
                MsgUpdateAdmin::TYPE_URL => {
                    let msg = MsgUpdateAdmin::decode(msg.value.as_slice())
                        .map_err(DecodeError::ProtoDecodeError)?;
                    Ok(CosmosMsg::<Empty>::Wasm(WasmMsg::UpdateAdmin {
                        contract_addr: msg.contract,
                        admin: msg.new_admin,
                    }))
                }
                MsgClearAdmin::TYPE_URL => {
                    let msg = MsgClearAdmin::decode(msg.value.as_slice())
                        .map_err(DecodeError::ProtoDecodeError)?;
                    Ok(CosmosMsg::<Empty>::Wasm(WasmMsg::ClearAdmin {
                        contract_addr: msg.contract,
                    }))
                }
                // BankMsg
                MsgSend::TYPE_URL => {
                    let msg = MsgSend::decode(msg.value.as_slice())
                        .map_err(DecodeError::ProtoDecodeError)?;
                    Ok(CosmosMsg::<Empty>::Bank(BankMsg::Send {
                        to_address: msg.to_address,
                        amount: msg
                            .amount
                            .into_iter()
                            .map(|c| coin(u128::from_str(&c.amount).unwrap(), c.denom))
                            .collect(),
                    }))
                }
                // StakingMsg
                MsgDelegate::TYPE_URL => {
                    let msg = MsgDelegate::decode(msg.value.as_slice())
                        .map_err(DecodeError::ProtoDecodeError)?;
                    let proto_coin = msg.amount.unwrap_or_default();
                    Ok(CosmosMsg::<Empty>::Staking(StakingMsg::Delegate {
                        validator: msg.validator_address,
                        amount: coin(
                            u128::from_str(&proto_coin.amount).unwrap(),
                            proto_coin.denom,
                        ),
                    }))
                }
                MsgUndelegate::TYPE_URL => {
                    let msg = MsgUndelegate::decode(msg.value.as_slice())
                        .map_err(DecodeError::ProtoDecodeError)?;
                    let proto_coin = msg.amount.unwrap_or_default();
                    Ok(CosmosMsg::<Empty>::Staking(StakingMsg::Undelegate {
                        validator: msg.validator_address,
                        amount: coin(
                            u128::from_str(&proto_coin.amount).unwrap(),
                            proto_coin.denom,
                        ),
                    }))
                }
                MsgBeginRedelegate::TYPE_URL => {
                    let msg = MsgBeginRedelegate::decode(msg.value.as_slice())
                        .map_err(DecodeError::ProtoDecodeError)?;
                    let proto_coin = msg.amount.unwrap_or_default();
                    Ok(CosmosMsg::<Empty>::Staking(StakingMsg::Redelegate {
                        src_validator: msg.validator_src_address,
                        dst_validator: msg.validator_dst_address,
                        amount: coin(
                            u128::from_str(&proto_coin.amount).unwrap(),
                            proto_coin.denom,
                        ),
                    }))
                }
                _ => {
                    // Else assume StargateMsg
                    Ok(CosmosMsg::<Empty>::Stargate {
                        type_url: msg.type_url.clone(),
                        value: msg.value.clone().into(),
                    })
                }
            })
            .collect::<Result<Vec<_>, RunnerError>>()?;

        self.execute_cosmos_msgs(&msgs, signer)
    }

    fn query<Q, R>(&self, path: &str, query: &Q) -> test_tube::RunnerResult<R>
    where
        Q: prost::Message,
        R: prost::Message + DeserializeOwned + Default,
    {
        let querier = self.app.wrap();

        querier
            .query::<R>(&QueryRequest::Stargate {
                path: path.to_string(),
                data: query.encode_to_vec().into(),
            })
            .map_err(|e| RunnerError::GenericError(e.to_string()))
    }

    fn execute_tx(
        &self,
        _tx_bytes: &[u8],
    ) -> test_tube::RunnerResult<cosmrs::proto::tendermint::v0_37::abci::ResponseDeliverTx> {
        todo!()
    }
}

impl<'a> CwItRunner<'a> for MultiTestRunner<'a> {
    fn store_code(
        &self,
        code: ContractType,
        _signer: &SigningAccount,
    ) -> Result<u64, anyhow::Error> {
        match code {
            ContractType::MultiTestContract(contract) => Ok(self.app.store_code(contract)),
            ContractType::Artifact(_) => bail!("Artifact not supported for MultiTestRunner"),
        }
    }

    fn init_account(&self, initial_balance: &[Coin]) -> Result<SigningAccount, anyhow::Error> {
        // Create a random signing account
        let signing_key = SigningKey::random();
        let account = SigningAccount::new(
            self.address_prefix.to_string(),
            signing_key,
            FeeSetting::Auto {
                gas_price: coin(0, "coin"),
                gas_adjustment: 1.0,
            },
        );

        // Mint the initial balances to the account
        if !initial_balance.is_empty() {
            self.app
                .sudo(
                    BankSudo::Mint {
                        to_address: account.address(),
                        amount: initial_balance.to_vec(),
                    }
                    .into(),
                )
                .unwrap();
        }

        Ok(account)
    }

    fn init_accounts(
        &self,
        initial_balance: &[Coin],
        num_accounts: usize,
    ) -> Result<Vec<SigningAccount>, anyhow::Error> {
        let mut accounts = vec![];
        for _ in 0..num_accounts {
            accounts.push(self.init_account(initial_balance)?);
        }
        Ok(accounts)
    }

    fn increase_time(&self, seconds: u64) -> Result<(), anyhow::Error> {
        self.app.update_block(|block| {
            block.time = block.time.plus_seconds(seconds);
            block.height += 1;
        });

        Ok(())
    }

    fn query_block_time_nanos(&self) -> u64 {
        self.app.block_info().time.nanos()
    }
}

#[cfg(test)]
mod tests {
    use cosmrs::proto::cosmos::bank::v1beta1::MsgSendResponse;
    use cosmwasm_std::{coin, Event, Uint128};

    use crate::test_helpers::*;
    use crate::{artifact::Artifact, helpers::upload_wasm_file};
    use apollo_cw_multi_test::ContractWrapper;

    use cw20::MinterResponse;
    use osmosis_std::types::cosmos::bank::v1beta1::{
        QueryBalanceRequest, QuerySupplyOfRequest, QueryTotalSupplyRequest,
    };
    use osmosis_std::types::cosmwasm::wasm::v1::QueryContractInfoRequest;
    use osmosis_std::types::{
        cosmos::bank::v1beta1::QueryAllBalancesRequest,
        cosmwasm::wasm::v1::MsgInstantiateContractResponse,
    };
    use test_tube::{Bank, Module, RunnerExecuteResult, Wasm};

    use super::*;

    fn instantiate_astro_token(
        app: &MultiTestRunner,
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgInstantiateContractResponse> {
        let code_id = upload_wasm_file(
            app,
            signer,
            ContractType::MultiTestContract(Box::new(ContractWrapper::new(
                cw20_base::contract::execute,
                cw20_base::contract::instantiate,
                cw20_base::contract::query,
            ))),
        )
        .unwrap();

        let init_msg = cw20_base::msg::InstantiateMsg {
            name: "Astro Token".to_string(),
            symbol: "ASTRO".to_string(),
            decimals: 6,
            initial_balances: vec![],
            mint: Some(MinterResponse {
                minter: signer.address(),
                cap: None,
            }),
            marketing: None,
        };

        let wasm = Wasm::new(app);
        wasm.instantiate(code_id, &init_msg, None, Some("counter"), &[], signer)
    }

    #[test]
    fn upload_contract() {
        let contract = ContractType::MultiTestContract(test_contract::contract());

        let app = MultiTestRunner::new("osmo");
        let alice = app.init_account(&[coin(1000, "uosmo")]).unwrap();

        let code_id = app.store_code(contract, &alice).unwrap();

        assert_eq!(code_id, 1);
    }

    #[test]
    #[should_panic]
    // This test should panic because we are trying to upload a wasm contract to a MultiTestRunner
    // which does not support wasm contracts.
    fn upload_wasm_artifact() {
        let app = MultiTestRunner::new("osmo");
        let alice = app.init_account(&[coin(1000, "uosmo")]).unwrap();

        let _code_id = upload_wasm_file(
            &app,
            &alice,
            ContractType::Artifact(Artifact::Local(counter::WASM_PATH.to_string())),
        )
        .unwrap();
    }

    #[test]
    // This test should panic because we are trying to upload a wasm contract to a MultiTestRunner
    // which does not support wasm contracts.
    fn wasm_instantiate_contract() {
        let app = MultiTestRunner::new("osmo");
        let alice = app.init_account(&[coin(1000, "uosmo")]).unwrap();

        // Instantiate with test_tube::Wasm
        let res = instantiate_astro_token(&app, &alice).unwrap();
        assert_eq!(res.events.len(), 1);
        assert_eq!(res.events[0].ty, "instantiate".to_string());
    }

    #[test]
    fn wasm_execute_contract() {
        // start the keeper
        let app = MultiTestRunner::new("osmo");

        let alice = app.init_account(&[coin(1000, "uosmo")]).unwrap();

        let res = instantiate_astro_token(&app, &alice).unwrap();

        let contract_addr = res.data.address;

        let wasm = Wasm::new(&app);
        let res = wasm
            .execute(
                &contract_addr,
                &cw20_base::msg::ExecuteMsg::Mint {
                    recipient: alice.address(),
                    amount: 100u128.into(),
                },
                &[],
                &alice,
            )
            .unwrap();
        assert_eq!(res.events.len(), 2);

        let wasm_event = res.events.iter().find(|e| e.ty == "wasm").unwrap();
        assert_eq!(
            wasm_event,
            &Event::new("wasm")
                .add_attribute("_contract_address", contract_addr)
                .add_attribute("action", "mint")
                .add_attribute("to", alice.address())
                .add_attribute("amount", "100")
        );
    }

    #[test]
    fn wasm_smart_query_contract() {
        let app = MultiTestRunner::new("osmo");

        let alice = app.init_account(&[coin(1000, "uosmo")]).unwrap();

        let res = instantiate_astro_token(&app, &alice).unwrap();

        let contract_addr = res.data.address;

        let wasm = Wasm::new(&app);

        let _code_id = upload_wasm_file(
            &app,
            &alice,
            ContractType::MultiTestContract(Box::new(ContractWrapper::new(
                cw20_base::contract::execute,
                cw20_base::contract::instantiate,
                cw20_base::contract::query,
            ))),
        )
        .unwrap();

        let res = wasm
            .query::<_, cw20::BalanceResponse>(
                &contract_addr,
                &cw20_base::msg::QueryMsg::Balance {
                    address: alice.address(),
                },
            )
            .unwrap();

        assert_eq!(res.balance, Uint128::zero());
    }

    #[test]
    fn wasm_contract_info_query() {
        let app = MultiTestRunner::new("osmo");

        let alice = app.init_account(&[coin(1000, "uosmo")]).unwrap();

        let res = instantiate_astro_token(&app, &alice).unwrap();
        println!("MsgInstantiateContractRes: {:?}", res);
        let contract_addr = res.data.address;

        let res = QueryContractInfoRequest {
            address: contract_addr.clone(),
        }
        .query(&app.app.wrap())
        .unwrap();

        println!("QueryContractInfoRes: {:?}", res);

        assert_eq!(res.address, contract_addr);
        let info = res.contract_info.unwrap();
        assert_eq!(info.code_id, 1);
        assert_eq!(info.creator, alice.address());
        assert_eq!(info.label, "");
    }

    #[test]
    fn bank_send() {
        let app = MultiTestRunner::new("osmo");
        let alice = app.init_account(&[coin(1000, "uatom")]).unwrap();
        let bob = app.init_account(&[]).unwrap();

        let msgs = vec![cosmwasm_std::CosmosMsg::Bank(cosmwasm_std::BankMsg::Send {
            to_address: bob.address(),
            amount: vec![cosmwasm_std::Coin {
                denom: "uatom".to_string(),
                amount: 100u128.into(),
            }],
        })];

        let res = app
            .execute_cosmos_msgs::<MsgSendResponse>(&msgs, &alice)
            .unwrap();

        assert_eq!(res.events.len(), 1);
        assert_eq!(
            res.events[0],
            Event::new("transfer")
                .add_attribute("recipient", bob.address())
                .add_attribute("sender", alice.address())
                .add_attribute("amount", "100uatom")
        );

        let bank = Bank::new(&app);
        let res = bank
            .send(
                MsgSend {
                    from_address: alice.address(),
                    to_address: bob.address(),
                    amount: vec![coin(100, "uatom").into()],
                },
                &alice,
            )
            .unwrap();
        assert_eq!(res.events.len(), 1);
        assert_eq!(
            res.events[0],
            Event::new("transfer")
                .add_attribute("recipient", bob.address())
                .add_attribute("sender", alice.address())
                .add_attribute("amount", "100uatom")
        );
    }

    #[test]
    fn bank_queries() {
        let app = MultiTestRunner::new("osmo");
        let alice = app.init_account(&[coin(1000, "uatom")]).unwrap();

        let bank = Bank::new(&app);

        // Query all balances
        let res = bank
            .query_all_balances(&QueryAllBalancesRequest {
                address: alice.address(),
                pagination: None,
            })
            .unwrap();
        assert_eq!(res.balances.len(), 1);
        assert_eq!(res.balances[0].denom, "uatom".to_string());
        assert_eq!(res.balances[0].amount, "1000");

        // Query balance
        let res = bank
            .query_balance(&QueryBalanceRequest {
                address: alice.address(),
                denom: "uatom".to_string(),
            })
            .unwrap()
            .balance
            .unwrap();
        assert_eq!(res.denom, "uatom".to_string());
        assert_eq!(res.amount, "1000");

        // Query total supply should fail since there is no cosmwasm bank query for it
        let _res = bank
            .query_total_supply(&QueryTotalSupplyRequest { pagination: None })
            .unwrap_err();

        // Query supply of
        let supply = QuerySupplyOfRequest {
            denom: "uatom".to_string(),
        }
        .query(&app.app.wrap())
        .unwrap()
        .amount
        .unwrap();
        assert_eq!(supply.denom, "uatom".to_string());
        assert_eq!(supply.amount, "1000");
    }

    #[test]
    fn query_bank_through_test_tube_bank_module() {
        let app = MultiTestRunner::new("osmo");
        let alice = app.init_account(&[coin(1000, "uatom")]).unwrap();

        let bank = Bank::new(&app);

        let res = bank
            .query_all_balances(&QueryAllBalancesRequest {
                address: alice.address(),
                pagination: None,
            })
            .unwrap();

        assert_eq!(res.balances.len(), 1);
        assert_eq!(res.balances[0].denom, "uatom".to_string());
        assert_eq!(res.balances[0].amount, "1000");
    }

    #[test]
    fn test_increase_time() {
        let app = MultiTestRunner::new("osmo");

        let time = app.app.block_info().time;
        app.increase_time(69).unwrap();
        assert_eq!(app.app.block_info().time.seconds(), time.seconds() + 69);
    }
}

