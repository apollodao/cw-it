/// Returns true if the provided denom follows the format of an Osmosis LP token
pub fn is_osmosis_lp_token(denom: &str) -> bool {
    let parts = denom.split('/').collect::<Vec<_>>();
    parts.len() == 3 && parts[0] == "gamm" && parts[1] == "pool" && parts[2].parse::<u32>().is_ok()
}

#[cfg(test)]
mod tests {
    use crate::osmosis::utils::is_osmosis_lp_token;

    #[test]
    fn test_is_osmosis_lp_token() {
        // Success cases
        assert!(is_osmosis_lp_token("gamm/pool/1"));
        assert!(is_osmosis_lp_token("gamm/pool/12"));

        // Failure cases
        assert!(!is_osmosis_lp_token(""));
        assert!(!is_osmosis_lp_token("gam/pool/1"));
        assert!(!is_osmosis_lp_token("gamm/pol/1"));
        assert!(!is_osmosis_lp_token("gamm/pool/one"));
        assert!(!is_osmosis_lp_token("gamm/pol/1/2"));
    }
}
