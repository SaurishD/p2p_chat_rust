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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        assert!(init().is_ok());
    }
}
