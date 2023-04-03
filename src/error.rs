use thiserror::Error;

#[derive(Debug, Error)]
pub enum CwItError {
    #[error("{0}")]
    ArtifactError(#[from] crate::artifact::ArtifactError),
    #[error("{0}")]
    RunnerError(#[from] test_tube::RunnerError),
    #[error("{0}")]
    RpcError(#[from] cosmrs::rpc::error::Error),
    #[error("{0}")]
    AnyhowError(#[from] anyhow::Error),
}
