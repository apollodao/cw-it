use astroport::asset::{Asset, AssetInfo};
use astroport::factory::{ConfigResponse, ExecuteMsg as AstroportFactoryExecuteMsg, PairType};
use cosmwasm_std::{Binary, Coin, Decimal, Uint128};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};
use std::collections::HashMap;
use test_tube::{RunnerResult, SigningAccount};

use super::utils::{parse_astroport_create_pair_events, AstroportContracts};
use crate::robot::TestRobot;
use crate::traits::CwItRunner;
use crate::ContractMap;

pub trait AstroportTestRobot<'a, R>: TestRobot<'a, R>
where
    R: CwItRunner<'a> + 'a,
{
    fn astroport_contracts(&self) -> &AstroportContracts;

    /// Instantiates the astroport contracts, returning a struct containing the addresses and code
    /// ids of the contracts.
    fn instantiate_astroport_contracts(
        runner: &'a R,
        admin: &SigningAccount,
        code_ids: &HashMap<String, u64>,
    ) -> AstroportContracts {
        crate::astroport::utils::instantiate_astroport(runner, admin, code_ids)
    }

    /// Uploads and instantiates the astroport contracts, returning a struct containing the
    /// addresses and code ids of the contracts.
    fn upload_and_init_astroport_contracts(
        runner: &'a R,
        contracts: ContractMap,
        admin: &SigningAccount,
    ) -> AstroportContracts {
        crate::astroport::utils::setup_astroport(runner, contracts, admin)
    }

    /// Queries the balance of a CW20 token for the given address.
    fn query_cw20_balance(&self, cw20_addr: &str, address: &str) -> Uint128 {
        let msg = Cw20QueryMsg::Balance {
            address: address.to_string(),
        };
        let res: BalanceResponse = self.wasm().query(cw20_addr, &msg).unwrap();
        res.balance
    }

    /// Queries the balance of an Astroport AssetInfo for the given address.
    fn query_asset_balance(&self, asset: &AssetInfo, address: &str) -> Uint128 {
        match asset {
            AssetInfo::NativeToken { denom } => self.query_native_token_balance(address, denom),
            AssetInfo::Token { contract_addr } => {
                self.query_cw20_balance(contract_addr.as_str(), address)
            }
        }
    }

    /// Asserts that the balance of an Astroport AssetInfo for the given address is less than the
    /// expected amount.
    fn assert_asset_balance_lt(
        &self,
        asset: &AssetInfo,
        address: &str,
        expected: impl Into<Uint128>,
    ) -> &Self {
        let actual = self.query_asset_balance(asset, address);
        assert!(actual < expected.into());
        self
    }

    /// Asserts that the balance of an Astroport AssetInfo for the given address is greater than the
    /// expected amount.
    fn assert_asset_balance_gt(
        &self,
        asset: &AssetInfo,
        address: &str,
        expected: impl Into<Uint128>,
    ) -> &Self {
        let actual = self.query_asset_balance(asset, address);
        assert!(actual > expected.into());
        self
    }

    /// Asserts that the balance of an Astroport AssetInfo for the given address is equal to the
    /// expected amount.
    fn assert_asset_balance_eq(
        &self,
        asset: &AssetInfo,
        address: &str,
        expected: impl Into<Uint128>,
    ) -> &Self {
        let actual = self.query_asset_balance(asset, address);
        assert_eq!(actual, expected.into());
        self
    }

    /// Queries the LP token balance given the pair's address and the address of the account.
    fn query_lp_token_balance(&self, pair_addr: &str, address: &str) -> Uint128 {
        // Get lp token address
        let msg = astroport::pair::QueryMsg::Pair {};
        let lp_token_addr = self
            .wasm()
            .query::<_, astroport::asset::PairInfo>(pair_addr, &msg)
            .unwrap()
            .liquidity_token;

        // Query balance
        self.query_cw20_balance(lp_token_addr.as_str(), address)
    }

    /// Queries the PairInfo of the given pair.
    fn query_pair_info(&self, pair_addr: &str) -> astroport::asset::PairInfo {
        let msg = astroport::pair::QueryMsg::Pair {};
        self.wasm()
            .query::<_, astroport::asset::PairInfo>(pair_addr, &msg)
            .unwrap()
    }

    /// Queries the PoolInfo of the given pair (contains the reserves and the total supply of LP tokens).
    fn query_pool(&self, pair_addr: &str) -> astroport::pair::PoolResponse {
        let msg = astroport::pair::QueryMsg::Pool {};
        self.wasm()
            .query::<_, astroport::pair::PoolResponse>(pair_addr, &msg)
            .unwrap()
    }

    /// Queries the Config of the Astroport Factory contract.
    fn query_factory_config(&self) -> ConfigResponse {
        let msg = astroport::factory::QueryMsg::Config {};
        self.wasm()
            .query::<_, ConfigResponse>(&self.astroport_contracts().factory.address, &msg)
            .unwrap()
    }

    /// Queries the precision of a native denom on the coin registry.
    fn query_native_coin_registry(
        &self,
        denom: &str,
    ) -> RunnerResult<ap_native_coin_registry::CoinResponse> {
        let msg = ap_native_coin_registry::QueryMsg::NativeToken {
            denom: denom.to_string(),
        };
        self.wasm()
            .query::<_, ap_native_coin_registry::CoinResponse>(
                &self.astroport_contracts().coin_registry.address,
                &msg,
            )
    }

    /// Adds the given native coin denoms and their precisions to the registry.
    fn add_native_coins_to_registry(
        &self,
        native_coins: Vec<(String, u8)>,
        signer: &SigningAccount,
    ) -> &Self {
        let msg = ap_native_coin_registry::ExecuteMsg::Add { native_coins };
        self.wasm()
            .execute(
                &self.astroport_contracts().coin_registry.address,
                &msg,
                &[],
                signer,
            )
            .unwrap();
        self
    }

    /// Provides liquidity to the given pair.
    fn provide_liquidity(
        &self,
        pair_addr: &str,
        assets: Vec<Asset>,
        signer: &SigningAccount,
    ) -> &Self {
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
                    self.wasm()
                        .execute(contract_addr.as_ref(), &msg, &[], signer)
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

        // Provide liquidity
        let msg = astroport::pair::ExecuteMsg::ProvideLiquidity {
            assets,
            slippage_tolerance: None,
            receiver: None,
            auto_stake: Some(false),
        };
        self.wasm()
            .execute(pair_addr, &msg, &funds, signer)
            .unwrap();

        self
    }

    /// Creates a new pair with the given assets and initial liquidity.
    fn create_astroport_pair(
        &self,
        pair_type: PairType,
        asset_infos: [AssetInfo; 2],
        init_params: Option<Binary>,
        signer: &SigningAccount,
        initial_liquidity: Option<[Uint128; 2]>,
    ) -> (String, String) {
        // If the pair is a stableswap pair, add the native coins to the registry
        if let PairType::Stable {} = pair_type {
            //Query factory for native coin registry address
            let native_coins = asset_infos
                .iter()
                .filter_map(|info| match info {
                    AssetInfo::NativeToken { denom } => Some((denom.to_string(), 6)),
                    _ => None,
                })
                .collect();
            self.add_native_coins_to_registry(native_coins, signer);
        }

        let msg = AstroportFactoryExecuteMsg::CreatePair {
            pair_type,
            asset_infos: asset_infos.to_vec(),
            init_params,
        };
        let res = self
            .wasm()
            .execute(
                &self.astroport_contracts().factory.address,
                &msg,
                &[],
                signer,
            )
            .unwrap();

        // Get pair and lp_token addresses from event
        let (pair_addr, lp_token_addr) = parse_astroport_create_pair_events(&res.events);

        if let Some(initial_liquidity) = initial_liquidity {
            let assets = asset_infos
                .into_iter()
                .zip(initial_liquidity.into_iter())
                .map(|(info, amount)| Asset { info, amount })
                .collect();
            self.provide_liquidity(&pair_addr, assets, signer);
        }

        (pair_addr, lp_token_addr)
    }

    fn query_simulate_swap(
        &self,
        pair_addr: &str,
        offer_asset: Asset,
        ask_asset_info: Option<AssetInfo>,
    ) -> astroport::pair::SimulationResponse {
        let msg = astroport::pair::QueryMsg::Simulation {
            offer_asset,
            ask_asset_info,
        };
        self.wasm()
            .query::<_, astroport::pair::SimulationResponse>(pair_addr, &msg)
            .unwrap()
    }

    /// Swaps `offer_asset` for `ask_asset_info` on the given Astroport pair.
    fn swap_on_astroport_pair(
        &self,
        pair_addr: &str,
        offer_asset: Asset,
        ask_asset_info: Option<AssetInfo>,
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
        signer: &SigningAccount,
    ) -> &Self {
        // Increase allowance for cw20 tokens and add coins to funds
        let funds = match &offer_asset.info {
            AssetInfo::Token { contract_addr } => {
                let msg = Cw20ExecuteMsg::IncreaseAllowance {
                    spender: pair_addr.to_string(),
                    amount: offer_asset.amount,
                    expires: None,
                };
                self.wasm()
                    .execute(contract_addr.as_ref(), &msg, &[], signer)
                    .unwrap();
                vec![]
            }
            AssetInfo::NativeToken { denom } => {
                vec![Coin {
                    denom: denom.to_string(),
                    amount: offer_asset.amount,
                }]
            }
        };

        let msg = astroport::pair::ExecuteMsg::Swap {
            offer_asset,
            ask_asset_info,
            belief_price,
            max_spread,
            to: None,
        };
        self.wasm()
            .execute(pair_addr, &msg, &funds, signer)
            .unwrap();
        self
    }

    fn add_denom_precision_to_coin_registry(
        &self,
        denom: impl Into<String>,
        precision: u8,
        signer: &SigningAccount,
    ) -> &Self {
        let msg = astroport::native_coin_registry::ExecuteMsg::Add {
            native_coins: vec![(denom.into(), precision)],
        };
        self.wasm()
            .execute(
                &self.astroport_contracts().coin_registry.address,
                &msg,
                &[],
                signer,
            )
            .unwrap();
        self
    }
}

// Feature gated because we use OsmosisTestApp by default
#[cfg(feature = "osmosis-test-tube")]
#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use astroport::{
        asset::{Asset, AssetInfo},
        factory::PairType,
        pair::StablePoolParams,
    };
    use cosmwasm_std::{to_binary, Binary, Uint128};
    use test_case::test_case;
    use test_tube::{Account, SigningAccount};

    use super::AstroportTestRobot;
    use crate::{
        astroport::utils::{cw20_info, native_info, AstroportContracts},
        robot::TestRobot,
        ContractMap, TestRunner,
    };

    struct TestingRobot<'a> {
        runner: &'a TestRunner<'a>,
        accs: Vec<SigningAccount>,
        astroport_contracts: AstroportContracts,
    }
    impl<'a> TestingRobot<'a> {
        fn new(runner: &'a TestRunner<'a>, contracts: ContractMap) -> Self {
            // Initialize accounts
            let accs = runner.init_accounts();
            let admin = &accs[0];

            // Upload and initialize contracts
            let astroport_contracts =
                Self::upload_and_init_astroport_contracts(runner, contracts, admin);

            Self {
                runner,
                accs,
                astroport_contracts,
            }
        }
    }

    impl<'a> TestRobot<'a, TestRunner<'a>> for TestingRobot<'a> {
        fn runner(&self) -> &'a TestRunner<'a> {
            self.runner
        }
    }

    impl<'a> AstroportTestRobot<'a, TestRunner<'a>> for TestingRobot<'a> {
        fn astroport_contracts(&self) -> &AstroportContracts {
            &self.astroport_contracts
        }
    }

    /// cw-optimizoor adds the CPU architecture to the wasm file name
    pub const APPEND_ARCH: bool = true;
    pub const ARCH: Option<&str> = Some("aarch64");

    /// The path to the artifacts folder
    pub const ARTIFACTS_PATH: Option<&str> = Some("artifacts/042b076");

    /// Which TestRunner to use
    pub const TEST_RUNNER: &str = "osmosis-test-app";
    // pub const TEST_RUNNER: &str = "multi-test";

    /// Get astroport artifacts already from disk
    pub fn get_contracts(test_runner: &TestRunner) -> ContractMap {
        crate::astroport::utils::get_local_contracts(
            test_runner,
            &ARTIFACTS_PATH,
            APPEND_ARCH,
            &ARCH,
        )
    }

    fn get_test_robot<'a>(runner: &'a TestRunner) -> TestingRobot<'a> {
        let contracts = get_contracts(runner);
        TestingRobot::new(runner, contracts)
    }

    /// Helper to get a pair of native token asset infos.
    fn native_native_pair() -> [AssetInfo; 2] {
        [native_info("uatom"), native_info("uion")]
    }

    /// Helper enum for choice of asset infos.
    enum AssetChoice {
        NativeNative,
        NativeCw20,
    }

    /// Helper to get asset infos for a given choice.
    fn get_asset_infos(choice: AssetChoice, astro_token: &str) -> [AssetInfo; 2] {
        match choice {
            AssetChoice::NativeNative => native_native_pair(),
            AssetChoice::NativeCw20 => [native_info("uatom"), cw20_info(astro_token)],
        }
    }

    /// Returns some stable pool initialization params.
    fn stable_init_params() -> Option<Binary> {
        Some(to_binary(&StablePoolParams { amp: 10 }).unwrap())
    }

    #[test]
    fn test_upload_and_init_astroport() {
        get_test_robot(&TestRunner::from_str(TEST_RUNNER).unwrap());
    }

    #[test]
    fn test_query_factory_config() {
        let runner = TestRunner::from_str(TEST_RUNNER).unwrap();
        let robot = get_test_robot(&runner);

        let astro_contracts = &robot.astroport_contracts;

        robot.query_factory_config();
    }

    #[test_case(PairType::Xyk {},AssetChoice::NativeNative,None,None; "XYK, native-native, no liq")]
    #[test_case(PairType::Xyk {},AssetChoice::NativeNative,None,Some([420420,696969]); "XYK, native-native, with liq")]
    #[test_case(PairType::Xyk {},AssetChoice::NativeCw20,None,Some([420420,696969]); "XYK, native-cw20, with liq")]
    #[test_case(PairType::Stable {},AssetChoice::NativeNative,stable_init_params(),None; "Stable, native-native, no liq")]
    #[test_case(PairType::Stable {},AssetChoice::NativeNative,stable_init_params(),Some([420420,696969]); "Stable, native-native, with liq")]
    fn test_create_astroport_pair(
        pair_type: PairType,
        asset_info_choice: AssetChoice,
        init_params: Option<Binary>,
        initial_liquidity: Option<[u128; 2]>,
    ) {
        let runner = TestRunner::from_str(TEST_RUNNER).unwrap();
        let robot = get_test_robot(&runner);

        let contracts = &robot.astroport_contracts;
        let admin = &robot.accs[0];

        let asset_infos = get_asset_infos(asset_info_choice, &contracts.astro_token.address);

        let (pair_addr, lp_token_addr) = robot.create_astroport_pair(
            pair_type.clone(),
            asset_infos.clone(),
            init_params,
            admin,
            initial_liquidity.map(|liq| liq.map(Uint128::from)),
        );

        // Check pair info
        let pair_info = robot.query_pair_info(&pair_addr);
        assert_eq!(pair_info.pair_type, pair_type);
        assert_eq!(pair_info.asset_infos, asset_infos.to_vec());
        assert_eq!(pair_info.liquidity_token.to_string(), lp_token_addr);

        if let Some(initial_liq) = initial_liquidity {
            // Check lp token balance
            let lp_token_balance = robot.query_cw20_balance(&lp_token_addr, &admin.address());
            assert_ne!(lp_token_balance, Uint128::zero());

            // Check pair reserves
            let pool_res = robot.query_pool(&pair_addr);

            pool_res
                .assets
                .iter()
                .zip(initial_liq.iter())
                .for_each(|(asset, liq)| {
                    assert_eq!(asset.amount.u128(), *liq);
                });
        }
    }

    #[test_case(PairType::Xyk {},AssetChoice::NativeNative,None; "Swap on XYK, native-native")]
    #[test_case(PairType::Xyk {},AssetChoice::NativeCw20,None; "Swap on XYK, native-cw20")]
    fn test_swap_on_pair(
        pair_type: PairType,
        asset_info_choice: AssetChoice,
        init_params: Option<Binary>,
    ) {
        let runner = TestRunner::from_str(TEST_RUNNER).unwrap();
        let contracts = get_contracts(&runner);
        let robot = TestingRobot::new(&runner, contracts);

        let contracts = &robot.astroport_contracts;
        let admin = &robot.accs[0];
        let admin_addr = &admin.address();

        let asset_infos = get_asset_infos(asset_info_choice, &contracts.astro_token.address);
        let initial_liquidity = Some([Uint128::from(420420u128), Uint128::from(696969u128)]);
        let (pair_addr, _lp_token_addr) = robot.create_astroport_pair(
            pair_type,
            asset_infos.clone(),
            init_params,
            admin,
            initial_liquidity,
        );

        let swap_amount = Uint128::from(1000u128);
        let offer_asset_info = &asset_infos[0];
        let offer_asset = Asset {
            info: offer_asset_info.clone(),
            amount: swap_amount,
        };
        let ask_asset_info = &asset_infos[1];

        // First simulate
        let simulation = robot.query_simulate_swap(
            &pair_addr,
            offer_asset.clone(),
            Some(ask_asset_info.clone()),
        );

        // Query balance before swap
        let offer_balance_before = robot.query_asset_balance(&offer_asset.info, admin_addr);
        let ask_balance_before = robot.query_asset_balance(ask_asset_info, admin_addr);

        //Perform swap and assert result
        robot
            .swap_on_astroport_pair(
                &pair_addr,
                offer_asset,
                Some(ask_asset_info.clone()),
                None,
                None,
                admin,
            )
            .assert_asset_balance_eq(
                offer_asset_info,
                admin_addr,
                offer_balance_before - swap_amount,
            )
            .assert_asset_balance_eq(
                ask_asset_info,
                admin_addr,
                ask_balance_before + simulation.return_amount,
            );
    }
}
