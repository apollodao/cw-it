pub mod robot;
pub mod utils;

pub use astroport;

pub mod test_helpers {
    pub fn initial_coins() -> Vec<cosmwasm_std::Coin> {
        vec![
            cosmwasm_std::coin(u128::MAX, "uosmo"),
            cosmwasm_std::coin(u128::MAX, "uion"),
            cosmwasm_std::coin(u128::MAX, "uatom"),
            cosmwasm_std::coin(u128::MAX, "stake"),
        ]
    }
}
