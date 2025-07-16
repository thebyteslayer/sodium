// Copyright (c) 2025, TheByteSlayer, Sodium
// A scalable and optimized Key Value Caching System, written in Rust.

use serde::{Deserialize, Serialize};
use std::fs;
use crate::configuration::{ConfigError, SodiumConfig};
use rand::Rng;

type ConfigResult<T> = Result<T, ConfigError>;

const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
const NODE_ID_LENGTH: usize = 7;

fn generate_node_id() -> String {
    let mut rng = rand::thread_rng();
    (0..NODE_ID_LENGTH)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClusterNode {
    pub node_id: String,
    pub node_validation: u32,
    pub address: String,
    pub slots: [u32; 2],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClusterConfig {
    pub cluster_validation: u32,
    pub nodes: Vec<ClusterNode>,
}

pub fn generate_cluster_file(config: &SodiumConfig) -> ConfigResult<()> {
    let cluster_node = ClusterNode {
        node_id: generate_node_id(),
        node_validation: 0,
        address: config.bind_address(),
        slots: [0, 16383],
    };

    let cluster_config = ClusterConfig {
        cluster_validation: 0,
        nodes: vec![cluster_node],
    };

    let content = serde_json::to_string_pretty(&cluster_config)?;
    fs::write("cluster.json", content)?;
    Ok(())
} 