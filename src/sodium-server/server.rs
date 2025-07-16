// Copyright (c) 2025, TheByteSlayer, Sodium
// A scalable and optimized Key Value Caching System, written in Rust.

mod api;
mod core;
mod cluster;
mod configuration;
mod search;
mod threading;

use api::TcpApiServer;
use configuration::SodiumConfig;

use tracing::{info, error};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = SodiumConfig::load_or_create()?;
    
    if !config.silent {
        tracing_subscriber::fmt()
            .with_target(false)
            .with_thread_ids(true)
            .with_level(true)
            .init();
    }

    threading::initialize_threading();
    core::initialize_cache();
    
    let bind_addr = config.bind_address();
    
    let server = TcpApiServer::new(&bind_addr).await?;
    
    if !config.silent {
        info!("Sodium running on {}", server.local_addr()?);
        info!("Sodium listening on {}", config.public_bind_address());
    }
    
    tokio::select! {
        result = server.run() => {
            if let Err(e) = result {
                if !config.silent {
                    error!("Error accepting TCP connection: {}", e);
                }
            }
        }
        _ = tokio::signal::ctrl_c() => {
        }
    }

    Ok(())
} 