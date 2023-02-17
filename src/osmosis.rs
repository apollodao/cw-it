use cosmwasm_std::Coin;
use osmosis_std::types::osmosis::gamm::poolmodels::balancer::v1beta1::MsgCreateBalancerPool;
use osmosis_std::types::osmosis::gamm::poolmodels::stableswap::v1beta1::{
    MsgCreateStableswapPool, PoolParams as StableSwapPoolParams,
};
use osmosis_std::types::osmosis::gamm::v1beta1::{PoolAsset, PoolParams};
use osmosis_test_tube::{Account, Gamm, Module, Runner, SigningAccount};
use prop::collection::vec;
use proptest::prelude::{any_with, prop};
use proptest::prop_compose;
use proptest::strategy::{Just, Strategy};
use proptest_derive::Arbitrary;

const MAX_SCALE_FACTOR: u64 = 0x7FFF_FFFF_FFFF_FFFF; // 2^63 - 1
const MAX_POOL_WEIGHT: u64 = 1048575; //2^20 - 1

#[derive(Debug, Clone, PartialEq, Eq, Arbitrary)]
pub enum OsmosisPoolType {
    Basic,
    Balancer {
        #[proptest(strategy = "vec(1..MAX_POOL_WEIGHT, param_0.len())")]
        pool_weights: Vec<u64>,
    },
    StableSwap {
        #[proptest(params = "Vec<u64>")]
        #[proptest(value = "params.clone()")]
        scaling_factors: Vec<u64>,
    },
}

#[derive(Debug, Clone)]
pub struct OsmosisTestPool {
    pub assets: Vec<String>,
    pub pool_liquidity: Vec<u64>,
    pub pool_type: OsmosisPoolType,
}

prop_compose! {
    /// Generates a touple of vectors with (pool_liquidity, scaling_factors) of size 2..8
    pub fn pool_params()(pool_params in vec((1..u64::MAX, 1..MAX_SCALE_FACTOR), 2..8).prop_filter("scaling factors must be smaller than liquidity",|v| v.iter().all(|(liq, scale)| scale < liq))) -> (Vec<u64>,Vec<u64>) {
         let (pool_liquidity, scaling_factors): (Vec<u64>,Vec<u64>) = pool_params.into_iter().unzip();
            (pool_liquidity, scaling_factors)
    }
}

prop_compose! {
    /// Generates a random OsmosisPoolType with the given scaling factors
    pub fn pool_type(scaling_factors: Vec<u64>)(pool_type in any_with::<OsmosisPoolType>(scaling_factors)) -> OsmosisPoolType {
        pool_type
    }
}

prop_compose! {
    /// Generates a random OsmosisTestPool with 2..8 assets
    pub fn test_pool()(pool_params in pool_params())(pool_type in pool_type(pool_params.clone().1), pool_liquidity in Just(pool_params.0)) -> OsmosisTestPool {
        let mut assets = vec![];
        for i in 0..pool_liquidity.len() {
            assets.push(format!("denom{}", i));
        }
        OsmosisTestPool {
            assets,
            pool_liquidity,
            pool_type,
        }
    }
}

/// Create an Osmosis pool with the given initial liquidity.
pub fn create_osmosis_pool<'a, R: Runner<'a>>(
    runner: &'a R,
    pool_type: &OsmosisPoolType,
    initial_liquidity: &[Coin],
    signer: &SigningAccount,
) -> u64 {
    let gamm = Gamm::new(runner);
    match pool_type {
        OsmosisPoolType::Basic => {
            gamm.create_basic_pool(initial_liquidity, signer)
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
                    pool_assets: initial_liquidity
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
                    initial_pool_liquidity: initial_liquidity
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
