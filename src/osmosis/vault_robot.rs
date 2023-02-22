use cosmwasm_schema::cw_serde;
use cosmwasm_std::{testing::MockApi, Coin};
use cw_vault_standard::{VaultStandardExecuteMsg, VaultStandardQueryMsg};
use osmosis_test_tube::{Account, Module, OsmosisTestApp, Runner, SigningAccount, Wasm};

use apollo_cw_asset::{AssetInfo, AssetInfoUnchecked};
use apollo_vault::msg::{
    ApolloExtensionQueryMsg, ExtensionExecuteMsg, ExtensionQueryMsg, StateResponse,
};
use apollo_vault::state::ConfigUnchecked;

use crate::config::TestConfig;
use crate::helpers::{
    bank_balance_query, bank_send, instantiate_contract, instantiate_contract_with_funds,
    upload_wasm_files,
};
use crate::osmosis::create_osmosis_pool;
use cosmwasm_std::{to_binary, Addr, Decimal, Empty, Uint128};
use cw_dex::osmosis::{OsmosisPool, OsmosisStaking};
use cw_dex::traits::Pool as PoolTrait;
use cw_dex::Pool;
use cw_dex_router::helpers::CwDexRouterUnchecked;
use cw_dex_router::operations::{SwapOperation, SwapOperationsList};
use cw_vault_standard::extensions::lockup::{LockupExecuteMsg, LockupQueryMsg, UnlockingPosition};
use std::time::Duration;

use cw_vault_token::osmosis::OsmosisDenom;
use liquidity_helper::{LiquidityHelper, LiquidityHelperUnchecked};
use osmosis_test_tube::cosmrs::proto::cosmwasm::wasm::v1::MsgExecuteContractResponse;

use super::OsmosisTestPool;

// TODO: Replace with imported messages
/// ExecuteMsg for an Autocompounding Vault.
pub type ExecuteMsg = VaultStandardExecuteMsg<ExtensionExecuteMsg>;

/// QueryMsg for an Autocompounding Vault.
pub type QueryMsg = VaultStandardQueryMsg<ExtensionQueryMsg>;

#[cw_serde]
pub struct InstantiateMsg {
    /// Address that is allowed to update config.
    pub admin: String,
    /// The ID of the pool that this vault will autocompound.
    pub pool_id: u64,
    /// The lockup duration in seconds that this vault will use when staking
    /// LP tokens.
    pub lockup_duration: u64,
    /// Configurable parameters for the contract.
    pub config: ConfigUnchecked,
    /// The subdenom that will be used for the native vault token, e.g.
    /// the denom of the vault token will be:
    /// "factory/{vault_contract}/{vault_token_subdenom}".
    pub vault_token_subdenom: String,
}

const UOSMO: &str = "uosmo";

pub struct OsmosisVaultRobot<'a, R: Runner<'a>> {
    app: &'a R,
    vault_addr: String,
    base_pool: OsmosisPool,
    liquidity_helper: LiquidityHelper,
}

impl<'a, R: Runner<'a>> OsmosisVaultRobot<'a, R> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        app: &'a R,
        admin: &SigningAccount,
        force_withdraw_admin: &SigningAccount,
        treasury: &SigningAccount,
        base_pool: OsmosisTestPool,
        reward1_pool: OsmosisTestPool,
        reward2_pool: Option<OsmosisTestPool>,
        reward_liquidation_target: String,
        performance_fee: Decimal,
        test_config_path: &str,
    ) -> Self {
        let api = MockApi::default();

        let test_config = TestConfig::from_yaml(test_config_path);

        // Create base pool (the pool this vault will compound)
        let base_pool_id =
            create_osmosis_pool(app, &base_pool.pool_type, &base_pool.liquidity, admin);
        let base_pool = OsmosisPool::unchecked(base_pool_id);

        // Create pool for first reward token
        let reward1_pool_id =
            create_osmosis_pool(app, &reward1_pool.pool_type, &reward1_pool.liquidity, admin);
        let reward1_token = reward1_pool
            .liquidity
            .iter()
            .find(|x| x.denom != reward_liquidation_target)
            .unwrap()
            .denom
            .clone();
        let reward1_pool = OsmosisPool::unchecked(reward1_pool_id);

        // Create pool for second reward token (if set)
        let reward2_osmosis_pool = reward2_pool.clone().map(|pool| {
            let rewards2_pool_id =
                create_osmosis_pool(app, &pool.pool_type, &pool.liquidity, admin);
            OsmosisPool::unchecked(rewards2_pool_id)
        });
        let reward2_token = reward2_pool.clone().map(|pool| {
            pool.liquidity
                .iter()
                .find(|x| x.denom != reward_liquidation_target)
                .unwrap()
                .denom
                .clone()
        });

        // Upload wasm files
        let code_ids = upload_wasm_files(app, admin, test_config).unwrap();

        // Instantiate Osmosis Liquidity Helper
        let osmosis_liquidity_helper = instantiate_contract::<_, _, LiquidityHelperUnchecked>(
            app,
            admin,
            code_ids["osmosis_liquidity_helper"],
            &Empty {},
        )
        .unwrap();

        // Instantiate CwDexRouter
        let cw_dex_router = instantiate_contract::<_, _, CwDexRouterUnchecked>(
            app,
            admin,
            code_ids["cw_dex_router"],
            &Empty {},
        )
        .unwrap()
        .check(&api)
        .unwrap();

        // Update paths for CwDexRouter
        let update_path_for_reward_pool = |reward_token: String, pool: Pool| {
            let msg = cw_dex_router
                .set_path_msg(
                    AssetInfo::Native(reward_token.clone()),
                    AssetInfo::Native(reward_liquidation_target.clone()),
                    &SwapOperationsList::new(vec![SwapOperation {
                        offer_asset_info: AssetInfo::Native(reward_token),
                        ask_asset_info: AssetInfo::Native(reward_liquidation_target.clone()),
                        pool,
                    }]),
                    false,
                )
                .unwrap();
            app.execute_cosmos_msgs::<MsgExecuteContractResponse>(&[msg], admin)
                .unwrap();
        };
        update_path_for_reward_pool(reward1_token.clone(), Pool::Osmosis(reward1_pool));
        if let Some(reward2_token) = &reward2_token {
            update_path_for_reward_pool(
                reward2_token.clone(),
                Pool::Osmosis(reward2_osmosis_pool.unwrap()),
            );
        }

        // Create vault config
        let reward_assets = [reward1_token.clone(), reward2_token.unwrap_or_default()]
            .into_iter()
            .filter(|x| !x.is_empty())
            .map(|x| AssetInfoUnchecked::Native(x.to_string()))
            .collect();
        let config = ConfigUnchecked {
            force_withdraw_whitelist: vec![force_withdraw_admin.address()],
            performance_fee,
            reward_assets,
            reward_liquidation_target: AssetInfoUnchecked::Native(reward_liquidation_target),
            treasury: treasury.address(),
            liquidity_helper: osmosis_liquidity_helper.clone(),
            router: cw_dex_router.clone().into(),
        };

        // Instantiate osmosis vault contract
        let vault_addr: String = instantiate_contract_with_funds(
            app,
            admin,
            code_ids["osmosis_vault"],
            &InstantiateMsg {
                admin: admin.address(),
                lockup_duration: 86400u64,
                pool_id: base_pool.pool_id(),
                vault_token_subdenom: "osmosis-vault".to_string(),
                config,
            },
            &[Coin {
                denom: UOSMO.to_string(),
                amount: Uint128::from(10_000_000u128), // 10 OSMO needed to create vault token
            }],
        )
        .unwrap();

        println!(" ------ Addresses -------");
        println!("admin: {}", admin.address());
        println!("force_withdraw_admin: {}", force_withdraw_admin.address());
        println!("treasury: {}", treasury.address());

        println!(" ------ Contracts -------");
        println!("Vault: {}", vault_addr);
        println!("Liquidity helper: {:?}", osmosis_liquidity_helper);
        println!("CwDexRouter: {}", cw_dex_router.clone().addr());
        println!("-----------------------------------");

        Self {
            app,
            vault_addr,
            base_pool,
            liquidity_helper: LiquidityHelper::new(Addr::unchecked(osmosis_liquidity_helper.0)),
        }
    }

    pub fn query_state(&self) -> StateResponse<OsmosisStaking, OsmosisPool, OsmosisDenom> {
        let wasm = Wasm::new(self.app);
        wasm.query(
            &self.vault_addr,
            &QueryMsg::VaultExtension(ExtensionQueryMsg::Apollo(ApolloExtensionQueryMsg::State {})),
        )
        .unwrap()
    }

    pub fn query_vault_token_balance(&self, address: &str) -> Uint128 {
        let state = self.query_state();
        let vault_token_denom = state.vault_token.to_string();
        bank_balance_query(self.app, address.to_string(), vault_token_denom).unwrap()
    }

    pub fn query_base_token_balance(&self, address: &str) -> Uint128 {
        bank_balance_query(
            self.app,
            address.to_string(),
            self.base_pool.lp_token().to_string(),
        )
        .unwrap()
    }

    pub fn assert_vault_token_balance(&self, address: &str, expected: Uint128) -> &Self {
        assert_eq!(self.query_vault_token_balance(address), expected);

        self
    }

    pub fn assert_base_token_balance_eq(&self, address: &str, expected: Uint128) -> &Self {
        assert_eq!(self.query_base_token_balance(address), expected);

        self
    }

    pub fn assert_base_token_balance_gt(&self, address: &str, expected: Uint128) -> &Self {
        assert!(
            self.query_base_token_balance(address) > expected,
            "Expected {} to be greater than {}",
            self.query_base_token_balance(address),
            expected
        );

        self
    }

    pub fn send_base_tokens(&self, from: &SigningAccount, to: &str, amount: Uint128) -> &Self {
        bank_send(
            self.app,
            from,
            to,
            vec![Coin::new(amount.u128(), &self.base_token())],
        )
        .unwrap();

        self
    }

    pub fn provide_liquidity(&self, signer: &SigningAccount, added_liquidity: Vec<Coin>) -> &Self {
        let msgs = self
            .liquidity_helper
            .balancing_provide_liquidity(
                added_liquidity.into(),
                Uint128::zero(),
                to_binary(&self.base_pool).unwrap(),
                None,
            )
            .unwrap();

        self.app
            .execute_cosmos_msgs::<MsgExecuteContractResponse>(&msgs, signer)
            .unwrap();
        self
    }

    pub fn deposit(
        &self,
        signer: &SigningAccount,
        recipient: Option<String>,
        amount: Uint128,
    ) -> &Self {
        let deposit_msg = ExecuteMsg::Deposit { amount, recipient };

        let wasm = Wasm::new(self.app);
        wasm.execute(
            &self.vault_addr,
            &deposit_msg,
            &[Coin {
                amount,
                denom: self.base_token(),
            }],
            signer,
        )
        .unwrap();

        self
    }

    pub fn deposit_all(&self, signer: &SigningAccount, recipient: Option<String>) -> &Self {
        let balance = self.query_base_token_balance(&signer.address());
        self.deposit(signer, recipient, balance)
    }

    pub fn unlock_all(&self, signer: &SigningAccount) -> &Self {
        let balance = self.query_vault_token_balance(&signer.address());
        self.unlock(signer, balance)
    }

    pub fn vault_token(&self) -> String {
        self.query_state().vault_token.to_string()
    }

    pub fn unlock(&self, signer: &SigningAccount, amount: Uint128) -> &Self {
        let unlock_msg =
            ExecuteMsg::VaultExtension(ExtensionExecuteMsg::Lockup(LockupExecuteMsg::Unlock {
                amount,
            }));

        let wasm = Wasm::new(self.app);
        wasm.execute(
            &self.vault_addr,
            &unlock_msg,
            &[Coin {
                amount,
                denom: self.vault_token(),
            }],
            signer,
        )
        .unwrap();

        self
    }

    pub fn query_unlocking_positions(&self, address: &str) -> Vec<UnlockingPosition> {
        let wasm = Wasm::new(self.app);
        wasm.query(
            &self.vault_addr,
            &QueryMsg::VaultExtension(ExtensionQueryMsg::Lockup(
                LockupQueryMsg::UnlockingPositions {
                    owner: address.to_string(),
                    start_after: None,
                    limit: None,
                },
            )),
        )
        .unwrap()
    }

    pub fn withdraw_unlocked(
        &self,
        signer: &SigningAccount,
        recipient: Option<String>,
        lockup_id: u64,
    ) -> &Self {
        let withdraw_msg = ExecuteMsg::VaultExtension(ExtensionExecuteMsg::Lockup(
            LockupExecuteMsg::WithdrawUnlocked {
                recipient,
                lockup_id,
            },
        ));

        let user1_base_token_balance_before = self.query_base_token_balance(&signer.address());

        let wasm = Wasm::new(self.app);
        wasm.execute(&self.vault_addr, &withdraw_msg, &[], signer)
            .unwrap();

        let user1_base_token_balance_after = self.query_base_token_balance(&signer.address());

        assert!(user1_base_token_balance_after > user1_base_token_balance_before);

        self
    }

    pub fn withdraw_first_unlocked(
        &self,
        signer: &SigningAccount,
        recipient: Option<String>,
    ) -> &Self {
        let lockup_id = self.query_unlocking_positions(&signer.address())[0].id;

        self.withdraw_unlocked(signer, recipient, lockup_id)
    }

    pub fn base_token(&self) -> String {
        self.base_pool.lp_token().to_string()
    }

    pub fn assert_total_staked_base_tokens(&self, expected: Uint128) -> &Self {
        let state = self.query_state();
        assert_eq!(state.total_staked_base_tokens, expected);

        self
    }

    pub fn assert_vault_token_supply(&self, expected: Uint128) -> &Self {
        let state = self.query_state();
        assert_eq!(state.vault_token_supply, expected);

        self
    }

    pub fn assert_vault_token_share(&self, address: &str, expected: Decimal) -> &Self {
        let vault_token_supply = self.query_state().vault_token_supply;
        let vault_token_balance = self.query_vault_token_balance(address);

        assert_eq!(
            Decimal::from_ratio(vault_token_balance, vault_token_supply),
            expected
        );

        self
    }

    pub fn simulate_reward_accrual(
        &self,
        signer: &SigningAccount,
        reward_denom: &str,
        amount: Uint128,
    ) -> &Self {
        if amount > Uint128::zero() {
            bank_send(
                self.app,
                signer,
                &self.vault_addr,
                vec![Coin::new(amount.u128(), reward_denom)],
            )
            .unwrap();
        }

        self
    }
}

impl<'a> OsmosisVaultRobot<'a, OsmosisTestApp> {
    pub fn increase_time(&self, duration: Duration) -> &Self {
        self.app.increase_time(duration.as_secs());

        self
    }
}
