#[cfg(feature = "multi-test")]
pub mod test_contract {
    use std::fmt;

    use cosmwasm_schema::{cw_serde, schemars::JsonSchema};
    use cosmwasm_std::{
        Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, SubMsg, WasmMsg,
    };
    use cw_multi_test::{Contract, ContractWrapper};

    #[cw_serde]
    pub struct EmptyMsg {}

    fn instantiate(
        _deps: DepsMut,
        _env: Env,
        _info: MessageInfo,
        _msg: EmptyMsg,
    ) -> Result<Response, StdError> {
        Ok(Response::default())
    }

    fn execute(
        _deps: DepsMut,
        _env: Env,
        _info: MessageInfo,
        msg: WasmMsg,
    ) -> Result<Response, StdError> {
        let message = SubMsg::new(msg);

        Ok(Response::new().add_submessage(message))
    }

    fn query(_deps: Deps, _env: Env, _msg: EmptyMsg) -> Result<Binary, StdError> {
        Err(StdError::generic_err(
            "query not implemented for the `test_contract` contract",
        ))
    }

    pub fn contract<C>() -> Box<dyn Contract<C>>
    where
        C: Clone + fmt::Debug + PartialEq + JsonSchema + 'static,
    {
        let contract = ContractWrapper::new_with_empty(execute, instantiate, query);
        Box::new(contract)
    }
}

pub mod counter {
    use cosmwasm_schema::{cw_serde, QueryResponses};

    #[cw_serde]
    pub struct InstantiateMsg {
        pub count: i32,
    }

    #[cw_serde]
    pub enum ExecuteMsg {
        Increment {},
        Reset { count: i32 },
    }

    #[cw_serde]
    #[derive(QueryResponses)]
    pub enum QueryMsg {
        // GetCount returns the current count as a json-encoded number
        #[returns(GetCountResponse)]
        GetCount {},
    }

    // We define a custom struct for each query response
    #[cw_serde]
    pub struct GetCountResponse {
        pub count: i32,
    }

    pub const WASM_PATH: &str = "artifacts/counter.wasm";
}
