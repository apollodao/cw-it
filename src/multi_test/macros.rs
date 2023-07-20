#[macro_export]
macro_rules! create_contract_wrappers {
    ( $( $name:expr ),* ) => {{
        use std::collections::HashMap;
        use apollo_cw_multi_test::{ContractWrapper, Contract};
        use cosmwasm_std::Empty;
        vec![
            $(
                {

                    paste::paste! {
                      use[<$name>]::contract::{execute, instantiate, query};
                    }
                    ($name.to_string(), Box::new(ContractWrapper::new_with_empty(
                        execute,
                        instantiate,
                        query,
                    )) as Box<dyn Contract<Empty>>)
                }
            ),*
        ].into_iter().collect::<HashMap<String,Box<dyn Contract<Empty>>>>()
    }};
}

#[macro_export]
macro_rules! create_contract_wrappers_with_reply {
    ( $( $name:expr ),* ) => {{
        use std::collections::HashMap;
        use apollo_cw_multi_test::{ContractWrapper, Contract};
        use cosmwasm_std::Empty;
        vec![
            $(
                {

                    paste::paste! {
                      use[<$name>]::contract::{execute, instantiate, query, reply};
                    }
                    ($name.to_string(), Box::new(ContractWrapper::new_with_empty(
                        execute,
                        instantiate,
                        query,
                    ).with_reply(reply)) as Box<dyn Contract<Empty>>)
                }
            ),*
        ].into_iter().collect::<HashMap<String,Box<dyn Contract<Empty>>>>()
    }};
}

#[cfg(feature = "astroport")]
#[cfg(test)]
mod tests {
    #[test]
    fn test_create_contract_wrappers_macro() {
        let contract_wrappers =
            create_contract_wrappers!("astroport_factory", "astroport-pair-stable");

        assert_eq!(contract_wrappers.len(), 2);
    }

    #[test]
    fn test_create_contract_wrappers_with_reply_macro() {
        let contract_wrappers =
            create_contract_wrappers_with_reply!("astroport_factory", "astroport-pair-stable");

        assert_eq!(contract_wrappers.len(), 2);
    }
}
