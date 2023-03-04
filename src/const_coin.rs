use cosmwasm_std::{Coin, Uint128};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstCoin {
    pub denom: &'static str,
    pub amount: Uint128,
}

impl ConstCoin {
    pub const fn new(amount: u128, denom: &'static str) -> Self {
        Self {
            denom,
            amount: Uint128::new(amount),
        }
    }
}

impl From<ConstCoin> for Coin {
    fn from(coin: ConstCoin) -> Self {
        (&coin).into()
    }
}

impl From<&ConstCoin> for Coin {
    fn from(coin: &ConstCoin) -> Self {
        Self {
            denom: coin.denom.to_string(),
            amount: coin.amount,
        }
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{Coin, Uint128};

    use crate::const_coin::ConstCoin;

    #[test]
    fn new() {
        let coin = ConstCoin::new(100_000_000_000_000_000u128, "uatom");
        assert_eq!(coin.denom, "uatom");
        assert_eq!(coin.amount, Uint128::new(100_000_000_000_000_000u128));
    }

    #[test]
    fn test_into_coin() {
        let const_coin = ConstCoin::new(100_000_000_000_000_000u128, "uatom");
        let coin: Coin = const_coin.into();
        assert_eq!(coin.denom, "uatom");
        assert_eq!(coin.amount, Uint128::new(100_000_000_000_000_000u128));
    }

    #[test]
    fn test_ref_into_coin() {
        let const_coin_ref = &ConstCoin::new(100_000_000_000_000_000u128, "uatom");
        let coin: Coin = const_coin_ref.into();
        assert_eq!(coin.denom, "uatom");
        assert_eq!(coin.amount, Uint128::new(100_000_000_000_000_000u128));
    }
}
