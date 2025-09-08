//! Application state and message handling

use anyhow::Result;
use chat_core::{ChatClient, MessageType, NetworkEvent};
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, BufReader};

/// Main application state
pub struct ChatApp {
    pub client: ChatClient,
}

impl ChatApp {
    pub fn new(client: ChatClient) -> Self {
        ChatApp { client }
    }

    /// Handle user input and send commands
    pub async fn handle_user_input(&self) -> Result<()> {
        let stdin = tokio::io::stdin();
        let mut lines = BufReader::new(stdin).lines();
        
        print!("> ");
        io::stdout().flush()?;
        
        while let Ok(Some(line)) = lines.next_line().await {
            let trimmed = line.trim();
            
            if trimmed == "quit" || trimmed == "exit" {
                break;
            }
            
            if trimmed == "/peers" || trimmed == "/list" {
                let _ = self.client.list_peers();
            } else if trimmed.starts_with("/dm ") {
                // Parse direct message: /dm <peer_id> <message>
                let parts: Vec<&str> = trimmed[4..].splitn(2, ' ').collect();
                if parts.len() == 2 {
                    let peer_id = parts[0].to_string();
                    let message = parts[1].to_string();
                    if let Err(e) = self.client.send_direct(peer_id.clone(), message.clone()) {
                        println!("❌ Failed to send direct message: {}", e);
                    } else {
                        println!("📤 You → {}: {}", &peer_id[..12.min(peer_id.len())], message);
                    }
                } else {
                    println!("Usage: /dm <peer_id> <message>");
                    println!("Example: /dm 12D3KooW... Hello there!");
                }
            } else if !trimmed.is_empty() && !trimmed.starts_with('/') {
                // Regular message - broadcast to all
                if let Err(e) = self.client.send_broadcast(trimmed.to_string()) {
                    println!("❌ Failed to send message: {}", e);
                } else {
                    println!("📤 You (broadcast): {}", trimmed);
                }
            } else if trimmed.starts_with('/') {
                println!("Unknown command. Available commands:");
                println!("  /peers or /list  - Show connected peers");
                println!("  /dm <peer_id> <message> - Send direct message");
                println!("  quit or exit - Exit the chat");
            }
            
            print!("> ");
            io::stdout().flush()?;
        }
        
        Ok(())
    }
}

/// Handle network events from the chat client
pub async fn handle_network_events(mut event_receiver: tokio::sync::mpsc::UnboundedReceiver<NetworkEvent>) {
    while let Some(event) = event_receiver.recv().await {
        match event {
            NetworkEvent::PeerDiscovered(peer_info) => {
                println!("🔍 Discovered peer: {} ({})", 
                    &peer_info.peer_id[..12.min(peer_info.peer_id.len())], peer_info.addresses.len());
                print!("> ");
                io::stdout().flush().unwrap();
            }
            NetworkEvent::PeerConnected(peer_id) => {
                println!("✅ Connected to peer: {}", &peer_id[..12.min(peer_id.len())]);
                print!("> ");
                io::stdout().flush().unwrap();
            }
            NetworkEvent::PeerDisconnected(peer_id) => {
                println!("❌ Disconnected from peer: {}", &peer_id[..12.min(peer_id.len())]);
                print!("> ");
                io::stdout().flush().unwrap();
            }
            NetworkEvent::MessageReceived(message) => {
                match message.message_type {
                    MessageType::Broadcast => {
                        println!("💬 {}: {}", message.sender, message.content);
                    }
                    MessageType::Direct { .. } => {
                        println!("📩 {} (DM): {}", message.sender, message.content);
                    }
                }
                print!("> ");
                io::stdout().flush().unwrap();
            }
            NetworkEvent::DhtBootstrapped => {
                println!("🌐 DHT bootstrap successful! You can now discover and connect to peers.");
                println!("Commands: /peers (list peers), /dm <peer_id> <message> (direct message)");
                print!("> ");
                io::stdout().flush().unwrap();
            }
            NetworkEvent::PeerListUpdated(peers) => {
                if peers.is_empty() {
                    println!("No peers connected yet.");
                } else {
                    println!("📋 Connected peers ({}):", peers.len());
                    for peer in peers {
                        println!("  • {} ({})", &peer.peer_id[..12.min(peer.peer_id.len())], peer.peer_id);
                    }
                }
                print!("> ");
                io::stdout().flush().unwrap();
            }
        }
    }
}
