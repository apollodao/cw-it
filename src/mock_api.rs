use cosmwasm_std::testing::{digit_sum, riffle_shuffle, MockApi};
use cosmwasm_std::{Addr, Api, CanonicalAddr, StdError, StdResult};

/// The standard [`MockApi`] implementation, does not support human readable addresses
/// longer than 54 characters. This is a workaround to allow for longer addresses.
///
/// Currently, the longest human readable address is 64 characters long. If changing this
/// the shuffles_encode and shuffles_decode variables will need to be updated such that
/// the riffle_shuffle restore property holds. See comments in [`MockApi`], specifically
/// [`cosmwasm_std::testing::CANONICAL_LENGTH`], and [`riffle_shuffle`] for more details.
pub struct OsmosisMockApi {
    api: MockApi,
    canonical_length: usize,
    shuffles_encode: usize,
    shuffles_decode: usize,
}

impl OsmosisMockApi {
    pub fn new() -> Self {
        Self {
            api: MockApi::default(),
            canonical_length: 64,
            shuffles_encode: 10,
            shuffles_decode: 2,
        }
    }
}

impl Default for OsmosisMockApi {
    fn default() -> Self {
        Self::new()
    }
}

impl Api for OsmosisMockApi {
    fn addr_validate(&self, human: &str) -> StdResult<Addr> {
        let canonical = self.addr_canonicalize(human)?;
        let normalized = self.addr_humanize(&canonical)?;
        if human != normalized {
            return Err(StdError::generic_err(
                "Invalid input: address not normalized",
            ));
        }

        Ok(Addr::unchecked(human))
    }

    fn addr_canonicalize(&self, input: &str) -> StdResult<CanonicalAddr> {
        // Dummy input validation. This is more sophisticated for formats like bech32, where format and checksum are validated.
        if input.len() < 3 {
            return Err(StdError::generic_err(
                "Invalid input: human address too short",
            ));
        }
        if input.len() > self.canonical_length {
            return Err(StdError::generic_err(
                "Invalid input: human address too long",
            ));
        }

        // mimicks formats like hex or bech32 where different casings are valid for one address
        let normalized = input.to_lowercase();

        let mut out = Vec::from(normalized);

        // pad to canonical length with NULL bytes
        out.resize(self.canonical_length, 0x00);
        // content-dependent rotate followed by shuffle to destroy
        // the most obvious structure (https://github.com/CosmWasm/cosmwasm/issues/552)
        let rotate_by = digit_sum(&out) % self.canonical_length;
        out.rotate_left(rotate_by);
        for _ in 0..self.shuffles_encode {
            out = riffle_shuffle(&out);
        }
        Ok(out.into())
    }

    fn addr_humanize(&self, canonical: &cosmwasm_std::CanonicalAddr) -> StdResult<Addr> {
        if canonical.len() != self.canonical_length {
            return Err(StdError::generic_err(
                "Invalid input: canonical address length not correct",
            ));
        }

        let mut tmp: Vec<u8> = canonical.clone().into();
        // Shuffle two more times which restored the original value (24 elements are back to original after 20 rounds)
        for _ in 0..self.shuffles_decode {
            tmp = riffle_shuffle(&tmp);
        }
        // Rotate back
        let rotate_by = digit_sum(&tmp) % self.canonical_length;
        tmp.rotate_right(rotate_by);
        // Remove NULL bytes (i.e. the padding)
        let trimmed = tmp.into_iter().filter(|&x| x != 0x00).collect();
        // decode UTF-8 bytes into string
        let human = String::from_utf8(trimmed)?;
        Ok(Addr::unchecked(human))
    }

    fn secp256k1_verify(
        &self,
        message_hash: &[u8],
        signature: &[u8],
        public_key: &[u8],
    ) -> Result<bool, cosmwasm_std::VerificationError> {
        self.api
            .secp256k1_verify(message_hash, signature, public_key)
    }

    fn secp256k1_recover_pubkey(
        &self,
        message_hash: &[u8],
        signature: &[u8],
        recovery_param: u8,
    ) -> Result<Vec<u8>, cosmwasm_std::RecoverPubkeyError> {
        self.api
            .secp256k1_recover_pubkey(message_hash, signature, recovery_param)
    }

    fn ed25519_verify(
        &self,
        message: &[u8],
        signature: &[u8],
        public_key: &[u8],
    ) -> Result<bool, cosmwasm_std::VerificationError> {
        self.api.ed25519_verify(message, signature, public_key)
    }

    fn ed25519_batch_verify(
        &self,
        messages: &[&[u8]],
        signatures: &[&[u8]],
        public_keys: &[&[u8]],
    ) -> Result<bool, cosmwasm_std::VerificationError> {
        self.api
            .ed25519_batch_verify(messages, signatures, public_keys)
    }

    fn debug(&self, message: &str) {
        self.api.debug(message)
    }
}
