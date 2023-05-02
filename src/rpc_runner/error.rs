use thiserror::Error;

#[derive(Debug, Error)]
pub enum RpcRunnerError {
    #[error("{0}")]
    ChainError(#[from] super::chain::ChainError),

    #[error("Config error: {0}")]
    ConfigError(String),

    #[error("{0}")]
    Generic(String),
}
