use anyhow::bail;
use cosmwasm_std::{
    from_binary, to_binary, AllBalanceResponse, Api, BalanceResponse, BankQuery, Binary, BlockInfo,
    Empty, Querier, Storage, SupplyResponse,
};
use cw_multi_test::{BankKeeper, Module, StargateKeeper, StargateQueryHandler};
use osmosis_test_tube::osmosis_std::types::cosmos::bank::v1beta1::{
    QueryAllBalancesRequest, QueryAllBalancesResponse, QueryBalanceRequest, QueryBalanceResponse,
    QuerySupplyOfRequest, QuerySupplyOfResponse,
};

// Temp solution to get the query service paths. TODO: Figure out how to get this from the proto files (PR to osmosis-rust?)
const QUERY_ALL_BALANCES_PATH: &str = "/cosmos.bank.v1beta1.Query/AllBalances";
const QUERY_BALANCE_PATH: &str = "/cosmos.bank.v1beta1.Query/Balance";
const QUERY_SUPPLY_PATH: &str = "/cosmos.bank.v1beta1.Query/SupplyOf";

#[derive(Clone)]
pub struct BankModule(pub BankKeeper);

impl StargateQueryHandler for BankModule {
    fn stargate_query(
        &self,
        api: &dyn Api,
        storage: &dyn Storage,
        querier: &dyn Querier,
        block: &BlockInfo,
        request: cw_multi_test::StargateMsg,
    ) -> anyhow::Result<Binary> {
        match request.type_url.as_str() {
            QUERY_ALL_BALANCES_PATH => {
                let msg: QueryAllBalancesRequest = request.value.try_into()?;
                let bin_res = self.0.query(
                    api,
                    storage,
                    querier,
                    block,
                    BankQuery::AllBalances {
                        address: msg.address,
                    },
                )?;

                let bank_res: AllBalanceResponse = from_binary(&bin_res)?;

                let res = QueryAllBalancesResponse {
                    balances: bank_res
                        .amount
                        .into_iter()
                        .map(|c| c.into())
                        .collect::<Vec<_>>(),
                    pagination: None,
                };
                Ok(to_binary(&res)?)
            }
            QUERY_BALANCE_PATH => {
                let req: QueryBalanceRequest = request.value.try_into()?;
                let bin_res = self.0.query(
                    api,
                    storage,
                    querier,
                    block,
                    BankQuery::Balance {
                        address: req.address,
                        denom: req.denom,
                    },
                )?;

                let res: BalanceResponse = from_binary(&bin_res)?;
                let res = QueryBalanceResponse {
                    balance: Some(res.amount.into()),
                };

                Ok(to_binary(&res)?)
            }
            QUERY_SUPPLY_PATH => {
                let req: QuerySupplyOfRequest = request.value.try_into()?;
                let req = BankQuery::Supply { denom: req.denom };

                let bin_res = self.0.query(api, storage, querier, block, req)?;

                let res: SupplyResponse = from_binary(&bin_res)?;
                let res = QuerySupplyOfResponse {
                    amount: Some(res.amount.into()),
                };

                Ok(to_binary(&res)?)
            }
            _ => bail!("Unsupported bank query: {}", request.type_url),
        }
    }

    fn register_queries(&'static self, keeper: &mut StargateKeeper<Empty, Empty>) {
        keeper.register_query(QUERY_ALL_BALANCES_PATH, Box::new(self.clone()));
        keeper.register_query(QUERY_BALANCE_PATH, Box::new(self.clone()));
        keeper.register_query(QUERY_SUPPLY_PATH, Box::new(self.clone()));
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{coin, Addr, Coin, Empty};
    use cw_multi_test::BasicAppBuilder;
    use std::str::FromStr;

    use super::*;

    const BANK_KEEPER: BankModule = BankModule(BankKeeper {});

    #[test]
    fn query_bank_module_via_stargate() {
        let mut stargate_keeper = StargateKeeper::new();

        BANK_KEEPER.register_queries(&mut stargate_keeper);

        let owner = Addr::unchecked("owner");
        let init_funds = vec![coin(20, "btc"), coin(100, "eth")];

        let app = BasicAppBuilder::<Empty, Empty>::new()
            .with_stargate(stargate_keeper)
            .build(|router, _, storage| {
                router
                    .bank
                    .init_balance(storage, &owner, init_funds.clone())
                    .unwrap();
            });

        let querier = app.wrap();

        // QueryAllBalancesRequest
        let res = QueryAllBalancesRequest {
            address: owner.to_string(),
            pagination: None,
        }
        .query(&querier)
        .unwrap();
        let blances: Vec<Coin> = res
            .balances
            .into_iter()
            .map(|c| Coin::new(u128::from_str(&c.amount).unwrap(), c.denom))
            .collect();
        assert_eq!(blances, init_funds);

        // QueryBalanceRequest
        let res = QueryBalanceRequest {
            address: owner.to_string(),
            denom: "eth".to_string(),
        }
        .query(&querier)
        .unwrap();
        let balance = res.balance.unwrap();
        assert_eq!(balance.amount, init_funds[1].amount.to_string());
        assert_eq!(balance.denom, init_funds[1].denom);

        // QueryTotalSupplyRequest
        let res = QuerySupplyOfRequest {
            denom: "eth".to_string(),
        };
        let res = res.query(&querier).unwrap();
        let supply = res.amount.unwrap();
        assert_eq!(supply.amount, init_funds[1].amount.to_string());
        assert_eq!(supply.denom, init_funds[1].denom);
    }
}
