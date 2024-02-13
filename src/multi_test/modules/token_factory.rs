use std::str::FromStr;

use anyhow::{bail, Ok};
use cosmwasm_std::{
    from_json, Addr, Api, BankMsg, BankQuery, BlockInfo, Coin, Empty, Event, QueryRequest, Storage,
    SupplyResponse, Uint128,
};
use osmosis_std::types::osmosis::tokenfactory::v1beta1::{
    MsgBurn, MsgBurnResponse, MsgCreateDenom, MsgCreateDenomResponse, MsgMint, MsgMintResponse,
};
use regex::Regex;

use apollo_cw_multi_test::{
    AppResponse, BankSudo, CosmosRouter, StargateKeeper, StargateMessageHandler, StargateMsg,
};

/// This is a struct that implements the [`apollo_cw_multi_test::StargateMessageHandler`] trait to
/// mimic the behavior of the Osmosis TokenFactory module version 0.15.
#[derive(Clone)]
pub struct TokenFactory<'a> {
    pub module_denom_prefix: &'a str,
    pub max_subdenom_len: usize,
    pub max_hrp_len: usize,
    pub max_creator_len: usize,
    pub denom_creation_fee: &'a str,
}

impl<'a> TokenFactory<'a> {
    /// Creates a new TokenFactory instance with the given parameters.
    pub const fn new(
        prefix: &'a str,
        max_subdenom_len: usize,
        max_hrp_len: usize,
        max_creator_len: usize,
        denom_creation_fee: &'a str,
    ) -> Self {
        Self {
            module_denom_prefix: prefix,
            max_subdenom_len,
            max_hrp_len,
            max_creator_len,
            denom_creation_fee,
        }
    }
}

impl Default for TokenFactory<'_> {
    fn default() -> Self {
        Self::new("factory", 32, 16, 59 + 16, "10000000uosmo")
    }
}

impl TokenFactory<'_> {
    fn create_denom(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = Empty, QueryC = Empty>,
        block: &BlockInfo,
        sender: Addr,
        msg: StargateMsg,
    ) -> anyhow::Result<AppResponse> {
        let msg: MsgCreateDenom = msg.value.try_into()?;

        //Validate subdenom length
        if msg.subdenom.len() > self.max_subdenom_len {
            bail!(
                "Subdenom length is too long, max length is {}",
                self.max_subdenom_len
            );
        }
        // Validate creator length
        if msg.sender.len() > self.max_creator_len {
            bail!(
                "Creator length is too long, max length is {}",
                self.max_creator_len
            );
        }
        // Validate creator address not contains '/'
        if msg.sender.contains('/') {
            bail!("Invalid creator address, creator address cannot contains '/'");
        }
        // Validate sender is the creator
        if msg.sender != sender {
            bail!("Invalid creator address, creator address must be the same as the sender");
        }

        let denom = format!(
            "{}/{}/{}",
            self.module_denom_prefix, msg.sender, msg.subdenom
        );

        println!("denom: {}", denom);

        // Query supply of denom
        let request = QueryRequest::Bank(BankQuery::Supply {
            denom: denom.clone(),
        });
        let raw = router.query(api, storage, block, request)?;
        let supply: SupplyResponse = from_json(raw)?;
        println!("supply: {:?}", supply);
        println!(
            "supply.amount.amount.is_zero: {:?}",
            supply.amount.amount.is_zero()
        );
        if !supply.amount.amount.is_zero() {
            println!("bailing");
            bail!("Subdenom already exists");
        }

        // Charge denom creation fee
        let fee = coin_from_sdk_string(self.denom_creation_fee)?;
        let fee_msg = BankMsg::Burn { amount: vec![fee] };
        router.execute(api, storage, block, sender, fee_msg.into())?;

        let create_denom_response = MsgCreateDenomResponse {
            new_token_denom: denom.clone(),
        };

        let mut res = AppResponse::default();
        res.events.push(
            Event::new("create_denom")
                .add_attribute("creator", msg.sender)
                .add_attribute("new_token_denom", denom),
        );
        res.data = Some(create_denom_response.into());

        Ok(res)
    }

    pub fn mint(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = Empty, QueryC = Empty>,
        block: &BlockInfo,
        sender: Addr,
        msg: StargateMsg,
    ) -> anyhow::Result<AppResponse> {
        let msg: MsgMint = msg.value.try_into()?;

        let denom = msg.amount.clone().unwrap().denom;

        // Validate sender
        let parts = denom.split('/').collect::<Vec<_>>();
        if parts[1] != sender {
            bail!("Unauthorized mint. Not the creator of the denom.");
        }
        if sender != msg.sender {
            bail!("Invalid sender. Sender in msg must be same as sender of transaction.");
        }

        // Validate denom
        if parts.len() != 3 && parts[0] != self.module_denom_prefix {
            bail!("Invalid denom");
        }

        let amount = Uint128::from_str(&msg.amount.unwrap().amount)?;
        if amount.is_zero() {
            bail!("Invalid zero amount");
        }

        // Mint through BankKeeper sudo method
        let mint_msg = BankSudo::Mint {
            to_address: sender.to_string(),
            amount: vec![Coin {
                denom: denom.clone(),
                amount,
            }],
        };
        router.sudo(api, storage, block, mint_msg.into())?;

        let mut res = AppResponse::default();
        let data = MsgMintResponse {};
        res.data = Some(data.into());
        res.events.push(
            Event::new("tf_mint")
                .add_attribute("mint_to_address", "sender")
                .add_attribute("amount", amount.to_string()),
        );
        Ok(res)
    }

    pub fn burn(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = Empty, QueryC = Empty>,
        block: &BlockInfo,
        sender: Addr,
        msg: StargateMsg,
    ) -> anyhow::Result<AppResponse> {
        let msg: MsgBurn = msg.value.try_into()?;

        // Validate sender
        let denom = msg.amount.clone().unwrap().denom;
        let parts = denom.split('/').collect::<Vec<_>>();
        if parts[1] != sender {
            bail!("Unauthorized burn. Not the creator of the denom.");
        }
        if sender != msg.sender {
            bail!("Invalid sender. Sender in msg must be same as sender of transaction.");
        }

        // Validate denom
        if parts.len() != 3 && parts[0] != self.module_denom_prefix {
            bail!("Invalid denom");
        }

        let amount = Uint128::from_str(&msg.amount.unwrap().amount)?;
        if amount.is_zero() {
            bail!("Invalid zero amount");
        }

        // Burn through BankKeeper
        let burn_msg = BankMsg::Burn {
            amount: vec![Coin {
                denom: denom.clone(),
                amount,
            }],
        };
        router.execute(api, storage, block, sender.clone(), burn_msg.into())?;

        let mut res = AppResponse::default();
        let data = MsgBurnResponse {};
        res.data = Some(data.into());

        res.events.push(
            Event::new("tf_burn")
                .add_attribute("burn_from_address", sender.to_string())
                .add_attribute("amount", amount.to_string()),
        );

        Ok(res)
    }
}

impl StargateMessageHandler<Empty, Empty> for TokenFactory<'_> {
    fn execute(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = Empty, QueryC = Empty>,
        block: &BlockInfo,
        sender: Addr,
        msg: StargateMsg,
    ) -> anyhow::Result<AppResponse> {
        match msg.type_url.as_str() {
            MsgCreateDenom::TYPE_URL => self.create_denom(api, storage, router, block, sender, msg),
            MsgMint::TYPE_URL => self.mint(api, storage, router, block, sender, msg),
            MsgBurn::TYPE_URL => self.burn(api, storage, router, block, sender, msg),
            _ => bail!("Unknown message type {}", msg.type_url),
        }
    }

    fn register_msgs(&'static self, keeper: &mut StargateKeeper<Empty, Empty>) {
        keeper.register_msg(MsgCreateDenom::TYPE_URL, Box::new(self.clone()));
        keeper.register_msg(MsgMint::TYPE_URL, Box::new(self.clone()));
        keeper.register_msg(MsgBurn::TYPE_URL, Box::new(self.clone()));
    }
}

fn coin_from_sdk_string(sdk_string: &str) -> anyhow::Result<Coin> {
    let denom_re = Regex::new(r"^[0-9]+[a-z]+$")?;
    let ibc_re = Regex::new(r"^[0-9]+(ibc|IBC)/[0-9A-F]{64}$")?;
    let factory_re = Regex::new(r"^[0-9]+factory/[0-9a-z]+/[0-9a-zA-Z]+$")?;

    if !(denom_re.is_match(sdk_string)
        || ibc_re.is_match(sdk_string)
        || factory_re.is_match(sdk_string))
    {
        bail!("Invalid sdk string");
    }

    // Parse amount
    let re = Regex::new(r"[0-9]+")?;
    let amount = re.find(sdk_string).unwrap().as_str();
    let amount = Uint128::from_str(amount)?;

    // The denom is the rest of the string
    let denom = sdk_string[amount.to_string().len()..].to_string();

    Ok(Coin { denom, amount })
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{BalanceResponse, Binary, Coin};

    use apollo_cw_multi_test::{BasicAppBuilder, Executor, StargateKeeper};

    use super::*;

    use test_case::test_case;

    const TOKEN_FACTORY: &TokenFactory =
        &TokenFactory::new("factory", 32, 16, 59 + 16, "10000000uosmo");

    #[test_case(Addr::unchecked("sender"), "subdenom", &["10000000uosmo"]; "valid denom")]
    #[test_case(Addr::unchecked("sen/der"), "subdenom", &["10000000uosmo"] => panics "creator address cannot contains" ; "invalid creator address")]
    #[test_case(Addr::unchecked("asdasdasdasdasdasdasdasdasdasdasdasdasdasdasd"), "subdenom", &["10000000uosmo"] => panics ; "creator address too long")]
    #[test_case(Addr::unchecked("sender"), "subdenom", &["10000000uosmo", "100factory/sender/subdenom"] => panics "Subdenom already exists" ; "denom exists")]
    #[test_case(Addr::unchecked("sender"), "subdenom", &["100000uosmo"] => panics "Cannot Sub" ; "insufficient funds for fee")]
    fn create_denom(sender: Addr, subdenom: &str, initial_coins: &[&str]) {
        let initial_coins = initial_coins
            .iter()
            .map(|s| coin_from_sdk_string(s).unwrap())
            .collect::<Vec<_>>();

        let mut stargate_keeper = StargateKeeper::new();
        TOKEN_FACTORY.register_msgs(&mut stargate_keeper);

        let app = BasicAppBuilder::<Empty, Empty>::new()
            .with_stargate(stargate_keeper)
            .build(|router, _, storage| {
                router
                    .bank
                    .init_balance(storage, &sender, initial_coins)
                    .unwrap();
            });

        let msg = StargateMsg {
            type_url: MsgCreateDenom::TYPE_URL.to_string(),
            value: MsgCreateDenom {
                sender: sender.to_string(),
                subdenom: subdenom.to_string(),
            }
            .into(),
        };

        let res = app.execute(sender.clone(), msg.into()).unwrap();

        res.assert_event(
            &Event::new("create_denom")
                .add_attribute("creator", sender.to_string())
                .add_attribute(
                    "new_token_denom",
                    format!(
                        "{}/{}/{}",
                        TOKEN_FACTORY.module_denom_prefix, sender, subdenom
                    ),
                ),
        );

        assert_eq!(
            res.data.unwrap(),
            Binary::from(MsgCreateDenomResponse {
                new_token_denom: format!(
                    "{}/{}/{}",
                    TOKEN_FACTORY.module_denom_prefix, sender, subdenom
                )
            })
        );
    }

    #[test_case(Addr::unchecked("sender"), Addr::unchecked("sender"), 1000u128 ; "valid mint")]
    #[test_case(Addr::unchecked("sender"), Addr::unchecked("sender"), 0u128 => panics "Invalid zero amount" ; "zero amount")]
    #[test_case(Addr::unchecked("sender"), Addr::unchecked("creator"), 1000u128 => panics "Unauthorized mint. Not the creator of the denom." ; "sender is not creator")]
    fn mint(sender: Addr, creator: Addr, mint_amount: u128) {
        let mut stargate_keeper = StargateKeeper::new();
        TOKEN_FACTORY.register_msgs(&mut stargate_keeper);

        let app = BasicAppBuilder::<Empty, Empty>::new()
            .with_stargate(stargate_keeper)
            .build(|_, _, _| {});

        let msg = StargateMsg {
            type_url: MsgMint::TYPE_URL.to_string(),
            value: MsgMint {
                sender: sender.to_string(),
                amount: Some(
                    Coin {
                        denom: format!(
                            "{}/{}/{}",
                            TOKEN_FACTORY.module_denom_prefix, creator, "subdenom"
                        ),
                        amount: Uint128::from(mint_amount),
                    }
                    .into(),
                ),
                mint_to_address: sender.to_string(),
            }
            .into(),
        };

        let res = app.execute(sender.clone(), msg.into()).unwrap();

        // Assert event
        res.assert_event(
            &Event::new("tf_mint")
                .add_attribute("mint_to_address", sender.to_string())
                .add_attribute("amount", "1000"),
        );

        // Query bank balance
        let balance_query = BankQuery::Balance {
            address: sender.to_string(),
            denom: format!(
                "{}/{}/{}",
                TOKEN_FACTORY.module_denom_prefix, creator, "subdenom"
            ),
        };
        let balance = app
            .wrap()
            .query::<BalanceResponse>(&balance_query.into())
            .unwrap()
            .amount
            .amount;
        assert_eq!(balance, Uint128::from(mint_amount));
    }

    #[test_case(Addr::unchecked("sender"), Addr::unchecked("sender"), 1000u128, 1000u128 ; "valid burn")]
    #[test_case(Addr::unchecked("sender"), Addr::unchecked("sender"), 1000u128, 2000u128 ; "valid burn 2")]
    #[test_case(Addr::unchecked("sender"), Addr::unchecked("creator"), 1000u128, 1000u128 => panics "Unauthorized burn. Not the creator of the denom." ; "sender is not creator")]
    #[test_case(Addr::unchecked("sender"), Addr::unchecked("sender"), 0u128, 1000u128 => panics "Invalid zero amount" ; "zero amount")]
    #[test_case(Addr::unchecked("sender"), Addr::unchecked("sender"), 2000u128, 1000u128 => panics "Cannot Sub" ; "insufficient funds")]
    fn burn(sender: Addr, creator: Addr, burn_amount: u128, initial_balance: u128) {
        let mut stargate_keeper = StargateKeeper::new();
        TOKEN_FACTORY.register_msgs(&mut stargate_keeper);

        let tf_denom = format!(
            "{}/{}/{}",
            TOKEN_FACTORY.module_denom_prefix, creator, "subdenom"
        );

        let app = BasicAppBuilder::<Empty, Empty>::new()
            .with_stargate(stargate_keeper)
            .build(|router, _, storage| {
                router
                    .bank
                    .init_balance(
                        storage,
                        &sender,
                        vec![Coin {
                            denom: tf_denom.clone(),
                            amount: Uint128::from(initial_balance),
                        }],
                    )
                    .unwrap();
            });

        // Execute burn
        let msg = StargateMsg {
            type_url: MsgBurn::TYPE_URL.to_string(),
            value: MsgBurn {
                sender: sender.to_string(),
                amount: Some(
                    Coin {
                        denom: tf_denom.clone(),
                        amount: Uint128::from(burn_amount),
                    }
                    .into(),
                ),
                burn_from_address: sender.to_string(),
            }
            .into(),
        };
        let res = app.execute(sender.clone(), msg.into()).unwrap();

        // Assert event
        res.assert_event(
            &Event::new("tf_burn")
                .add_attribute("burn_from_address", sender.to_string())
                .add_attribute("amount", "1000"),
        );

        // Query bank balance
        let balance_query = BankQuery::Balance {
            address: sender.to_string(),
            denom: tf_denom,
        };
        let balance = app
            .wrap()
            .query::<BalanceResponse>(&balance_query.into())
            .unwrap()
            .amount
            .amount;
        assert_eq!(balance.u128(), initial_balance - burn_amount);
    }

    #[test_case("uosmo" ; "native denom")]
    #[test_case("IBC/27394FB092D2ECCD56123C74F36E4C1F926001CEADA9CA97EA622B25F41E5EB2" ; "ibc denom")]
    #[test_case("IBC/27394FB092D2ECCD56123CA622B25F41E5EB2" => panics "Invalid sdk string" ; "invalid ibc denom")]
    #[test_case("IB/27394FB092D2ECCD56123C74F36E4C1F926001CEADA9CA97EA622B25F41E5EB2" => panics "Invalid sdk string" ; "invalid ibc denom 2")]
    #[test_case("factory/sender/subdenom" ; "token factory denom")]
    #[test_case("factory/se1298der/subde192MAnom" ; "token factory denom 2")]
    #[test_case("factor/sender/subdenom" => panics "Invalid sdk string" ; "invalid token factory denom")]
    #[test_case("factory/sender/subdenom/extra" => panics "Invalid sdk string" ; "invalid token factory denom 2")]
    fn test_coin_from_sdk_string(denom: &str) {
        let sdk_string = format!("{}{}", 1000, denom);
        let coin = coin_from_sdk_string(&sdk_string).unwrap();
        assert_eq!(coin.denom, denom);
        assert_eq!(coin.amount, Uint128::from(1000u128));
    }
}
