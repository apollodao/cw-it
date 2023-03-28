use crate::config::TestConfig;
use crate::helpers::upload_wasm_files;
use ap_native_coin_registry::InstantiateMsg as CoinRegistryInstantiateMsg;
use astroport::asset::{Asset, AssetInfo};
use astroport::factory::{
    ExecuteMsg as AstroportFactoryExecuteMsg, InstantiateMsg as AstroportFactoryInstantiateMsg,
    PairConfig, PairType,
};
use astroport::generator::InstantiateMsg as GeneratorInstantiateMsg;
use astroport::maker::InstantiateMsg as MakerInstantiateMsg;
use std::collections::HashMap;

use astroport::router::InstantiateMsg as RouterInstantiateMsg;
use astroport::staking::InstantiateMsg as StakingInstantiateMsg;
use astroport::token::InstantiateMsg as AstroTokenInstantiateMsg;
use astroport::vesting::{
    Cw20HookMsg as VestingHookMsg, InstantiateMsg as VestingInstantiateMsg, VestingAccount,
    VestingSchedule, VestingSchedulePoint,
};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{to_binary, Addr, Binary, Coin, Event, Uint128, Uint64};
use cw20::{BalanceResponse, Cw20Coin, Cw20ExecuteMsg, Cw20QueryMsg, MinterResponse};
use osmosis_test_tube::{Account, Module, Runner, SigningAccount, Wasm};

pub const ASTROPORT_CONTRACT_NAMES: [&str; 11] = [
    "astroport_token",
    "astroport_native_coin_registry",
    "astroport_factory",
    "astroport_generator",
    "astroport_maker",
    "astroport_pair_stable",
    "astroport_pair",
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
    pub coin_registry: Contract,
    pub generator: Contract,
    pub astro_token: Contract,
    pub maker: Contract,
    pub pair_stable: Contract,
    pub pair: Contract,
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
            code_ids["astroport_token"],
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
                marketing: None,
            },
            Some(&admin.address()),
            Some("Astro Token"),
            &[],
            admin,
        )
        .unwrap()
        .data
        .address;

    // Instantiate coin registry
    println!("Instantiating coin registry ...");
    let coin_registry = wasm
        .instantiate(
            code_ids["astroport_native_coin_registry"],
            &CoinRegistryInstantiateMsg {
                owner: admin.address(),
            },
            Some(&admin.address()),
            Some("Coin Registry"),
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
                        code_id: code_ids["astroport_pair"],
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
                token_code_id: code_ids["astroport_token"], // TODO: is this correct or do we need another contract?
                fee_address: None,
                generator_address: None, // TODO: Set this
                owner: admin.address(),
                whitelist_code_id: code_ids["astroport_whitelist"],
                coin_registry_address: coin_registry.clone(),
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
                vesting_token: AssetInfo::Token {
                    contract_addr: Addr::unchecked(&astro_token),
                },
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
                voting_escrow: None,
                guardian: None,
                astro_token: AssetInfo::Token {
                    contract_addr: Addr::unchecked(&astro_token),
                },
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
                coin_registry_address: None,
            },
            &[],
            admin,
        )
        .unwrap();

    // Instantiate staking
    println!("Instantiating staking ...");
    let staking_code_id = code_ids.get("astroport_staking");
    let staking = staking_code_id.map(|code_id| {
        wasm.instantiate(
            *code_id,
            &StakingInstantiateMsg {
                owner: admin.address(),
                deposit_token_addr: astro_token.clone(),
                token_code_id: code_ids["astroport_token"],
                marketing: None,
            },
            Some(&admin.address()),    // contract admin used for migration
            Some("Astroport Staking"), // contract label
            &[],                       // funds
            admin,                     // signer
        )
        .unwrap()
        .data
        .address
    });

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
                factory_contract: factory.clone(),
                governance_contract: None,
                governance_percent: None,
                max_spread: None,
                owner: admin.address(),
                staking_contract: staking.clone(),
                astro_token: AssetInfo::Token {
                    contract_addr: Addr::unchecked(&astro_token),
                },
                // TODO: Uncertain about this
                default_bridge: Some(AssetInfo::NativeToken {
                    denom: "uosmo".to_string(),
                }),
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
        staking: Contract::new(staking.unwrap_or_default(), code_ids["astroport_staking"]),
        router: Contract::new(router, code_ids["astroport_router"]),
        maker: Contract::new(maker, code_ids["astroport_maker"]),
        vesting: Contract::new(vesting, code_ids["astroport_vesting"]),
        astro_token: Contract::new(astro_token, code_ids["astroport_token"]),
        coin_registry: Contract::new(coin_registry, code_ids["astroport_native_coin_registry"]),
        pair_stable: Contract::new(String::from(""), code_ids["astroport_pair_stable"]),
        pair: Contract::new(String::from(""), code_ids["astroport_pair"]),
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
    initial_liquidity: Option<[Uint128; 2]>,
) -> (String, String)
where
    R: Runner<'a>,
{
    let wasm = Wasm::new(app);

    let msg = AstroportFactoryExecuteMsg::CreatePair {
        pair_type,
        asset_infos: asset_infos.to_vec(),
        init_params,
    };
    let res = wasm.execute(factory_addr, &msg, &[], signer).unwrap();

    // Get pair and lp_token addresses from event
    let (pair_addr, lp_token_addr) = parse_astroport_create_pair_events(&res.events);

    if let Some(initial_liquidity) = initial_liquidity {
        let assets = asset_infos
            .into_iter()
            .zip(initial_liquidity.into_iter())
            .map(|(info, amount)| Asset { info, amount })
            .collect();
        provide_liquidity(app, &pair_addr, assets, signer);
    }

    (pair_addr, lp_token_addr)
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

pub fn get_lp_token_balance<'a, R>(wasm: &Wasm<'a, R>, pair_addr: &str, address: &str) -> Uint128
where
    R: Runner<'a>,
{
    // Get lp token address
    let msg = astroport::pair::QueryMsg::Pair {};
    let lp_token_addr = wasm
        .query::<_, astroport::asset::PairInfo>(pair_addr, &msg)
        .unwrap()
        .liquidity_token;

    let msg = Cw20QueryMsg::Balance {
        address: address.to_string(),
    };
    let res: BalanceResponse = wasm.query(&lp_token_addr.to_string(), &msg).unwrap();
    res.balance
}

pub fn coin_to_astro_asset(coin: &Coin) -> Asset {
    Asset {
        info: AssetInfo::NativeToken {
            denom: coin.denom.clone(),
        },
        amount: coin.amount,
    }
}

pub fn provide_liquidity<'a, R>(
    app: &'a R,
    pair_addr: &str,
    assets: Vec<Asset>,
    signer: &SigningAccount,
) -> Uint128
where
    R: Runner<'a>,
{
    let wasm = Wasm::new(app);

    // Get lp token balance before providing liquidity
    let lp_token_balance_before = get_lp_token_balance(&wasm, pair_addr, &signer.address());

    // Increase allowance for cw20 tokens and add coins to funds
    let mut funds = vec![];
    for asset in &assets {
        match &asset.info {
            AssetInfo::Token { contract_addr } => {
                let msg = Cw20ExecuteMsg::IncreaseAllowance {
                    spender: pair_addr.to_string(),
                    amount: asset.amount,
                    expires: None,
                };
                wasm.execute(&contract_addr.to_string(), &msg, &[], signer)
                    .unwrap();
            }
            AssetInfo::NativeToken { denom } => {
                funds.push(Coin {
                    denom: denom.to_string(),
                    amount: asset.amount,
                });
            }
        }
    }

    funds.sort_by(|a, b| a.denom.cmp(&b.denom));
    println!("funds: {:?}", funds);

    // Provide liquidity
    let msg = astroport::pair::ExecuteMsg::ProvideLiquidity {
        assets: assets,
        slippage_tolerance: None,
        receiver: None,
        auto_stake: Some(false),
    };
    wasm.execute(pair_addr, &msg, &funds, signer).unwrap();

    // Get lp token balance after providing liquidity
    let lp_token_balance_after = get_lp_token_balance(&wasm, pair_addr, &signer.address());

    // Return lp token balance difference
    lp_token_balance_after - lp_token_balance_before
}

#[cfg(test)]
mod tests {
    use astroport::{
        asset::{Asset, AssetInfo},
        factory::PairType,
        pair::PoolResponse,
    };
    use cosmrs::proto::cosmos::bank::v1beta1::QueryAllBalancesRequest;
    use cosmwasm_std::{Addr, Coin, Decimal, Uint128};
    use cw20::{AllowanceResponse, BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};
    use osmosis_test_tube::{Account, Bank, OsmosisTestApp, SigningAccount, Wasm};
    use test_case::test_case;
    use test_tube::Module;

    use crate::{
        artifact::Artifact,
        astroport::utils::{create_astroport_pair, setup_astroport, ASTROPORT_CONTRACT_NAMES},
        config::TestConfig,
        test_runner::TestRunner,
    };
    use astroport::pair::ExecuteMsg as PairExecuteMsg;
    use std::{collections::HashMap, str::FromStr};

    #[cfg(feature = "rpc-runner")]
    use {
        crate::rpc_runner::{config::RpcRunnerConfig, RpcRunner},
        testcontainers::clients::Cli,
    };

    #[cfg(feature = "chain-download")]
    use crate::artifact::ChainArtifact;

    #[cfg(feature = "rpc-runner")]
    pub const TEST_CONFIG_PATH: &str = "configs/terra.yaml";

    /// Whether or not your used cw-optimizoor to compile artifacts
    /// (adds cpu architecture to wasm file name).
    pub const USE_CW_OPTIMIZOOR: bool = true;

    /// The commit hash from where the contracts were compiled. If set, the artifacts should be in
    /// a subfolder with this name.
    pub const COMMIT: Option<&str> = Some("042b076");

    /// Get the wasm path for the contract depending on above consts
    fn get_wasm_path(name: &str) -> String {
        // If using cw-optimizoor, it prepends the cpu architecture to the wasm file name
        let name = if USE_CW_OPTIMIZOOR {
            format!("{}-{}.wasm", name, std::env::consts::ARCH)
        } else {
            format!("{}.wasm", name)
        };
        // If commit is set, use the relevant folder
        let folder = format!(
            "{}/{}",
            std::env::var("ARTIFACTS_DIR").unwrap_or_else(|_| "artifacts".to_string()),
            COMMIT.unwrap_or("")
        );
        format!("{}/{}", folder, name)
    }

    /// Get artifacts already on the disk
    fn get_local_artifacts() -> HashMap<String, Artifact> {
        ASTROPORT_CONTRACT_NAMES
            .into_iter()
            .map(|name| (name.to_string(), Artifact::Local(get_wasm_path(name))))
            .collect::<HashMap<String, Artifact>>()
    }

    #[cfg(feature = "chain-download")]
    /// The Neutron testnet RPC to use to download wasm files
    pub const NEUTRON_RPC: &str = "https://rpc.baryon.ntrn.info/";

    #[cfg(feature = "chain-download")]
    // The Neutron testnet contract addresses to use to download wasm files
    pub const NEUTRON_CONTRACT_ADDRESSES: &[(&str, ChainArtifact)] = &[
        ("astroport_token", ChainArtifact::CodeId(62)),
        ("astroport_pair_stable", ChainArtifact::CodeId(64)),
        ("astroport_pair", ChainArtifact::CodeId(63)),
        ("astroport_whitelist", ChainArtifact::CodeId(65)),
        (
            "astroport_native_coin_registry",
            ChainArtifact::Addr(
                "neutron1rfxpyypcseumuyxmln43d7lc9h0kjw87xc433x38s7w22ukmw8vqd3k35c",
            ),
        ),
        (
            "astroport_factory",
            ChainArtifact::Addr(
                "neutron1fuaym3wkqvts8r9vafd77q00jxuplacchde552amyk05gjqtmy2s84lnvr",
            ),
        ),
        (
            "astroport_generator",
            ChainArtifact::Addr(
                "neutron1mum2jzk55uhl375cmpydla9lsen65fvmcz2sm6k92n9uc8mm8r5sev5pen",
            ),
        ),
        (
            "astroport_maker",
            ChainArtifact::Addr(
                "neutron1t9u4yesvzlprm37zlaujppfl9u3fpkv5jze77rh8tj38rww3dneqvruq8j",
            ),
        ),
        (
            "astroport_router",
            ChainArtifact::Addr(
                "neutron13umcxfjs2jufxsjrheggf6zy9tx7jclm9uemkkre64unrwuzzs9sc355f3",
            ),
        ),
        (
            "astroport_vesting",
            ChainArtifact::Addr(
                "neutron1u430x73aack5zz0gx99zmu83yfjfe9wjf0vfguz8q9fdl04cspjs6ftcta",
            ),
        ),
        (
            "astroport_satellite",
            ChainArtifact::Addr(
                "neutron1zuskfye2n07q6ylnrhkrvuha5y886q4m2m44nam5ljsrzrl63q6q07q4r7",
            ),
        ),
    ];

    #[cfg(feature = "chain-download")]
    /// Get articacts from Neutron testnet
    fn get_neutron_testnet_artifacts() -> HashMap<String, Artifact> {
        let mut artifacts = NEUTRON_CONTRACT_ADDRESSES
            .into_iter()
            .map(|(name, chain_artifact)| {
                (
                    name.to_string(),
                    chain_artifact.into_artifact(NEUTRON_RPC.to_string()),
                )
            })
            .collect::<HashMap<String, Artifact>>();
        // Staking contract not deployed on Neutron testnet
        artifacts.insert(
            "astroport_staking".to_string(),
            Artifact::Local(get_wasm_path("astroport_staking")),
        );
        artifacts
    }

    /// Creates an Osmosis test runner and accounts.
    fn get_osmosis_test_app<'a>() -> (TestRunner<'a>, Vec<SigningAccount>, &'a str) {
        let app = OsmosisTestApp::new();
        let accs = app
            .init_accounts(&[Coin::new(100000000000000000u128, "uosmo")], 10)
            .unwrap();
        (TestRunner::OsmosisTestApp(app), accs, "uosmo")
    }

    /// Creates an RPC test runner and accounts. If `cli` is Some, it will attempt to run the tests
    /// against the configured docker container.
    #[cfg(feature = "rpc-runner")]
    fn get_rpc_runner<'a>(cli: Option<&'a Cli>) -> (TestRunner<'a>, Vec<SigningAccount>, &'a str) {
        let rpc_runner_config = RpcRunnerConfig::from_yaml(TEST_CONFIG_PATH);
        let test_config = TestConfig {
            artifacts: ASTROPORT_CONTRACT_NAMES
                .iter()
                .map(|name| (name.to_string(), Artifact::Local(get_wasm_path(name))))
                .collect::<HashMap<String, Artifact>>(),
            rpc_runner_config: rpc_runner_config.clone(),
        };

        let runner = if let Some(cli) = cli {
            RpcRunner::new(test_config, Some(cli)).unwrap()
        } else {
            RpcRunner::new(test_config, None).unwrap()
        };

        let accs = runner
            .test_config
            .rpc_runner_config
            .import_all_accounts()
            .into_values()
            .collect::<Vec<_>>();
        (TestRunner::RpcRunner(runner), accs, "uluna") //TODO: Add native token to config
    }

    #[cfg(feature = "rpc-runner")]
    #[test_case(get_local_artifacts => (); "local artifacts, rpc runner")]
    pub fn test_with_rpc_runner<'a>(get_artifacts: impl Fn() -> HashMap<String, Artifact>) {
        let cli = Cli::default();
        test_instantiate_astroport(get_rpc_runner(Some(&cli)), get_artifacts);
    }

    #[cfg(feature = "chain-download")]
    #[test_case(get_osmosis_test_app() => (); "Neutron testnet artifacts, osmosis test app")]
    pub fn test_with_neutron_testnet_artifacts<'a>(
        (app, accs, native_denom): (TestRunner<'a>, Vec<SigningAccount>, &'a str),
    ) {
        test_instantiate_astroport((app, accs, native_denom), get_neutron_testnet_artifacts);
    }

    #[test_case(get_osmosis_test_app(),get_local_artifacts => (); "local artifacts, osmosis test app")]
    pub fn test_instantiate_astroport<'a>(
        (app, accs, native_denom): (TestRunner<'a>, Vec<SigningAccount>, &'a str),
        get_artifacts: impl Fn() -> HashMap<String, Artifact>,
    ) {
        #[cfg(feature = "rpc-runner")]
        let rpc_runner_config = RpcRunnerConfig::from_yaml(TEST_CONFIG_PATH);
        let test_config = TestConfig {
            artifacts: get_artifacts(),
            #[cfg(feature = "rpc-runner")]
            rpc_runner_config,
        };

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
                denom: native_denom.into(),
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
            None,
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
            assets: vec![
                Asset {
                    amount: Uint128::from(420000000u128),
                    info: AssetInfo::NativeToken {
                        denom: native_denom.into(),
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
                denom: native_denom.into(),
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

    #[test]
    fn test_create_astroport_pair() {
        let app = OsmosisTestApp::new();
        let admin = app
            .init_account(&[Coin::new(100000000000000000u128, "uosmo")])
            .unwrap();

        let test_config = TestConfig {
            artifacts: get_local_artifacts(),
            #[cfg(feature = "rpc-runner")]
            rpc_runner_config: RpcRunnerConfig::from_yaml(TEST_CONFIG_PATH),
        };

        // Instantiate contracts
        let contracts = setup_astroport(&app, &test_config, &admin);

        // Create XYK pool

        let asset_infos: [AssetInfo; 2] = [
            AssetInfo::NativeToken {
                denom: "uosmo".into(),
            },
            AssetInfo::NativeToken {
                denom: "uatom".into(),
            },
        ];

        create_astroport_pair(
            &app,
            &contracts.factory.address,
            PairType::Xyk {},
            asset_infos.clone(),
            None,
            &admin,
            None,
        );
    }

    #[test]
    fn test_create_astroport_pair_with_initial_liquidity() {
        let app = OsmosisTestApp::new();
        let admin = app
            .init_account(&[
                Coin::new(100000000000000000u128, "uosmo"),
                Coin::new(100000000000000000u128, "uatom"),
            ])
            .unwrap();

        let test_config = TestConfig {
            artifacts: get_local_artifacts(),
            #[cfg(feature = "rpc-runner")]
            rpc_runner_config: RpcRunnerConfig::from_yaml(TEST_CONFIG_PATH),
        };

        // Instantiate contracts
        let contracts = setup_astroport(&app, &test_config, &admin);

        // Create XYK pool
        let asset_infos: [AssetInfo; 2] = [
            AssetInfo::NativeToken {
                denom: "uosmo".into(),
            },
            AssetInfo::NativeToken {
                denom: "uatom".into(),
            },
        ];

        let (pool, lp) = create_astroport_pair(
            &app,
            &contracts.factory.address,
            PairType::Xyk {},
            asset_infos.clone(),
            None,
            &admin,
            Some([1000000u128.into(), 1000000u128.into()]),
        );

        // Query pool info
        let wasm = Wasm::new(&app);
        let pool_info: PoolResponse = wasm
            .query(&pool, &astroport::pair::QueryMsg::Pool {})
            .unwrap();
        assert_eq!(pool_info.assets[0].amount, Uint128::from(1000000u128));
        assert_eq!(pool_info.assets[1].amount, Uint128::from(1000000u128));
    }
}
