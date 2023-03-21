use std::env;

use serde::Deserialize;
use testcontainers::{core::WaitFor, images::generic::GenericImage};
use thiserror::Error;

pub const DEFAULT_WAIT: u64 = 30;

#[derive(Clone, Debug, Deserialize)]
pub struct ContainerInfo {
    pub name: String,
    pub tag: String,
    pub volumes: Vec<(String, String)>,
    pub entrypoint: Option<String>,
    pub ports: Vec<u16>,
}

#[derive(Error, Debug)]
pub enum ContainerError {
    #[error("{0}")]
    IoError(#[from] std::io::Error),

    #[error("{0}")]
    Generic(String),
}

impl ContainerInfo {
    pub fn get_container_image(&self) -> Result<GenericImage, String> {
        let mut image = GenericImage::new(self.name.clone(), self.tag.clone())
            .with_wait_for(WaitFor::seconds(DEFAULT_WAIT));

        for port in self.ports.iter() {
            image = image.with_exposed_port(*port);
        }
        if let Some(entrypoint) = &self.entrypoint {
            image = image.with_entrypoint(entrypoint);
        }

        let dir_os_string = env::current_dir()
            .map(|d| d.into_os_string())
            .map_err(|_e| "Failed to get current directory".to_string())?;
        let working_dir = dir_os_string
            .into_string()
            .map_err(|_e| "Failed to convert OS string to string".to_string())?;

        for (from, dest) in &self.volumes {
            // TODO: Merge paths in better way? Should allow leading dot in `from`...
            let from = format!("{}/{}", working_dir, from);
            image = image.with_volume(from, dest);
        }

        Ok(image)
    }
}
