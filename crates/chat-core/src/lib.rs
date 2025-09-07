//! Chat Core Library
//! 
//! This crate provides the core functionality for the P2P chat application,
//! including protocols, types, storage, and crypto utilities.

pub mod network;
pub mod types;

pub use network::*;
pub use types::*;

/// Initialize the chat core library
pub fn init() -> anyhow::Result<()> {
    tracing::info!("Chat core library initialized");
    Ok(())
}

/// Initialize chat core with DHT networking
pub async fn init_with_dht(config: NetworkConfig) -> anyhow::Result<(P2pNetwork, tokio::sync::mpsc::UnboundedReceiver<NetworkEvent>)> {
    tracing::info!("Initializing chat core with DHT networking");
    init_network_with_dht(config).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        assert!(init().is_ok());
    }
}
