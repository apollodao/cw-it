use cosmrs::{crypto::secp256k1::SigningKey, proto::cosmos::base::abci::v1beta1::GasInfo};
use cosmwasm_std::{coin, Addr, Coin, Empty, QueryRequest};
use cw_multi_test::{BankKeeper, BankSudo, BasicAppBuilder, StargateKeeper, StargateQueryHandler};
use serde::de::DeserializeOwned;
use test_tube::{Account, FeeSetting, Runner, RunnerError, RunnerResult, SigningAccount};

use super::modules::BankModule;

pub struct MultiTestRunner<'a> {
    pub app: cw_multi_test::App,
    pub address_prefix: &'a str,
}

const BANK_MODULE: BankModule = BankModule(BankKeeper {});

impl<'a> MultiTestRunner<'a> {
    pub fn new(address_prefix: &'a str) -> Self {
        // Setup stargate keeper with bank module support
        let mut stargate_keeper = StargateKeeper::new();
        BANK_MODULE.register_queries(&mut stargate_keeper);

        // Construct app
        let app = BasicAppBuilder::<Empty, Empty>::new()
            .with_stargate(stargate_keeper)
            .build(|_, _, _| {});

        Self {
            app,
            address_prefix,
        }
    }

    // TODO: move to trait
    pub fn init_account(&mut self, initial_balance: &[Coin]) -> RunnerResult<SigningAccount> {
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
        // TODO: dont unwrap
        self.app
            .sudo(
                BankSudo::Mint {
                    to_address: account.address(),
                    amount: initial_balance.to_vec(),
                }
                .into(),
            )
            .unwrap();

        Ok(account)
    }
}

impl Runner<'_> for MultiTestRunner<'_> {
    fn execute_cosmos_msgs<S>(
        &mut self,
        msgs: &[cosmwasm_std::CosmosMsg],
        signer: &test_tube::SigningAccount,
    ) -> test_tube::RunnerExecuteResult<S>
    where
        S: prost::Message + Default,
    {
        let sender = Addr::unchecked(signer.address());

        // TODO: dont unwrap
        let app_responses = self.app.execute_multi(sender, msgs.to_vec()).unwrap();
        let events = app_responses
            .iter()
            .map(|r| r.events.clone())
            .flatten()
            .collect();
        let tmp = app_responses
            .iter()
            .map(|r| r.data.clone())
            .filter(|d| d.is_some())
            .collect::<Vec<_>>();
        let last_data = tmp.last().unwrap();

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
        unimplemented!("cannot execute prost messages with MultiTestRunner")
    }

    fn execute_multiple_raw<R>(
        &self,
        msgs: Vec<cosmrs::Any>,
        signer: &test_tube::SigningAccount,
    ) -> test_tube::RunnerExecuteResult<R>
    where
        R: prost::Message + Default,
    {
        unimplemented!("cannot execute cosmrs::Any messages with MultiTestRunner")
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
}

#[cfg(test)]
mod tests {
    use cosmrs::proto::cosmos::bank::v1beta1::MsgSendResponse;
    use cosmwasm_std::coin;
    use osmosis_std::types::cosmos::bank::v1beta1::QueryAllBalancesRequest;
    use test_tube::{Bank, Module};

    use super::*;

    #[test]
    fn bank_send() {
        let mut app = MultiTestRunner::new("osmo");
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

        assert_eq!(res.events.len(), 2);
        assert_eq!(res.events[0].ty, "message");
        assert_eq!(res.events[1].ty, "transfer");
    }

    #[test]
    fn query_bank_through_test_tube_bank_module() {
        let mut app = MultiTestRunner::new("osmo");
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
}
