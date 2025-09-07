//! Core types for the P2P chat application

use libp2p::Multiaddr;
use serde::{Deserialize, Serialize};

/// A chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub sender: String,
    pub content: String,
    pub timestamp: u64,
}

/// User information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub public_key: Vec<u8>,
}

/// DHT Configuration
#[derive(Debug, Clone)]
pub struct DhtConfig {
    pub bootstrap_nodes: Vec<Multiaddr>,
    pub local_port: u16,
}

impl Default for DhtConfig {
    fn default() -> Self {
        // Using the public IP we retrieved: 49.43.242.2
        let bootstrap_addr = "/ip4/49.43.242.2/tcp/4001/p2p/12D3KooWBhv1RbRv26TNXM9sd99J3HAKM5ww2dET3EEaRMQ9HQeE"
            .parse()
            .expect("Invalid bootstrap address");
        
        Self {
            bootstrap_nodes: vec![bootstrap_addr],
            local_port: 0, // Let OS choose
        }
    }
}

/// Peer information for DHT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: String,
    pub addresses: Vec<String>,
    pub last_seen: u64,
}

/// Network events that can occur
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    PeerDiscovered(PeerInfo),
    PeerConnected(String),
    PeerDisconnected(String),
    MessageReceived(ChatMessage),
    DhtBootstrapped,
}
