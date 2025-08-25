//! Network-related functionality for P2P chat

/// Network configuration
pub struct NetworkConfig {
    pub listen_port: u16,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_port: 0, // Let the OS choose
        }
    }
}

/// Initialize network layer
pub fn init_network() -> anyhow::Result<()> {
    tracing::info!("Network layer initialized");
    Ok(())
}
