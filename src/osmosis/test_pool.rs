use std::ops::Range;

use apollo_utils::iterators::IntoElementwise;
use cosmwasm_std::{Coin, Uint128};
use osmosis_std::types::osmosis::gamm::{
    poolmodels::{
        balancer::v1beta1::MsgCreateBalancerPool,
        stableswap::v1beta1::{MsgCreateStableswapPool, PoolParams as StableSwapPoolParams},
    },
    v1beta1::{PoolAsset, PoolParams},
};
use osmosis_test_tube::{Account, Gamm, Module, Runner, SigningAccount};
use prop::collection::vec;
use proptest::prelude::*;
use proptest::strategy::{Just, Strategy};
use proptest::{option, prop_compose, proptest};

use crate::const_coin::ConstCoin;

const MAX_SCALE_FACTOR: u64 = 0x7FFF_FFFF_FFFF_FFFF; // 2^63 - 1
const MAX_POOL_WEIGHT: u64 = 1048575; //2^20 - 1

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OsmosisPoolType {
    Basic,
    Balancer { pool_weights: Vec<u64> },
    StableSwap { scaling_factors: Vec<u64> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstOsmosisTestPool {
    pub liquidity: &'static [ConstCoin],
    pub pool_type: OsmosisPoolType,
}

impl ConstOsmosisTestPool {
    pub const fn new(liquidity: &'static [ConstCoin], pool_type: OsmosisPoolType) -> Self {
        Self {
            liquidity,
            pool_type,
        }
    }
}

impl From<ConstOsmosisTestPool> for OsmosisTestPool {
    fn from(pool: ConstOsmosisTestPool) -> Self {
        Self {
            liquidity: pool.liquidity.into_elementwise(),
            pool_type: pool.pool_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsmosisTestPool {
    pub liquidity: Vec<Coin>,
    pub pool_type: OsmosisPoolType,
}

impl OsmosisTestPool {
    /// Create a new Osmosis pool with the given initial liquidity and pool type.
    pub fn new(liquidity: Vec<Coin>, pool_type: OsmosisPoolType) -> Self {
        Self {
            liquidity,
            pool_type,
        }
    }

    /// Create an Osmosis pool with the given initial liquidity.
    ///
    /// Returns the `u64` pool ID.
    pub fn create<'a, R: Runner<'a>>(&self, runner: &'a R, signer: &SigningAccount) -> u64 {
        let gamm = Gamm::new(runner);
        match &self.pool_type {
            OsmosisPoolType::Basic => {
                gamm.create_basic_pool(&self.liquidity, signer)
                    .unwrap()
                    .data
                    .pool_id
            }
            OsmosisPoolType::Balancer { pool_weights } => {
                gamm.create_balancer_pool(
                    MsgCreateBalancerPool {
                        sender: signer.address(),
                        pool_params: Some(PoolParams {
                            swap_fee: "10000000000000000".to_string(),
                            exit_fee: "10000000000000000".to_string(),
                            smooth_weight_change_params: None,
                        }),
                        pool_assets: self
                            .liquidity
                            .iter()
                            .zip(pool_weights.iter())
                            .map(|(c, weight)| PoolAsset {
                                token: Some(c.clone().into()),
                                weight: weight.to_string(),
                            })
                            .collect(),
                        future_pool_governor: "".to_string(),
                    },
                    signer,
                )
                .unwrap()
                .data
                .pool_id
            }
            OsmosisPoolType::StableSwap { scaling_factors } => {
                gamm.create_stable_swap_pool(
                    MsgCreateStableswapPool {
                        sender: signer.address(),
                        pool_params: Some(StableSwapPoolParams {
                            swap_fee: "10000000000000000".to_string(),
                            exit_fee: "10000000000000000".to_string(),
                        }),
                        initial_pool_liquidity: self
                            .liquidity
                            .iter()
                            .map(|c| c.clone().into())
                            .collect(),
                        scaling_factors: scaling_factors.clone(),
                        future_pool_governor: "".to_string(),
                        scaling_factor_controller: "".to_string(),
                    },
                    signer,
                )
                .unwrap()
                .data
                .pool_id
            }
        }
    }
}

/// Generates a vector of random denoms of the specified size
pub fn pool_denoms(count: usize) -> impl Strategy<Value = Vec<String>> {
    Just(Vec::from_iter(0..8))
        .prop_shuffle()
        .prop_flat_map(move |x| {
            let mut denoms = x
                .iter()
                .take(count)
                .map(|i| format!("denom{}", i))
                .collect::<Vec<String>>();
            denoms.sort(); // Osmosis requires denoms to be sorted...
            Just(denoms)
        })
}

/// Generates a vector of random denoms with at least one denom in common with
/// the given denoms.
pub fn pool_denoms_with_one_common(
    count: usize,
    input_denoms: Vec<String>,
) -> impl Strategy<Value = Vec<String>> {
    pool_denoms(count).prop_flat_map(move |mut denoms| {
        if denoms.iter().any(|x| input_denoms.contains(x)) {
            Just(denoms)
        } else {
            let common_denom = input_denoms[0].clone();
            denoms[0] = common_denom;
            denoms.sort(); // Osmosis requires denoms to be sorted...
            Just(denoms)
        }
    })
}

/// Generates a vector of random denoms with one of them being the given
/// specific_denom
pub fn pool_denoms_with_one_specific(
    count: usize,
    specific_denom: String,
) -> impl Strategy<Value = Vec<String>> {
    pool_denoms(count).prop_flat_map(move |mut denoms| {
        if denoms.iter().any(|x| x == &specific_denom) {
            Just(denoms)
        } else {
            denoms[0] = specific_denom.clone();
            denoms.sort(); // Osmosis requires denoms to be sorted...
            Just(denoms)
        }
    })
}

/// Generates a vector of size 2..8 with random amounts in the given range,
/// or 0..u128::MAX if no range is given.
pub fn pool_liquidity_amounts(
    liquidity_range: Option<Range<u128>>,
) -> impl Strategy<Value = Vec<u128>> {
    let liquidity_range = liquidity_range.unwrap_or(0..u64::MAX as u128);
    vec(liquidity_range, 2..8)
}

prop_compose! {
    /// Generates a randomly sized vector (size 2-8) of Coins with random amounts and denoms
    pub fn pool_liquidity(liquidity_range: Option<Range<u128>>)(amounts in pool_liquidity_amounts(liquidity_range))(denoms in pool_denoms(amounts.len()), amounts in Just(amounts)) -> Vec<Coin> {
        amounts.into_iter().zip(denoms).map(|(amount, denom)| Coin { amount: Uint128::from(amount), denom }).collect()
    }
}

prop_compose! {
    /// Generates a randomly sized vector (size 2-8) of Coins with random amounts
    /// and denoms, where one denom is in common with the given base_liquidity
    pub fn pool_liquidity_with_one_common_denom(base_liquidity: Vec<Coin>, liquidity_range: Option<Range<u128>>)
        (amounts in  pool_liquidity_amounts(liquidity_range))
        (
            denoms in pool_denoms_with_one_common(amounts.len(),
            base_liquidity.iter().map(|x| x.denom.clone()).collect()),
            amounts in Just(amounts)
        ) -> Vec<Coin> {
            amounts.into_iter().zip(denoms).map(|(amount, denom)|
                Coin { amount: Uint128::from(amount), denom
            }).collect()
        }
}

prop_compose! {
    /// Generates a randomly sized vector (size 2-8) of Coins with random amounts
    /// and denoms, where one denom is the given specific_denom
    pub fn pool_liquidity_with_one_specific_denom(specific_denom: String, liquidity_range: Option<Range<u128>>)
        (amounts in  pool_liquidity_amounts(liquidity_range))
        (
            denoms in pool_denoms_with_one_specific(amounts.len(), specific_denom.clone()),
            amounts in Just(amounts)
        ) -> Vec<Coin> {
            amounts.into_iter().zip(denoms).map(|(amount, denom)|
                Coin { amount: Uint128::from(amount), denom
            }).collect()
        }
}

/// Generates scaling factors for an Osmosis StableSwap pool for the given liquidity
pub fn scaling_factors(pool_liquidity: &[Coin]) -> impl Strategy<Value = Vec<u64>> {
    pool_liquidity
        .iter()
        .map(|x| {
            let max_scale_factor = if x.amount.u128() > MAX_SCALE_FACTOR.into() {
                MAX_SCALE_FACTOR
            } else {
                x.amount.u128() as u64
            };
            1..max_scale_factor
        })
        .collect::<Vec<Range<u64>>>()
}

prop_compose! {
    /// Generates a tuple of vectors with (pool_liquidity, scaling_factors)
    pub fn pool_params()(pool_liq in pool_liquidity(None))(scaling_factors in scaling_factors(&pool_liq), pool_liquidity in Just(pool_liq)) -> (Vec<Coin>,Vec<u64>) {
        (pool_liquidity, scaling_factors)
    }
}

/// Generates a random OsmosisPoolType
pub fn pool_type(pool_liquidity: &Vec<Coin>) -> impl Strategy<Value = OsmosisPoolType> {
    prop_oneof![
        Just(OsmosisPoolType::Basic),
        vec(1..MAX_POOL_WEIGHT, pool_liquidity.len())
            .prop_map(|pool_weights| { OsmosisPoolType::Balancer { pool_weights } }),
        scaling_factors(pool_liquidity)
            .prop_map(|scaling_factors| { OsmosisPoolType::StableSwap { scaling_factors } }),
    ]
    .no_shrink()
}

prop_compose! {
    /// Generates a random OsmosisTestPool with the given liquidity
    pub fn test_pool_from_liquidity(pool_liquidity: Vec<Coin>)(pool_type in pool_type(&pool_liquidity), liquidity in Just(pool_liquidity)) -> OsmosisTestPool {
        OsmosisTestPool {
            liquidity,
            pool_type,
        }
    }
}

prop_compose! {
    /// Generates a random OsmosisTestPool with 2..8 assets
    pub fn test_pool(liq_range: Option<Range<u128>>)(pool_liquidity in pool_liquidity(liq_range))(
        test_pool in test_pool_from_liquidity(pool_liquidity)
    ) -> OsmosisTestPool {
        test_pool
    }
}

prop_compose! {
    /// Generates a random OsmosisTestPool with one denom in common with the given base pool
    pub fn reward_pool(base_pool: OsmosisTestPool)(pool_liquidity in pool_liquidity_with_one_common_denom(base_pool.liquidity, None))(
        pool_type in pool_type(&pool_liquidity), liquidity in Just(pool_liquidity)
    ) -> OsmosisTestPool {
        OsmosisTestPool {
            liquidity,
            pool_type,
        }
    }
}

prop_compose! {
    /// Generates a random OsmosisTestPool with one denom being the given specific denom
    pub fn pool_with_denom(specific_denom: String, liq_range: Option<Range<u128>>)(pool_liquidity in pool_liquidity_with_one_specific_denom(specific_denom, liq_range))(
        pool_type in pool_type(&pool_liquidity), liquidity in Just(pool_liquidity)
    ) -> OsmosisTestPool {
        OsmosisTestPool {
            liquidity,
            pool_type,
        }
    }
}

prop_compose! {
    /// Generates a tuple of OsmosisTestPools with the given base pool and one or two reward pools
    /// with one denom in common with the base pool
    pub fn test_pools(liq_range: Option<Range<u128>>)
    ((liquidation_target, base_pool) in test_pool(liq_range.clone()).prop_flat_map(|pool| {
        (Just(pool.liquidity[0].denom.clone()),Just(pool))
    }))
    (
        reward1_pool in pool_with_denom(liquidation_target.clone(), liq_range.clone()),
        reward2_pool in option::of(pool_with_denom(liquidation_target, liq_range.clone())),
        base_pool in Just(base_pool)
    ) -> (OsmosisTestPool, OsmosisTestPool, Option<OsmosisTestPool>) {
        (base_pool, reward1_pool, reward2_pool)
    }
}

/// Asserts that all pool properties are valid.
#[cfg(test)]
fn assert_test_pool_properties(pool: OsmosisTestPool) {
    let OsmosisTestPool {
        liquidity,
        pool_type,
    } = pool;
    assert!(liquidity.len() >= 2);
    assert!(liquidity.len() <= 8);
    assert!(liquidity.iter().all(|liq| liq.amount.u128() > 0));
    assert!(liquidity.iter().all(|liq| liq.amount.u128() < u128::MAX));
    match pool_type {
        OsmosisPoolType::Basic => {}
        OsmosisPoolType::Balancer { pool_weights } => {
            assert_eq!(pool_weights.len(), liquidity.len());
            assert!(pool_weights.iter().all(|weight| weight > &0));
            assert!(pool_weights.iter().all(|weight| weight < &MAX_POOL_WEIGHT));
        }
        OsmosisPoolType::StableSwap { scaling_factors } => {
            assert_eq!(scaling_factors.len(), liquidity.len());
            assert!(scaling_factors.iter().all(|scale| scale > &0));
            assert!(scaling_factors
                .iter()
                .all(|scale| scale < &MAX_SCALE_FACTOR));
            assert!(liquidity
                .iter()
                .zip(scaling_factors.iter())
                .all(|(liq, scale)| (*scale as u128) < liq.amount.u128()));
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 10000,
        max_local_rejects: 0,
        ..ProptestConfig::default()
    })]

    #[test]
    fn test_pool_params(params in pool_params()) {
        let (pool_liquidity, scaling_factors): (Vec<Coin>,Vec<u64>) = params;
        assert_eq!(pool_liquidity.len(), scaling_factors.len());
        assert!(pool_liquidity.iter().all(|liq| liq.amount.u128() > 0));
        assert!(scaling_factors.iter().all(|scale| scale > &0));
        assert!(scaling_factors.iter().all(|scale| scale < &MAX_SCALE_FACTOR));
        assert!(pool_liquidity.iter().all(|liq| liq.amount.u128() < u128::MAX));
        assert!(pool_liquidity.iter().zip(scaling_factors.iter()).all(|(liq, scale)| (*scale as u128) < liq.amount.u128()));
    }

    #[test]
    fn test_denoms(mut denoms in pool_denoms(8)) {
        assert!(denoms.len() == 8);
        denoms.sort();
        assert_eq!(denoms, Vec::from_iter(0..8).iter().map(|i| format!("denom{}", i)).collect::<Vec<String>>());
    }

    #[test]
    fn test_denoms_one_common(denoms in vec(2..8usize,2).prop_flat_map(|counts| pool_denoms(counts[0]).prop_flat_map(move |base_denoms| {
        (Just(base_denoms.clone()), pool_denoms_with_one_common(counts[1], base_denoms))
    }))) {
        let (base_denoms, other_denoms) = denoms;
        assert!(base_denoms.iter().any(|denom| other_denoms.contains(denom)));
        if base_denoms.len() != other_denoms.len() {
            assert_ne!(base_denoms, other_denoms);
        }
    }

    #[test]
    fn test_denoms_one_specific(denoms in vec(2..8usize,2).prop_flat_map(|counts| pool_denoms(counts[0]).prop_flat_map(move |base_denoms| {
        (Just(base_denoms.clone()), pool_denoms_with_one_specific(counts[1], base_denoms[0].clone()))
    }))) {
        let (base_denoms, other_denoms) = denoms;
        let common_denom = &base_denoms[0];
        assert!(other_denoms.contains(common_denom));
    }

    #[test]
    fn test_test_pool(pool in test_pool(None)) {
        assert_test_pool_properties(pool);
    }

    #[test]
    fn test_pool_with_denom(pool in pool_with_denom(String::from("requested_denom"), None)) {
        assert!(pool.liquidity.iter().any(|liq| liq.denom == "requested_denom"));
        assert_test_pool_properties(pool);
    }

    #[test]
    fn test_reward_pool((pool, reward_pool) in test_pool(None).prop_flat_map(|base_pool| (Just(base_pool.clone()), reward_pool(base_pool)))) {
        let reward_denoms = reward_pool.liquidity.iter().map(|liq| liq.denom.clone()).collect::<Vec<String>>();
        let base_denoms = pool.liquidity.iter().map(|liq| liq.denom.clone()).collect::<Vec<String>>();
        // Assert that reward denoms has at least one denom in common with base denoms
        assert!(reward_denoms.iter().any(|denom| base_denoms.contains(denom)));

        assert_test_pool_properties(pool);
        assert_test_pool_properties(reward_pool);
    }
}
