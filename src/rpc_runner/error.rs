use thiserror::Error;

#[derive(Debug, Error)]
pub enum RpcRunnerError {
    #[error("{0}")]
    Bip32(#[from] bip32::Error),

    #[error("{0}")]
    ChainError(#[from] super::chain::ChainError),

    #[error("{0}")]
    Generic(String),
}
