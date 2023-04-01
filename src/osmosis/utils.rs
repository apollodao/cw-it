use cosmrs::Any;
use osmosis_test_tube::{osmosis_std::types::osmosis::lockup, OsmosisTestApp};
use prost::Message;

/// Returns true if the provided denom follows the format of an Osmosis LP token
pub fn is_osmosis_lp_token(denom: &str) -> bool {
    let parts = denom.split('/').collect::<Vec<_>>();
    parts.len() == 3 && parts[0] == "gamm" && parts[1] == "pool" && parts[2].parse::<u32>().is_ok()
}

pub fn set_chain_force_unlock_whitelisted_addresses(
    runner: &OsmosisTestApp,
    addresses: &[&str],
) -> () {
    let in_pset = lockup::Params {
        force_unlock_allowed_addresses: addresses.iter().map(|x| x.to_string()).collect(),
    };

    runner
        .set_param_set(
            "lockup",
            Any {
                type_url: lockup::Params::TYPE_URL.to_string(),
                value: in_pset.encode_to_vec(),
            },
        )
        .unwrap();
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
