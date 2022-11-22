use anyhow::Error;

use std::future::Future;

use cosmos_sdk_proto::cosmos::tx::v1beta1::service_client::ServiceClient;
use cosmrs::rpc::{Client, HttpClient};
use serde::Deserialize;
use std::time::Duration;
use tokio::time;
use tonic::transport::Channel;

use config::Config;

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
    pub fn new(chain_cfg: ChainConfig) -> Self {
        // To run with docker-compose externally
        //let rpc_endpoint="http://localhost:26657".to_string();
        let http_client = HttpClient::new(chain_cfg.rpc_endpoint.as_str()).unwrap();

        let grpc_client: ServiceClient<Channel> =
            tokio_block(async { ServiceClient::connect(chain_cfg.grpc_endpoint.clone()).await })
                .unwrap();

        Self {
            http_client,
            grpc_client,
            chain_cfg,
        }
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

    pub fn current_heigth(&self) -> u64 {
        tokio_block(async {
            self.http_client
                .latest_block()
                .await
                .unwrap()
                .block
                .header
                .height
                .into()
        })
    }

    pub fn wait(&self, n_block: u64) {
        tokio_block(async {
            let _wait = self.poll_for_n_blocks(n_block, false).await;
        });
    }

    pub async fn poll_for_n_blocks(&self, n: u64, is_first_block: bool) -> Result<(), Error> {
        if is_first_block {
            self.client()
                .wait_until_healthy(Duration::from_secs(5))
                .await
                .unwrap();

            while let Err(e) = self.client().latest_block().await {
                if !matches!(e.detail(), cosmrs::rpc::error::ErrorDetail::Serde(_)) {
                    return Err(e.into());
                }
                time::sleep(Duration::from_millis(500)).await;
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
            time::sleep(Duration::from_millis(500)).await;

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

pub fn tokio_block<F: Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
}
