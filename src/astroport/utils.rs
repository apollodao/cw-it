use crate::artifact::Artifact;
use crate::helpers::upload_wasm_files;
use crate::traits::CwItRunner;
use crate::{ContractMap, ContractType, TestRunner};
use astroport::asset::{Asset, AssetInfo};
use astroport::factory::{
    ExecuteMsg as AstroportFactoryExecuteMsg, InstantiateMsg as AstroportFactoryInstantiateMsg,
    PairConfig, PairType,
};
use astroport::incentives::InstantiateMsg as IncentivesInstantiateMsg;
use astroport_v2::asset::AssetInfo as AssetInfoV2;
use astroport_v2::liquidity_manager::InstantiateMsg as LiquidityManagerInstantiateMsg;
use astroport_v2::maker::InstantiateMsg as MakerInstantiateMsg;
use astroport_v2::native_coin_registry::InstantiateMsg as CoinRegistryInstantiateMsg;
use astroport_v2::router::InstantiateMsg as RouterInstantiateMsg;
use astroport_v2::token::InstantiateMsg as AstroTokenInstantiateMsg;
use astroport_v2::vesting::{
    Cw20HookMsg as VestingHookMsg, InstantiateMsg as VestingInstantiateMsg, VestingAccount,
    VestingSchedule, VestingSchedulePoint,
};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{to_json_binary, Addr, Binary, Coin, Event, Uint128, Uint64};
use cw20::{BalanceResponse, Cw20Coin, Cw20ExecuteMsg, Cw20QueryMsg, MinterResponse};
use osmosis_std::types::cosmos::bank::v1beta1::QueryBalanceRequest;
use osmosis_std::types::cosmos::base::v1beta1::Coin as OsmosisCoin;
use osmosis_std::types::osmosis::tokenfactory::v1beta1::{
    MsgCreateDenom, MsgCreateDenomResponse, MsgMint, MsgMintResponse,
};
use std::collections::HashMap;
use test_tube::ExecuteResponse;
use test_tube::{Account, Bank, Module, Runner, SigningAccount, Wasm};

pub const ASTROPORT_CONTRACT_NAMES: [&str; 12] = [
    "astroport_token",
    "astroport_native_coin_registry",
    "astroport_factory",
    "astroport_maker",
    "astroport_pair_stable",
    "astroport_pair",
    "astroport_router",
    "astroport_vesting",
    "astroport_pair_concentrated",
    "astroport_incentives",
    "astroport_tokenfactory_tracker",
    "astroport_liquidity_manager",
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
    pub astro_token: Contract,
    pub maker: Contract,
    pub pair_stable: Contract,
    pub pair: Contract,
    pub router: Contract,
    pub vesting: Contract,
    pub incentives: Contract,
    pub liquidity_manager: Contract,
}

impl AstroportContracts {
    pub fn new_from_local_contracts(
        runner: &TestRunner,
        path: &Option<&str>,
        append_arch: bool,
        arch: &Option<&str>,
        signer: &SigningAccount,
    ) -> Self {
        let contracts = get_local_contracts(runner, path, append_arch, arch);

        setup_astroport(runner, contracts, signer)
    }
}

pub fn setup_astroport<'a, R: CwItRunner<'a>>(
    app: &'a R,
    contracts: ContractMap,
    admin: &SigningAccount,
) -> AstroportContracts {
    // Upload contracts
    let code_ids = upload_wasm_files(app, admin, contracts).unwrap();

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
                        permissioned: false,
                    },
                    PairConfig {
                        code_id: code_ids["astroport_pair_stable"],
                        is_disabled: false,
                        is_generator_disabled: false,
                        maker_fee_bps: 5000,
                        total_fee_bps: 5,
                        pair_type: PairType::Stable {},
                        permissioned: false,
                    },
                    PairConfig {
                        code_id: code_ids["astroport_pair_concentrated"],
                        is_disabled: false,
                        is_generator_disabled: false,
                        maker_fee_bps: 5000,
                        total_fee_bps: 0,
                        pair_type: PairType::Custom("concentrated".to_string()),
                        permissioned: false,
                    },
                ],
                token_code_id: code_ids["astroport_token"], // TODO: is this correct or do we need another contract?
                fee_address: None,
                generator_address: None, // TODO: Set this
                owner: admin.address(),
                // whitelist_code_id: code_ids["astroport_whitelist"],
                whitelist_code_id: 0,
                coin_registry_address: coin_registry.clone(),
                tracker_config: None,
            },
            Some(&admin.address()),    // contract admin used for migration
            Some("Astroport Factory"), // contract label
            &[],                       // funds
            admin,                     // signer
        )
        .unwrap()
        .data
        .address;

    // Instantiate Liquidity Manager
    println!("Instantiating liquidity manager ...");
    let liquidity_manager = wasm
        .instantiate(
            code_ids["astroport_liquidity_manager"],
            &LiquidityManagerInstantiateMsg {
                astroport_factory: factory.clone(),
            },
            Some(&admin.address()),
            Some("Liquidity Manager"),
            &[],
            admin,
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
                vesting_token: AssetInfoV2::Token {
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

    println!("Instantiating incentives ...");
    let incentives = wasm
        .instantiate(
            code_ids["astroport_incentives"],
            &IncentivesInstantiateMsg {
                owner: admin.address(),
                factory: factory.clone(),
                guardian: None,
                astro_token: AssetInfo::Token {
                    contract_addr: Addr::unchecked(&astro_token),
                },
                vesting_contract: vesting.clone(),
                incentivization_fee_info: None,
            },
            Some(&admin.address()),       // contract admin used for migration
            Some("Astroport Incentives"), // contract label
            &[],                          // funds
            admin,                        // signer
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
                generator_address: Some(incentives.clone()),
                fee_address: None,
                token_code_id: None,
                whitelist_code_id: None,
                coin_registry_address: None,
            },
            &[],
            admin,
        )
        .unwrap();

    let msg = MsgCreateDenom {
        sender: admin.address(),
        subdenom: "astro".to_string(),
    };

    let res: ExecuteResponse<MsgCreateDenomResponse> =
        app.execute(msg, MsgCreateDenom::TYPE_URL, admin).unwrap();

    let staking_denom = res.data.new_token_denom;
    let msg = MsgMint {
        sender: admin.address(),
        amount: Some(OsmosisCoin {
            denom: staking_denom.clone(),
            amount: 1e18.to_string(),
        }),
        mint_to_address: admin.address(),
    };
    let _res: ExecuteResponse<MsgMintResponse> =
        app.execute(msg, MsgMint::TYPE_URL, admin).unwrap();

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
                governance_contract: Some(admin.address()),
                governance_percent: Some(Uint64::new(100u64)),
                max_spread: None,
                owner: admin.address(),
                staking_contract: None,
                astro_token: AssetInfoV2::Token {
                    contract_addr: Addr::unchecked(&astro_token),
                },
                // TODO: Uncertain about this
                default_bridge: Some(AssetInfoV2::NativeToken {
                    denom: "uosmo".to_string(),
                }),
                second_receiver_params: None,
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
        msg: to_json_binary(&VestingHookMsg::RegisterVestingAccounts {
            vesting_accounts: vec![VestingAccount {
                address: incentives.clone(),
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
        router: Contract::new(router, code_ids["astroport_router"]),
        maker: Contract::new(maker, code_ids["astroport_maker"]),
        vesting: Contract::new(vesting, code_ids["astroport_vesting"]),
        astro_token: Contract::new(astro_token, code_ids["astroport_token"]),
        coin_registry: Contract::new(coin_registry, code_ids["astroport_native_coin_registry"]),
        pair_stable: Contract::new(String::from(""), code_ids["astroport_pair_stable"]),
        pair: Contract::new(String::from(""), code_ids["astroport_pair"]),
        incentives: Contract::new(incentives, code_ids["astroport_incentives"]),
        liquidity_manager: Contract::new(
            liquidity_manager,
            code_ids["astroport_liquidity_manager"],
        ),
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
    denom_creation_fee: &[Coin],
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
    let res = wasm
        .execute(factory_addr, &msg, denom_creation_fee, signer)
        .unwrap();

    // Get pair and lp_token addresses from event
    let (pair_addr, lp_token) = parse_astroport_create_pair_events(&res.events);

    if let Some(initial_liquidity) = initial_liquidity {
        let assets = asset_infos
            .into_iter()
            .zip(initial_liquidity)
            .map(|(info, amount)| Asset { info, amount })
            .collect();
        provide_liquidity(app, &pair_addr, assets, signer);
    }

    (pair_addr, lp_token)
}

pub fn parse_astroport_create_pair_events(events: &[Event]) -> (String, String) {
    let mut pair_addr = String::from("");
    let mut lp_token_addr = String::from("");
    let mut lp_token_denom = String::from("");

    for event in events {
        if event.ty == "wasm" {
            let attributes = &event.attributes;
            for attr in attributes {
                if attr.key == "pair_contract_addr" {
                    pair_addr.clone_from(&attr.value);
                }
                if attr.key == "liquidity_token_addr" {
                    lp_token_addr.clone_from(&attr.value);
                }
                if attr.key == "lp_denom" {
                    lp_token_denom.clone_from(&attr.value);
                }
            }
        }
    }

    let lp_token = if lp_token_denom.is_empty() {
        lp_token_addr
    } else {
        lp_token_denom
    };

    (pair_addr, lp_token)
}

pub fn get_lp_token_balance<'a, R>(
    runner: &'a R,
    pair_addr: &str,
    address: &SigningAccount,
) -> Uint128
where
    R: Runner<'a>,
{
    let wasm = Wasm::new(runner);
    let bank = Bank::new(runner);
    // Get lp token address
    let msg = astroport::pair::QueryMsg::Pair {};
    let lp_token = wasm
        .query::<_, astroport::asset::PairInfo>(pair_addr, &msg)
        .unwrap()
        .liquidity_token;

    let balance = if lp_token.starts_with(address.prefix()) {
        let msg = Cw20QueryMsg::Balance {
            address: address.address(),
        };
        let res: BalanceResponse = wasm.query(lp_token.as_ref(), &msg).unwrap();
        res.balance
    } else {
        bank.query_balance(&QueryBalanceRequest {
            address: address.address().clone(),
            denom: lp_token,
        })
        .unwrap()
        .balance
        .unwrap_or_default()
        .amount
        .parse()
        .unwrap()
    };

    balance
}

/// Converts a Coin to an Astroport Asset
pub fn coin_to_astro_asset(coin: &Coin) -> Asset {
    Asset {
        info: AssetInfo::NativeToken {
            denom: coin.denom.clone(),
        },
        amount: coin.amount,
    }
}

/// Helper to get a native token Astroport AssetInfo.
pub fn native_info(denom: &str) -> AssetInfo {
    AssetInfo::NativeToken {
        denom: denom.to_string(),
    }
}

/// Helper to get a cw20 token Astroport AssetInfo
pub fn cw20_info(contract_addr: &str) -> AssetInfo {
    AssetInfo::Token {
        contract_addr: Addr::unchecked(contract_addr),
    }
}

/// Helper to get a native token Astroport Asset
pub fn native_asset(denom: &str, amount: impl Into<Uint128>) -> Asset {
    Asset {
        info: native_info(denom),
        amount: amount.into(),
    }
}

/// Helper to get a cw20 token Astroport Asset
pub fn cw20_asset(contract_addr: &str, amount: impl Into<Uint128>) -> Asset {
    Asset {
        info: cw20_info(contract_addr),
        amount: amount.into(),
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
    let lp_token_balance_before = get_lp_token_balance(app, pair_addr, signer);

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
                wasm.execute(contract_addr.as_ref(), &msg, &[], signer)
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
        assets,
        slippage_tolerance: None,
        receiver: None,
        auto_stake: Some(false),
        min_lp_to_receive: None,
    };
    wasm.execute(pair_addr, &msg, &funds, signer).unwrap();

    // Get lp token balance after providing liquidity
    let lp_token_balance_after = get_lp_token_balance(app, pair_addr, signer);

    // Return lp token balance difference
    lp_token_balance_after - lp_token_balance_before
}

/// Get the wasm path for the contract with the given name.
///
/// # Arguments:
/// * `name` - The name of the contract
/// * `path` - The path to the artifacts folder
/// * `append_arch` - If true, the architecture will be appended to the filename
pub fn get_wasm_path(
    name: &str,
    path: &Option<&str>,
    append_arch: bool,
    arch: &Option<&str>,
) -> String {
    // If using cw-optimizoor, it prepends the cpu architecture to the wasm file name
    let name = if append_arch {
        let arch = arch.unwrap_or(std::env::consts::ARCH);
        format!("{}-{}.wasm", name, arch)
    } else {
        format!("{}.wasm", name)
    };

    format!("{}/{}", path.unwrap_or_else(|| "artifacts"), name)
}

/// Get astroport artifacts already from disk
pub fn get_local_artifacts(
    path: &Option<&str>,
    append_arch: bool,
    arch: &Option<&str>,
) -> HashMap<String, Artifact> {
    ASTROPORT_CONTRACT_NAMES
        .into_iter()
        .map(|name| {
            (
                name.to_string(),
                Artifact::Local(get_wasm_path(name, path, append_arch, arch)),
            )
        })
        .collect::<HashMap<String, Artifact>>()
}

/// Get astroport contracts as artifacts from disk or as imports for multi-test-runner
pub fn get_local_contracts(
    test_runner: &TestRunner,
    path: &Option<&str>,
    append_arch: bool,
    arch: &Option<&str>,
) -> ContractMap {
    match test_runner {
        #[cfg(feature = "astroport-multi-test")]
        TestRunner::MultiTest(_) => crate::astroport::utils::get_astroport_multitest_contracts(),
        _ => crate::astroport::utils::get_local_artifacts(path, append_arch, arch)
            .into_iter()
            .map(|(name, artifact)| (name, ContractType::Artifact(artifact)))
            .collect(),
    }
}

#[cfg(feature = "astroport-multi-test")]
pub fn get_astroport_multitest_contracts() -> HashMap<String, ContractType> {
    use apollo_cw_multi_test::ContractWrapper;
    use cosmwasm_std::Empty;

    use crate::create_contract_wrappers;
    use crate::create_contract_wrappers_with_reply;

    let mut contract_wrappers = create_contract_wrappers!(
        "astroport_native_coin_registry",
        "astroport_maker",
        "astroport_token",
        "astroport_router",
        "astroport_vesting" // "astroport_whitelist"
    );

    contract_wrappers.extend(create_contract_wrappers_with_reply!(
        "astroport_factory",
        "astroport_pair_stable",
        "astroport_pair"
    ));

    // Liquidity manager, incentives, and concentrated pair don't have query entrypoint in contract module
    contract_wrappers.extend(vec![
        (
            "astroport_liquidity_manager".to_string(),
            Box::new(
                ContractWrapper::new_with_empty(
                    astroport_liquidity_manager::contract::execute,
                    astroport_liquidity_manager::contract::instantiate,
                    astroport_liquidity_manager::query::query,
                )
                .with_reply(astroport_liquidity_manager::contract::reply),
            ) as Box<dyn apollo_cw_multi_test::Contract<Empty>>,
        ),
        (
            "astroport_pair_concentrated".to_string(),
            Box::new(
                ContractWrapper::new_with_empty(
                    astroport_pair_concentrated::contract::execute,
                    astroport_pair_concentrated::contract::instantiate,
                    astroport_pair_concentrated::queries::query,
                )
                .with_reply(astroport_pair_concentrated::contract::reply),
            ) as Box<dyn apollo_cw_multi_test::Contract<Empty>>,
        ),
        (
            "astroport_incentives".to_string(),
            Box::new(
                ContractWrapper::new_with_empty(
                    astroport_incentives::execute::execute,
                    astroport_incentives::instantiate::instantiate,
                    astroport_incentives::query::query,
                )
                .with_reply(astroport_incentives::reply::reply),
            ) as Box<dyn apollo_cw_multi_test::Contract<Empty>>,
        ),
    ]);

    contract_wrappers
        .into_iter()
        .map(|(k, v)| (k, ContractType::MultiTestContract(v)))
        .collect()
}

#[allow(dead_code)]
#[cfg(test)]
mod tests {
    use astroport::asset::{Asset, AssetInfo};
    use cosmwasm_std::{Addr, Coin, Decimal, Uint128};
    use cw20::{AllowanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};
    use osmosis_std::types::cosmos::bank::v1beta1::QueryAllBalancesRequest;
    use osmosis_std::types::cosmos::bank::v1beta1::QueryBalanceRequest;
    use test_tube::{Account, Bank, Module, Wasm};

    use crate::{
        astroport::utils::{create_astroport_pair, setup_astroport},
        test_runner::TestRunner,
        ContractMap,
    };
    use astroport::pair::ExecuteMsg as PairExecuteMsg;
    use std::str::FromStr;

    use crate::traits::CwItRunner;

    #[cfg(any(
        feature = "rpc-runner",
        feature = "chain-download",
        feature = "osmosis-test-tube"
    ))]
    use crate::OwnedTestRunner;

    #[cfg(feature = "rpc-runner")]
    use crate::rpc_runner::{config::RpcRunnerConfig, RpcRunner};

    #[cfg(feature = "chain-download")]
    use {
        super::get_wasm_path, crate::artifact::ChainArtifact, crate::ContractType,
        std::collections::HashMap,
    };

    #[cfg(feature = "rpc-runner")]
    pub const TEST_CONFIG_PATH: &str = "configs/terra.yaml";

    /// Which test runner to use for the tests
    pub const TEST_RUNNER: &str = "osmosis-test-app";

    /// cw-optimizoor adds the CPU architecture to the wasm file name
    pub const APPEND_ARCH: bool = false;
    pub const ARCH: Option<&str> = None;

    /// The path to the artifacts folder
    pub const ARTIFACTS_PATH: Option<&str> = Some("artifacts/4d3be0e");

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

    /// Get astroport artifacts already from disk
    pub fn get_local_contracts(test_runner: &TestRunner) -> ContractMap {
        super::get_local_contracts(test_runner, &ARTIFACTS_PATH, APPEND_ARCH, &ARCH)
    }

    #[cfg(feature = "chain-download")]
    /// Get artifacts from Neutron testnet
    fn get_neutron_testnet_artifacts() -> HashMap<String, ContractType> {
        let artifacts = NEUTRON_CONTRACT_ADDRESSES
            .iter()
            .map(|(name, chain_artifact)| {
                (
                    name.to_string(),
                    ContractType::Artifact(chain_artifact.into_artifact(NEUTRON_RPC.to_string())),
                )
            })
            .collect::<HashMap<String, ContractType>>();
        // Staking contract not deployed on Neutron testnet
        // artifacts.insert(
        //     "astroport_staking".to_string(),
        //     ContractType::Artifact(Artifact::Local(get_wasm_path(
        //         "astroport_staking",
        //         &ARTIFACTS_PATH,
        //         APPEND_ARCH,
        //         &ARCH,
        //     ))),
        // );
        artifacts
    }

    // #[test]
    #[cfg(feature = "chain-download")]
    fn download_neutron_testnet_artifacts() {
        for (name, artifact) in NEUTRON_CONTRACT_ADDRESSES {
            let wasm = artifact
                .into_artifact(NEUTRON_RPC.to_string())
                .get_wasm_byte_code()
                .unwrap();
            // write wasm to disk
            let path = get_wasm_path(name, &Some("artifacts"), false, &None);
            std::fs::write(path, wasm).unwrap();
        }
    }

    /// Creates an RPC test runner and accounts. If `cli` is Some, it will attempt to run the tests
    /// against the configured docker container.
    #[cfg(feature = "rpc-runner")]
    fn get_rpc_runner<'a>() -> OwnedTestRunner<'a> {
        let rpc_runner_config = RpcRunnerConfig::from_yaml(TEST_CONFIG_PATH);

        let runner = RpcRunner::new(rpc_runner_config).unwrap();
        OwnedTestRunner::RpcRunner(runner)
    }

    #[cfg(feature = "rpc-runner")]
    // Commenting out because we have not set up Docker for CI yet.
    // #[test]
    pub fn test_with_rpc_runner() {
        let runner = get_rpc_runner();
        let contracts = get_local_contracts(&runner.as_ref());
        test_instantiate_astroport(runner.as_ref(), contracts);
    }

    #[cfg(feature = "chain-download")]
    // Commenting out test-case because Neutron's RPC node is down and it's breaking CI.
    // #[test]
    #[allow(dead_code)]
    pub fn test_with_neutron_testnet_artifacts() {
        let runner = OwnedTestRunner::from_str(TEST_RUNNER).unwrap();
        let contracts = get_neutron_testnet_artifacts();
        test_instantiate_astroport(runner.as_ref(), contracts);
    }

    fn get_fee_denom<'a>(runner: &'a TestRunner) -> &'a str {
        match runner {
            #[cfg(feature = "rpc-runner")]
            TestRunner::RpcRunner(rpc_runner) => rpc_runner.config.chain_config.denom(),
            _ => "uosmo",
        }
    }

    /// Feature-gated because we use OsmosisTestApp, change the TEST_RUNNER const to use a different runner
    #[cfg(feature = "osmosis-test-tube")]
    #[test]
    fn test_with_local_artifacts() {
        let runner = OwnedTestRunner::from_str(TEST_RUNNER).unwrap();
        let contracts = get_local_contracts(&runner.as_ref());
        test_instantiate_astroport(runner.as_ref(), contracts);
    }

    pub fn test_instantiate_astroport(app: TestRunner, contracts: ContractMap) {
        let accs = app.init_default_accounts().unwrap();
        let native_denom = get_fee_denom(&app);
        let wasm = Wasm::new(&app);

        let admin = &accs[0];

        // Print balances of admin
        let bank = Bank::new(&app);
        let balances = bank
            .query_all_balances(&QueryAllBalancesRequest {
                address: admin.address(),
                pagination: None,
            })
            .unwrap()
            .balances;
        println!("Balances of admin: {:?}", balances);

        // Instantiate contracts
        let contracts = setup_astroport(&app, contracts, admin);

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
            astroport::factory::PairType::Xyk {},
            asset_infos,
            None,
            admin,
            None,
            &[],
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
                &[],
                admin,
            )
            .unwrap();

        // Query allowance
        let allowance_res: AllowanceResponse = wasm
            .query(
                &contracts.astro_token.address,
                &Cw20QueryMsg::Allowance {
                    owner: admin.address(),
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
            min_lp_to_receive: None,
        };
        let _res = wasm.execute(
            &uluna_astro_pair_addr,
            &provide_liq_msg,
            &[Coin {
                amount: Uint128::from(420000000u128),
                denom: native_denom.into(),
            }],
            admin,
        );

        let lp_token_balance = bank
            .query_balance(&QueryBalanceRequest {
                address: admin.address(),
                denom: uluna_astro_lp_token,
            })
            .unwrap();

        assert!(
            Uint128::from_str(&lp_token_balance.balance.unwrap().amount).unwrap() > Uint128::zero()
        );
    }
}
