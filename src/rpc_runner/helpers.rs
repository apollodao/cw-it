pub fn mnemonic_to_signing_key(
    mnemonic: &str,
    path: &bip32::DerivationPath,
) -> Result<cosmrs::crypto::secp256k1::SigningKey, bip32::Error> {
    let seed = bip32::Mnemonic::new(mnemonic, bip32::Language::English)?.to_seed("");
    cosmrs::crypto::secp256k1::SigningKey::derive_from_path(seed, path)
}
