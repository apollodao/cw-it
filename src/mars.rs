use crate::config::TestConfig;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{to_binary, Binary, Decimal, Event, Uint128, Uint64};
use mars_mock_oracle::msg::{CoinPrice, InstantiateMsg as MockOracleInstantiateMsg};
use mars_mock_red_bank::msg::{CoinMarketInfo, InstantiateMsg as RedBankInstantiateMsg};
use mars_rover::adapters::account_nft::InstantiateMsg as NftInstantiateMsg;
use mars_rover::adapters::oracle::OracleBase;
use mars_rover::adapters::red_bank::RedBank;
use mars_rover::adapters::red_bank::RedBankBase;
use mars_rover::adapters::swap::{InstantiateMsg as SwapperInstantiateMsg, SwapperBase};
use mars_rover::adapters::vault::VaultConfig;
use mars_rover::adapters::vault::VaultUnchecked;
use mars_rover::adapters::zapper::ZapperBase;
use mars_rover::msg::instantiate::ConfigUpdates;
use mars_rover::msg::instantiate::VaultInstantiateConfig;
use mars_rover::msg::zapper::{InstantiateMsg as ZapperInstantiateMsg, LpConfig};
use mars_rover::msg::ExecuteMsg as CreditManagerExecuteMsg;
use mars_rover::msg::InstantiateMsg as CreditManagerInstantiateMsg;
use mars_rover::msg::QueryMsg as CreditManagerQueryMsg;
use mars_rover::msg::query::ConfigResponse;
use osmosis_testing::{Account, Module, Runner, SigningAccount, Wasm};
use std::collections::HashMap;
use std::path::Path;

pub const MARS_CONTRACT_NAMES: [&str; 6] = [
    "mars_credit_manager",
    "mars_zapper_mock",
    "mars_mock_oracle",
    "mars_swapper_mock",
    "mars_mock_red_bank",
    "mars_account_nft",
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
pub struct MarsContracts {
    pub credit_manager: Contract,
    pub oracle: Contract,
    pub swapper: Contract,
    pub zapper: Contract,
    pub red_bank: Contract,
}

pub fn setup_mars<'a, R>(
    app: &'a R,
    test_config: &TestConfig,
    admin: &SigningAccount,
) -> MarsContracts
where
    R: Runner<'a>,
{
    // Upload contracts
    let code_ids = upload_mars_contracts(app, test_config, admin);

    // Instantiate contracts
    instantiate_mars(app, admin, &code_ids)
}

pub fn upload_mars_contracts<'a, R>(
    app: &'a R,
    test_config: &TestConfig,
    signer: &SigningAccount,
) -> HashMap<String, u64>
where
    R: Runner<'a>,
{
    let wasm = Wasm::new(app);
    let mut code_ids: HashMap<String, u64> = HashMap::new();
    for contract_name in MARS_CONTRACT_NAMES {
        let path = Path::new(&test_config.artifacts_folder).join(format!("{}.wasm", contract_name));
        println!("Uploading {:?} ...", path);
        let wasm_byte_code = std::fs::read(path).unwrap();
        let code_id = wasm
            .store_code(&wasm_byte_code, None, signer)
            .unwrap()
            .data
            .code_id;
        code_ids.insert(contract_name.to_string(), code_id);
    }
    code_ids
}

pub fn instantiate_mars<'a, R>(
    app: &'a R,
    admin: &SigningAccount,
    code_ids: &HashMap<String, u64>,
) -> MarsContracts
where
    R: Runner<'a>,
{
    let wasm = Wasm::new(app);

    println!("Instantiating mock oracle ...");
    let mock_oracle = wasm
        .instantiate(
            code_ids["mars_mock_oracle"],
            &MockOracleInstantiateMsg {
                prices: vec![CoinPrice {
                    denom: "usdc".to_string(),
                    price: Decimal::one(),
                }],
            },
            Some(&admin.address()),
            Some("Mock Oracle"),
            &vec![],
            admin,
        )
        .unwrap()
        .data
        .address;

    println!("Instantiating zapper ...");
    let mock_zapper = wasm
        .instantiate(
            code_ids["mars_zapper_mock"],
            &ZapperInstantiateMsg {
                oracle: OracleBase::new(mock_oracle.clone()),
                lp_configs: vec![LpConfig {
                    lp_token_denom: "gamm/pool/1".to_string(),
                    lp_pair_denoms: ("uosmo".to_string(), "usdc".to_string()),
                }],
            },
            Some(&admin.address()),
            Some("Mock Zapper"),
            &vec![],
            admin,
        )
        .unwrap()
        .data
        .address;

    println!("Instantiating red bank ...");
    let red_bank = wasm
        .instantiate(
            code_ids["mars_mock_red_bank"],
            &RedBankInstantiateMsg {
                coins: vec![CoinMarketInfo {
                    denom: "usdc".to_string(),
                    max_ltv: Decimal::from_ratio(4u8, 5u8),
                    liquidation_threshold: Decimal::from_ratio(1u8, 20u8),
                    liquidation_bonus: Decimal::from_ratio(1u8, 100u8),
                }],
            },
            Some(&admin.address()),
            Some("Mock Mars Red Bank"),
            &vec![],
            admin,
        )
        .unwrap()
        .data
        .address;

    println!("Instantiating mock swapper ...");
    let mock_swapper = wasm
        .instantiate(
            code_ids["mars_swapper_mock"],
            &SwapperInstantiateMsg {
                owner: admin.address(),
            },
            Some(&admin.address()),
            Some("Mock Mars Swapper"),
            &vec![],
            admin,
        )
        .unwrap()
        .data
        .address;

    println!("Instantiating credit manager ...");
    let credit_manager = wasm
        .instantiate(
            code_ids["mars_credit_manager"],
            &CreditManagerInstantiateMsg {
                owner: admin.address(),
                allowed_coins: vec!["uosmo".to_string(), "usdc".to_string()],
                vault_configs: vec![],
                red_bank: RedBankBase::new(red_bank.clone()),
                oracle: OracleBase::new(mock_oracle.clone()),
                max_close_factor: Decimal::from_ratio(1u8, 5u8),
                max_unlocking_positions: Uint128::new(5),
                swapper: SwapperBase::new(mock_swapper.clone()),
                zapper: ZapperBase::new(mock_zapper.clone()),
            },
            Some(&admin.address()),
            Some("Credit Manager"),
            &vec![],
            admin,
        )
        .unwrap()
        .data
        .address;

    println!("Instantiating account nft ...");
    let account_nft = wasm
        .instantiate(
            code_ids["mars_account_nft"],
            &NftInstantiateMsg {
                max_value_for_burn: Default::default(),
                name: "Rover Credit Account".to_string(),
                symbol: "RCA".to_string(),
                minter: admin.address(),
            },
            Some(&admin.address()),
            Some("Account NFT"),
            &vec![],
            admin,
        )
        .unwrap()
        .data
        .address;

    println!("Update credit manager config for account NFT");

    let config_res: ConfigResponse = wasm
        .query(&credit_manager, &CreditManagerQueryMsg::Config {})
        .unwrap();

    println!("config_res : {:?}", config_res);

    println!("admin: {}", admin.address());

    wasm.execute(
        &credit_manager,
        &CreditManagerExecuteMsg::UpdateConfig {
            updates: ConfigUpdates {
                account_nft: Some(account_nft),
                allowed_coins: None,
                oracle: None,
                max_close_factor: None,
                max_unlocking_positions: None,
                swapper: None,
                vault_configs: None,
                zapper: None,
            },
        },
        &vec![],
        admin,
    )
    .unwrap();

    MarsContracts {
        credit_manager: Contract::new(credit_manager, code_ids["mars_credit_manager"]),
        oracle: Contract::new(mock_oracle, code_ids["mars_mock_oracle"]),
        swapper: Contract::new(mock_swapper, code_ids["mars_swapper_mock"]),
        zapper: Contract::new(mock_zapper, code_ids["mars_zapper_mock"]),
        red_bank: Contract::new(red_bank, code_ids["mars_mock_red_bank"]),
    }
}

// #[cfg(test)]
// mod tests {
//     use astroport::{
//         asset::{Asset, AssetInfo},
//         factory::PairType,
//     };
//     use cosmrs::proto::cosmos::bank::v1beta1::QueryAllBalancesRequest;
//     use cosmwasm_std::{Addr, Coin, Decimal, Uint128};
//     use cw20::{AllowanceResponse, BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};
//     use osmosis_testing::{Account, Bank, Module, Wasm};
//     use testcontainers::clients::Cli;

//     use crate::{
//         app::App as RpcRunner,
//         astroport::{create_astroport_pair, setup_astroport},
//         config::TestConfig,
//     };
//     use astroport::pair::ExecuteMsg as PairExecuteMsg;
//     use std::str::FromStr;

//     pub const TEST_CONFIG_PATH: &str = "configs/terra.yaml";

//     #[test]
//     pub fn test_instantiate_astroport_with_localterra() {
//         // let _ = env_logger::builder().is_test(true).try_init();
//         let docker: Cli = Cli::default();
//         let test_config = TestConfig::from_yaml(TEST_CONFIG_PATH);
//         let app = RpcRunner::new(test_config.clone(), &docker);
//         let accs = app
//             .test_config
//             .import_all_accounts()
//             .into_values()
//             .collect::<Vec<_>>();
//         let wasm = Wasm::new(&app);

//         let admin = &accs[0];

//         // Print balances of admin
//         let bank = Bank::new(&app);
//         let balances = bank
//             .query_all_balances(&QueryAllBalancesRequest {
//                 address: admin.address().to_string(),
//                 pagination: None,
//             })
//             .unwrap()
//             .balances;
//         println!("Balances of admin: {:?}", balances);

//         // Instantiate contracts
//         let contracts = setup_astroport(&app, &test_config, admin);

//         // Create XYK pool
//         let asset_infos: [AssetInfo; 2] = [
//             AssetInfo::NativeToken {
//                 denom: "uluna".into(),
//             },
//             AssetInfo::Token {
//                 contract_addr: Addr::unchecked(&contracts.astro_token.address),
//             },
//         ];
//         let (uluna_astro_pair_addr, uluna_astro_lp_token) = create_astroport_pair(
//             &app,
//             &contracts.factory.address,
//             PairType::Xyk {},
//             asset_infos.clone(),
//             None,
//             admin,
//         );

//         // Increase allowance of astro token
//         let increase_allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
//             spender: uluna_astro_pair_addr.clone(),
//             amount: Uint128::from(1000000000u128),
//             expires: None,
//         };
//         let _res = wasm
//             .execute(
//                 &contracts.astro_token.address,
//                 &increase_allowance_msg,
//                 &vec![],
//                 admin,
//             )
//             .unwrap();

//         // Query allowance
//         let allowance_res: AllowanceResponse = wasm
//             .query(
//                 &contracts.astro_token.address,
//                 &Cw20QueryMsg::Allowance {
//                     owner: admin.address().to_string(),
//                     spender: uluna_astro_pair_addr.clone(),
//                 },
//             )
//             .unwrap();
//         assert_eq!(allowance_res.allowance, Uint128::from(1000000000u128));

//         // Provide liquidity to XYK pool
//         let provide_liq_msg = PairExecuteMsg::ProvideLiquidity {
//             assets: [
//                 Asset {
//                     amount: Uint128::from(420000000u128),
//                     info: AssetInfo::NativeToken {
//                         denom: "uluna".into(),
//                     },
//                 },
//                 Asset {
//                     amount: Uint128::from(690000000u128),
//                     info: AssetInfo::Token {
//                         contract_addr: Addr::unchecked(&contracts.astro_token.address),
//                     },
//                 },
//             ],
//             slippage_tolerance: Some(Decimal::from_str("0.02").unwrap()),
//             auto_stake: Some(false),
//             receiver: None,
//         };
//         let _res = wasm.execute(
//             &uluna_astro_pair_addr,
//             &provide_liq_msg,
//             &vec![Coin {
//                 amount: Uint128::from(420000000u128),
//                 denom: "uluna".into(),
//             }],
//             admin,
//         );

//         // Query LP token balance
//         let lp_token_balance: BalanceResponse = wasm
//             .query(
//                 &uluna_astro_lp_token,
//                 &Cw20QueryMsg::Balance {
//                     address: admin.address().to_string(),
//                 },
//             )
//             .unwrap();
//         println!("LP token balance: {:?}", lp_token_balance);
//         assert!(lp_token_balance.balance > Uint128::zero());
//     }
// }
