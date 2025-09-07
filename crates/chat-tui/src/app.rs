//! Application state and message handling

use anyhow::Result;
use chat_core::{ChatMessage, NetworkEvent, P2pNetwork};
use tokio::sync::mpsc;
use tracing::{error, info};
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, BufReader};

/// Application command from user input
#[derive(Debug, Clone)]
pub enum AppCommand {
    SendMessage(String),
    Quit,
}

/// Main application state
pub struct ChatApp {
    pub username: String,
    pub network_sender: mpsc::UnboundedSender<AppCommand>,
}

impl ChatApp {
    pub fn new(username: String) -> (Self, mpsc::UnboundedReceiver<AppCommand>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        
        let app = ChatApp {
            username,
            network_sender: sender,
        };
        
        (app, receiver)
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
                let _ = self.network_sender.send(AppCommand::Quit);
                break;
            }
            
            if !trimmed.is_empty() {
                let _ = self.network_sender.send(AppCommand::SendMessage(trimmed.to_string()));
            }
            
            print!("> ");
            io::stdout().flush()?;
        }
        
        Ok(())
    }
}

/// Network task that handles both network events and app commands
pub async fn run_network_task(
    mut network: P2pNetwork,
    mut event_receiver: mpsc::UnboundedReceiver<NetworkEvent>,
    mut command_receiver: mpsc::UnboundedReceiver<AppCommand>,
    username: String,
) {
    // Start peer discovery
    network.start_peer_discovery();
    
    // Start network event loop in a separate task
    let mut network_clone = network;
    let network_handle = tokio::spawn(async move {
        if let Err(e) = network_clone.run().await {
            error!("Network error: {}", e);
        }
    });
    
    // Handle events and commands
    loop {
        tokio::select! {
            // Handle network events
            event = event_receiver.recv() => {
                match event {
                    Some(NetworkEvent::PeerDiscovered(peer_info)) => {
                        println!("🔍 Discovered peer: {} with {} addresses", 
                            peer_info.peer_id, peer_info.addresses.len());
                        print!("> ");
                        io::stdout().flush().unwrap();
                    }
                    Some(NetworkEvent::PeerConnected(peer_id)) => {
                        println!("✅ Connected to peer: {}", peer_id);
                        print!("> ");
                        io::stdout().flush().unwrap();
                    }
                    Some(NetworkEvent::PeerDisconnected(peer_id)) => {
                        println!("❌ Disconnected from peer: {}", peer_id);
                        print!("> ");
                        io::stdout().flush().unwrap();
                    }
                    Some(NetworkEvent::MessageReceived(message)) => {
                        println!("💬 {}: {}", message.sender, message.content);
                        print!("> ");
                        io::stdout().flush().unwrap();
                    }
                    Some(NetworkEvent::DhtBootstrapped) => {
                        println!("🌐 DHT bootstrap successful! You can now discover and connect to peers.");
                        print!("> ");
                        io::stdout().flush().unwrap();
                    }
                    None => break,
                }
            }
            
            // Handle app commands
            command = command_receiver.recv() => {
                match command {
                    Some(AppCommand::SendMessage(content)) => {
                        let _message = ChatMessage {
                            id: uuid::Uuid::new_v4().to_string(),
                            sender: username.clone(),
                            content: content.clone(),
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        };
                        
                        // This would need access to the network, which is tricky with the current structure
                        // For now, we'll just show the message locally
                        println!("📤 You: {}", content);
                        print!("> ");
                        io::stdout().flush().unwrap();
                        
                        // TODO: Actually send the message through the network
                        // This requires refactoring the network structure
                    }
                    Some(AppCommand::Quit) => {
                        info!("Received quit command");
                        break;
                    }
                    None => break,
                }
            }
        }
    }
    
    // Clean up
    network_handle.abort();
}
