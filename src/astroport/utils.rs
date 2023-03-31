use crate::config::TestConfig;
use crate::helpers::upload_wasm_files;
use astroport_types::asset::AssetInfo;
use astroport_types::factory::{
    ExecuteMsg as AstroportFactoryExecuteMsg, InstantiateMsg as AstroportFactoryInstantiateMsg,
    PairConfig, PairType,
};
use astroport_types::generator::InstantiateMsg as GeneratorInstantiateMsg;
use astroport_types::maker::InstantiateMsg as MakerInstantiateMsg;
use std::collections::HashMap;

use astroport_types::router::InstantiateMsg as RouterInstantiateMsg;
use astroport_types::staking::InstantiateMsg as StakingInstantiateMsg;
use astroport_types::token::InstantiateMsg as AstroTokenInstantiateMsg;
use astroport_types::vesting::{
    Cw20HookMsg as VestingHookMsg, InstantiateMsg as VestingInstantiateMsg, VestingAccount,
    VestingSchedule, VestingSchedulePoint,
};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{to_binary, Binary, Event, Uint128, Uint64};
use cw20::{Cw20Coin, Cw20ExecuteMsg, MinterResponse};
use osmosis_test_tube::{Account, Module, Runner, SigningAccount, Wasm};

pub const ASTROPORT_CONTRACT_NAMES: [&str; 10] = [
    "astro_token",
    "astroport_factory",
    "astroport_generator",
    "astroport_maker",
    "astroport_pair_stable",
    "astroport_pair_xyk",
    "astroport_router",
    "astroport_staking",
    "astroport_vesting",
    "astroport_whitelist",
];

#[cw_serde]
pub struct Contract {
    pub address: String,
    pub code_id: u64,
}

impl Contract {
    pub fn new(address: String, code_id: u64) -> Self {
        Self { address, code_id }
    }
}

#[cw_serde]
pub struct AstroportContracts {
    pub factory: Contract,
    pub generator: Contract,
    pub astro_token: Contract,
    pub maker: Contract,
    pub pair_stable: Contract,
    pub pair_xyk: Contract,
    pub router: Contract,
    pub staking: Contract,
    pub vesting: Contract,
    pub whitelist: Contract,
}

pub fn setup_astroport<'a, R>(
    app: &'a R,
    test_config: &TestConfig,
    admin: &SigningAccount,
) -> AstroportContracts
where
    R: Runner<'a>,
{
    // Upload contracts
    let code_ids = upload_wasm_files(app, admin, test_config.artifacts.clone()).unwrap();

    // Instantiate contracts
    instantiate_astroport(app, admin, &code_ids)
}

pub fn instantiate_astroport<'a, R>(
    app: &'a R,
    admin: &SigningAccount,
    code_ids: &HashMap<String, u64>,
) -> AstroportContracts
where
    R: Runner<'a>,
{
    let wasm = Wasm::new(app);

    // Instantiate astro token
    println!("Instantiating astro token ...");
    let astro_token = wasm
        .instantiate(
            code_ids["astro_token"],
            &AstroTokenInstantiateMsg {
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: admin.address(),
                    amount: Uint128::from(1000000000000000u128),
                }],
                mint: Some(MinterResponse {
                    minter: admin.address(),
                    cap: None,
                }),
                name: "Astro Token".to_string(),
                symbol: "ASTRO".to_string(),
            },
            Some(&admin.address()),
            Some("Astro Token"),
            &[],
            admin,
        )
        .unwrap()
        .data
        .address;

    // Instantiate factory
    println!("Instantiating factory ...");
    let factory = wasm
        .instantiate(
            code_ids["astroport_factory"],
            &AstroportFactoryInstantiateMsg {
                pair_configs: vec![
                    PairConfig {
                        code_id: code_ids["astroport_pair_xyk"],
                        is_disabled: false,
                        is_generator_disabled: false,
                        maker_fee_bps: 3333,
                        total_fee_bps: 30,
                        pair_type: PairType::Xyk {},
                    },
                    PairConfig {
                        code_id: code_ids["astroport_pair_stable"],
                        is_disabled: false,
                        is_generator_disabled: false,
                        maker_fee_bps: 5000,
                        total_fee_bps: 5,
                        pair_type: PairType::Stable {},
                    },
                ],
                token_code_id: code_ids["astro_token"], // TODO: is this correct or do we need another contract?
                fee_address: None,
                generator_address: None, // TODO: Set this
                owner: admin.address(),
                whitelist_code_id: code_ids["astroport_whitelist"],
            },
            Some(&admin.address()),    // contract admin used for migration
            Some("Astroport Factory"), // contract label
            &[],                       // funds
            admin,                     // signer
        )
        .unwrap()
        .data
        .address;

    // Instantiate vesting
    println!("Instantiating vesting ...");
    let vesting = wasm
        .instantiate(
            code_ids["astroport_vesting"],
            &VestingInstantiateMsg {
                owner: admin.address(),
                token_addr: astro_token.clone(),
            },
            Some(&admin.address()),
            Some("Astroport Vesting"),
            &[],
            admin,
        )
        .unwrap()
        .data
        .address;

    // Instantiate generator
    println!("Instantiating generator ...");
    let generator = wasm
        .instantiate(
            code_ids["astroport_generator"],
            &GeneratorInstantiateMsg {
                owner: admin.address(),
                whitelist_code_id: code_ids["astroport_whitelist"],
                factory: factory.clone(),
                generator_controller: Some(admin.address()),
                allowed_reward_proxies: vec![],
                voting_escrow: None,
                guardian: None,
                astro_token: astro_token.clone(),
                tokens_per_block: Uint128::from(10000000u128),
                start_block: Uint64::one(),
                vesting_contract: vesting.clone(),
            },
            Some(&admin.address()),    // contract admin used for migration
            Some("Astroport Factory"), // contract label
            &[],                       // funds
            admin,                     // signer
        )
        .unwrap()
        .data
        .address;

    // Update factory config to add generator
    println!("Updating factory config to add generator ...");
    let _res = wasm
        .execute(
            &factory,
            &AstroportFactoryExecuteMsg::UpdateConfig {
                generator_address: Some(generator.clone()),
                fee_address: None,
                token_code_id: None,
                whitelist_code_id: None,
            },
            &[],
            admin,
        )
        .unwrap();

    // Instantiate staking
    println!("Instantiating staking ...");
    let staking = wasm
        .instantiate(
            code_ids["astroport_staking"],
            &StakingInstantiateMsg {
                owner: admin.address(),
                deposit_token_addr: astro_token.clone(),
                token_code_id: code_ids["astro_token"],
                marketing: None,
            },
            Some(&admin.address()),    // contract admin used for migration
            Some("Astroport Staking"), // contract label
            &[],                       // funds
            admin,                     // signer
        )
        .unwrap()
        .data
        .address;

    // Instantiate Router
    println!("Instantiating router ...");
    let router = wasm
        .instantiate(
            code_ids["astroport_router"],
            &RouterInstantiateMsg {
                astroport_factory: factory.clone(),
            },
            Some(&admin.address()),   // contract admin used for migration
            Some("Astroport Router"), // contract label
            &[],                      // funds
            admin,                    // signer
        )
        .unwrap()
        .data
        .address;

    // Instantiate Maker
    println!("Instantiating maker ...");
    let maker = wasm
        .instantiate(
            code_ids["astroport_maker"],
            &MakerInstantiateMsg {
                astro_token_contract: astro_token.clone(),
                factory_contract: factory.clone(),
                governance_contract: None,
                governance_percent: None,
                max_spread: None,
                owner: admin.address(),
                staking_contract: staking.clone(),
            },
            Some(&admin.address()),  // contract admin used for migration
            Some("Astroport Maker"), // contract label
            &[],                     // funds
            admin,                   // signer
        )
        .unwrap()
        .data
        .address;

    // Register vesting for astro generator
    println!("Registering vesting for astro generator ...");
    let vesting_amount = Uint128::from(63072000000000u128);
    let msg = Cw20ExecuteMsg::Send {
        contract: vesting.clone(),
        amount: vesting_amount,
        msg: to_binary(&VestingHookMsg::RegisterVestingAccounts {
            vesting_accounts: vec![VestingAccount {
                address: generator.clone(),
                schedules: vec![VestingSchedule {
                    start_point: VestingSchedulePoint {
                        amount: vesting_amount,
                        time: 1664582400u64, //2022-10-01T00:00:00Z
                    },
                    end_point: None,
                }],
            }],
        })
        .unwrap(),
    };
    let _res = wasm.execute(&astro_token, &msg, &[], admin).unwrap();

    AstroportContracts {
        factory: Contract::new(factory, code_ids["astroport_factory"]),
        generator: Contract::new(generator, code_ids["astroport_generator"]),
        staking: Contract::new(staking, code_ids["astroport_staking"]),
        router: Contract::new(router, code_ids["astroport_router"]),
        maker: Contract::new(maker, code_ids["astroport_maker"]),
        vesting: Contract::new(vesting, code_ids["astroport_vesting"]),
        astro_token: Contract::new(astro_token, code_ids["astro_token"]),
        pair_stable: Contract::new(String::from(""), code_ids["astroport_pair_stable"]),
        pair_xyk: Contract::new(String::from(""), code_ids["astroport_pair_xyk"]),
        whitelist: Contract::new(String::from(""), code_ids["astroport_whitelist"]),
    }
}

pub fn create_astroport_pair<'a, R>(
    app: &'a R,
    factory_addr: &str,
    pair_type: PairType,
    asset_infos: [AssetInfo; 2],
    init_params: Option<Binary>,
    signer: &SigningAccount,
) -> (String, String)
where
    R: Runner<'a>,
{
    let wasm = Wasm::new(app);

    let msg = AstroportFactoryExecuteMsg::CreatePair {
        pair_type,
        asset_infos,
        init_params,
    };
    let res = wasm.execute(factory_addr, &msg, &[], signer).unwrap();
    // Get pair and lp_token addresses from event
    parse_astroport_create_pair_events(&res.events)
}

pub fn parse_astroport_create_pair_events(events: &[Event]) -> (String, String) {
    let mut pair_addr = String::from("");
    let mut lp_token = String::from("");
    for event in events {
        if event.ty == "wasm" {
            let attributes = &event.attributes;
            for attr in attributes {
                if attr.key == "pair_contract_addr" {
                    pair_addr = attr.value.clone();
                }
                if attr.key == "liquidity_token_addr" {
                    lp_token = attr.value.clone();
                }
            }
        }
    }
    (pair_addr, lp_token)
}
#[cfg(test)]
mod tests {
    use astroport_types::{
        asset::{Asset, AssetInfo},
        factory::PairType,
    };
    use cosmrs::proto::cosmos::bank::v1beta1::QueryAllBalancesRequest;
    use cosmwasm_std::{Addr, Coin, Decimal, Uint128};
    use cw20::{AllowanceResponse, BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};
    use osmosis_test_tube::{Account, Bank, OsmosisTestApp, Wasm};
    use test_tube::Module;

    use crate::{
        artifact::Artifact,
        astroport::utils::{create_astroport_pair, setup_astroport},
        config::TestConfig,
    };
    use astroport_types::pair::ExecuteMsg as PairExecuteMsg;
    use std::{collections::HashMap, str::FromStr};

    #[cfg(feature = "rpc-runner")]
    use {
        crate::rpc_runner::{config::RpcRunnerConfig, RpcRunner},
        testcontainers::clients::Cli,
    };

    #[cfg(feature = "rpc-runner")]
    pub const TEST_CONFIG_PATH: &str = "configs/terra.yaml";

    pub const ARTIFACTS: [(&str, &str); 10] = [
        ("astroport_factory", "artifacts/astroport_factory.wasm"),
        ("astroport_generator", "artifacts/astroport_generator.wasm"),
        ("astroport_staking", "artifacts/astroport_staking.wasm"),
        ("astroport_router", "artifacts/astroport_router.wasm"),
        ("astroport_maker", "artifacts/astroport_maker.wasm"),
        ("astroport_vesting", "artifacts/astroport_vesting.wasm"),
        ("astro_token", "artifacts/astro_token.wasm"),
        (
            "astroport_pair_stable",
            "artifacts/astroport_pair_stable.wasm",
        ),
        ("astroport_pair_xyk", "artifacts/astroport_pair_xyk.wasm"),
        ("astroport_whitelist", "artifacts/astroport_whitelist.wasm"),
    ];

    #[test]
    pub fn test_instantiate_astroport_with_osmosis_test_app() {
        #[cfg(feature = "rpc-runner")]
        let rpc_runner_config = RpcRunnerConfig::from_yaml(TEST_CONFIG_PATH);
        let test_config = TestConfig {
            artifacts: ARTIFACTS
                .iter()
                .map(|(name, path)| (name.to_string(), Artifact::Local(path.to_string())))
                .collect::<HashMap<String, Artifact>>(),
            #[cfg(feature = "rpc-runner")]
            rpc_runner_config,
        };

        let app = OsmosisTestApp::new();
        let accs = app
            .init_accounts(&[Coin::new(100000000000000000u128, "uosmo")], 10)
            .unwrap();
        let wasm = Wasm::new(&app);

        let admin = &accs[0];

        // Print balances of admin
        let bank = Bank::new(&app);
        let balances = bank
            .query_all_balances(&QueryAllBalancesRequest {
                address: admin.address().to_string(),
                pagination: None,
            })
            .unwrap()
            .balances;
        println!("Balances of admin: {:?}", balances);

        // Instantiate contracts
        let contracts = setup_astroport(&app, &test_config, admin);

        // Create XYK pool
        let asset_infos: [AssetInfo; 2] = [
            AssetInfo::NativeToken {
                denom: "uosmo".into(),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked(&contracts.astro_token.address),
            },
        ];
        let (uluna_astro_pair_addr, uluna_astro_lp_token) = create_astroport_pair(
            &app,
            &contracts.factory.address,
            PairType::Xyk {},
            asset_infos.clone(),
            None,
            admin,
        );

        // Increase allowance of astro token
        let increase_allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
            spender: uluna_astro_pair_addr.clone(),
            amount: Uint128::from(1000000000u128),
            expires: None,
        };
        let _res = wasm
            .execute(
                &contracts.astro_token.address,
                &increase_allowance_msg,
                &vec![],
                admin,
            )
            .unwrap();

        // Query allowance
        let allowance_res: AllowanceResponse = wasm
            .query(
                &contracts.astro_token.address,
                &Cw20QueryMsg::Allowance {
                    owner: admin.address().to_string(),
                    spender: uluna_astro_pair_addr.clone(),
                },
            )
            .unwrap();
        assert_eq!(allowance_res.allowance, Uint128::from(1000000000u128));

        // Provide liquidity to XYK pool
        let provide_liq_msg = PairExecuteMsg::ProvideLiquidity {
            assets: [
                Asset {
                    amount: Uint128::from(420000000u128),
                    info: AssetInfo::NativeToken {
                        denom: "uosmo".into(),
                    },
                },
                Asset {
                    amount: Uint128::from(690000000u128),
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked(&contracts.astro_token.address),
                    },
                },
            ],
            slippage_tolerance: Some(Decimal::from_str("0.02").unwrap()),
            auto_stake: Some(false),
            receiver: None,
        };
        let _res = wasm.execute(
            &uluna_astro_pair_addr,
            &provide_liq_msg,
            &vec![Coin {
                amount: Uint128::from(420000000u128),
                denom: "uosmo".into(),
            }],
            admin,
        );

        // Query LP token balance
        let lp_token_balance: BalanceResponse = wasm
            .query(
                &uluna_astro_lp_token.to_string(),
                &Cw20QueryMsg::Balance {
                    address: admin.address().to_string(),
                },
            )
            .unwrap();
        println!("LP token balance: {:?}", lp_token_balance);
        assert!(lp_token_balance.balance > Uint128::zero());
    }

    #[cfg(feature = "rpc-runner")]
    #[test]
    pub fn test_instantiate_astroport_with_localterra() {
        let docker: Cli = Cli::default();
        let rpc_runner_config = RpcRunnerConfig::from_yaml(TEST_CONFIG_PATH);
        let test_config = TestConfig {
            artifacts: ARTIFACTS
                .iter()
                .map(|(name, path)| (name.to_string(), Artifact::Local(path.to_string())))
                .collect::<HashMap<String, Artifact>>(),
            rpc_runner_config,
        };
        let app = RpcRunner::new(test_config.clone(), &docker).unwrap();
        let accs = app
            .test_config
            .rpc_runner_config
            .import_all_accounts()
            .into_values()
            .collect::<Vec<_>>();
        let wasm = Wasm::new(&app);

        let admin = &accs[0];

        // Print balances of admin
        let bank = Bank::new(&app);
        let balances = bank
            .query_all_balances(&QueryAllBalancesRequest {
                address: admin.address().to_string(),
                pagination: None,
            })
            .unwrap()
            .balances;
        println!("Balances of admin: {:?}", balances);

        // Instantiate contracts
        let contracts = setup_astroport(&app, &test_config, admin);

        // Create XYK pool
        let asset_infos: [AssetInfo; 2] = [
            AssetInfo::NativeToken {
                denom: "uluna".into(),
            },
            AssetInfo::Token {
                contract_addr: Addr::unchecked(&contracts.astro_token.address),
            },
        ];
        let (uluna_astro_pair_addr, uluna_astro_lp_token) = create_astroport_pair(
            &app,
            &contracts.factory.address,
            PairType::Xyk {},
            asset_infos.clone(),
            None,
            admin,
        );

        // Increase allowance of astro token
        let increase_allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
            spender: uluna_astro_pair_addr.clone(),
            amount: Uint128::from(1000000000u128),
            expires: None,
        };
        let _res = wasm
            .execute(
                &contracts.astro_token.address,
                &increase_allowance_msg,
                &vec![],
                admin,
            )
            .unwrap();

        // Query allowance
        let allowance_res: AllowanceResponse = wasm
            .query(
                &contracts.astro_token.address,
                &Cw20QueryMsg::Allowance {
                    owner: admin.address().to_string(),
                    spender: uluna_astro_pair_addr.clone(),
                },
            )
            .unwrap();
        assert_eq!(allowance_res.allowance, Uint128::from(1000000000u128));

        // Provide liquidity to XYK pool
        let provide_liq_msg = PairExecuteMsg::ProvideLiquidity {
            assets: [
                Asset {
                    amount: Uint128::from(420000000u128),
                    info: AssetInfo::NativeToken {
                        denom: "uluna".into(),
                    },
                },
                Asset {
                    amount: Uint128::from(690000000u128),
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked(&contracts.astro_token.address),
                    },
                },
            ],
            slippage_tolerance: Some(Decimal::from_str("0.02").unwrap()),
            auto_stake: Some(false),
            receiver: None,
        };
        let _res = wasm.execute(
            &uluna_astro_pair_addr,
            &provide_liq_msg,
            &vec![Coin {
                amount: Uint128::from(420000000u128),
                denom: "uluna".into(),
            }],
            admin,
        );

        // Query LP token balance
        let lp_token_balance: BalanceResponse = wasm
            .query(
                &uluna_astro_lp_token.to_string(),
                &Cw20QueryMsg::Balance {
                    address: admin.address().to_string(),
                },
            )
            .unwrap();
        println!("LP token balance: {:?}", lp_token_balance);
        assert!(lp_token_balance.balance > Uint128::zero());
    }
}
