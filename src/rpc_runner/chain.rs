use std::time::Duration;

use cosmos_sdk_proto::cosmos::tx::v1beta1::service_client::ServiceClient;
use cosmrs::rpc::{Client, HttpClient};
// use futures_time::{task::sleep, time::Duration};
use serde::Deserialize;
use thiserror::Error;
use tonic::transport::Channel;

use cosmrs::rpc::error::Error as RpcError;

use config::Config;

use crate::helpers::block_on;

#[derive(Debug, Error)]
pub enum ChainError {
    #[error("{0}")]
    RpcError(#[from] RpcError),

    #[error("{0}")]
    TonicTransportError(#[from] tonic::transport::Error),
}

#[derive(Debug)]
pub struct Chain {
    http_client: HttpClient,
    grpc_client: ServiceClient<Channel>,
    chain_cfg: ChainConfig,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, Deserialize)]
pub struct ChainConfig {
    pub name: String,
    denom: String,
    prefix: String,
    pub chain_id: String,
    pub gas_price: u64,
    pub gas_adjustment: f64,
    pub derivation_path: String,
    pub rpc_endpoint: String,
    pub grpc_endpoint: String,
}

impl ChainConfig {
    pub fn from_yaml(file: &str) -> Self {
        let settings = Config::builder()
            .add_source(config::File::with_name(file))
            .build()
            .unwrap();
        settings.try_deserialize::<Self>().unwrap()
    }

    pub fn denom(&self) -> &str {
        &self.denom
    }

    pub fn prefix(&self) -> &str {
        &self.prefix
    }
}

#[allow(clippy::missing_const_for_fn)]
#[allow(clippy::similar_names)]
impl Chain {
    pub fn new(chain_cfg: ChainConfig) -> Result<Self, ChainError> {
        // To run with docker-compose externally
        //let rpc_endpoint="http://localhost:26657".to_string();
        let http_client = HttpClient::new(chain_cfg.rpc_endpoint.as_str())?;

        let grpc_client: ServiceClient<Channel> =
            block_on(ServiceClient::connect(chain_cfg.grpc_endpoint.clone()))?;

        Ok(Self {
            http_client,
            grpc_client,
            chain_cfg,
        })
    }

    pub fn client(&self) -> &HttpClient {
        &self.http_client
    }

    pub fn grpc_client(&self) -> &ServiceClient<Channel> {
        &self.grpc_client
    }

    pub fn chain_cfg(&self) -> &ChainConfig {
        &self.chain_cfg
    }

    pub fn current_height(&self) -> Result<u64, RpcError> {
        block_on(self.http_client.latest_block()).map(|res| res.block.header.height.into())
    }

    pub fn wait(&self, n_block: u64) -> Result<(), RpcError> {
        block_on(self.poll_for_n_blocks(n_block, false))
    }

    pub async fn poll_for_n_blocks(&self, n: u64, is_first_block: bool) -> Result<(), RpcError> {
        if is_first_block {
            self.client()
                .wait_until_healthy(Duration::from_secs(5))
                .await
                .unwrap();

            while let Err(e) = self.client().latest_block().await {
                if !matches!(e.detail(), cosmrs::rpc::error::ErrorDetail::Serde(_)) {
                    return Err(e);
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }

        let mut curr_height: u64 = self
            .client()
            .latest_block()
            .await
            .unwrap()
            .block
            .header
            .height
            .into();
        let target_height: u64 = curr_height + n;

        while curr_height < target_height {
            tokio::time::sleep(Duration::from_millis(500)).await;

            curr_height = self
                .client()
                .latest_block()
                .await
                .unwrap()
                .block
                .header
                .height
                .into();
        }

        Ok(())
    }
}
