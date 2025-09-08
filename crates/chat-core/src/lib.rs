//! Chat Core Library
//! 
//! This crate provides the core functionality for the P2P chat application,
//! including protocols, types, storage, and crypto utilities.

pub mod network;
pub mod types;

pub use network::*;
pub use types::*;

use tokio::sync::mpsc;

/// Initialize the chat core library
pub fn init() -> anyhow::Result<()> {
    tracing::info!("Chat core library initialized");
    Ok(())
}

/// Chat client handle for applications to interact with
pub struct ChatClient {
    command_sender: mpsc::UnboundedSender<ChatCommand>,
}

/// Commands that can be sent to the chat network
#[derive(Debug, Clone)]
pub enum ChatCommand {
    SendBroadcast(String),
    SendDirect { peer_id: String, message: String },
    ListPeers,
    GetPeerList,
}

impl ChatClient {
    /// Send a broadcast message to all peers
    pub fn send_broadcast(&self, message: String) -> anyhow::Result<()> {
        self.command_sender.send(ChatCommand::SendBroadcast(message))?;
        Ok(())
    }
    
    /// Send a direct message to a specific peer
    pub fn send_direct(&self, peer_id: String, message: String) -> anyhow::Result<()> {
        self.command_sender.send(ChatCommand::SendDirect { peer_id, message })?;
        Ok(())
    }
    
    /// Request the list of connected peers
    pub fn list_peers(&self) -> anyhow::Result<()> {
        self.command_sender.send(ChatCommand::ListPeers)?;
        Ok(())
    }
}

/// Initialize chat core with DHT networking and return a client handle
pub async fn start_chat_client(
    config: NetworkConfig, 
    username: String
) -> anyhow::Result<(ChatClient, mpsc::UnboundedReceiver<NetworkEvent>)> {
    tracing::info!("Starting chat client with DHT networking");
    
    // Create command channel
    let (command_sender, command_receiver) = mpsc::unbounded_channel();
    
    // Initialize network
    let (network, event_receiver) = init_network_with_dht(config).await?;
    
    // Start the network task
    tokio::spawn(async move {
        if let Err(e) = run_chat_network(network, command_receiver, username).await {
            tracing::error!("Chat network error: {}", e);
        }
    });
    
    let client = ChatClient { command_sender };
    Ok((client, event_receiver))
}

/// Internal function to run the chat network
async fn run_chat_network(
    mut network: P2pNetwork,
    mut command_receiver: mpsc::UnboundedReceiver<ChatCommand>,
    username: String,
) -> anyhow::Result<()> {
    use futures::stream::StreamExt;
    
    // Subscribe to chat messages
    network.subscribe_to_chat()?;
    network.start_peer_discovery();
    
    loop {
        tokio::select! {
            // Handle swarm events
            swarm_event = network.swarm.select_next_some() => {
                network.handle_swarm_event(swarm_event).await;
            }
            
            // Handle commands from the client
            command = command_receiver.recv() => {
                match command {
                    Some(ChatCommand::SendBroadcast(content)) => {
                        let message = ChatMessage {
                            id: uuid::Uuid::new_v4().to_string(),
                            sender: username.clone(),
                            content,
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                            message_type: MessageType::Broadcast,
                        };
                        let _ = network.publish_message(&message);
                    }
                    Some(ChatCommand::SendDirect { peer_id, message: content }) => {
                        let message = ChatMessage {
                            id: uuid::Uuid::new_v4().to_string(),
                            sender: username.clone(),
                            content,
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                            message_type: MessageType::Direct { target_peer_id: peer_id },
                        };
                        let _ = network.publish_message(&message);
                    }
                    Some(ChatCommand::ListPeers) => {
                        let peers = network.get_peer_list();
                        let _ = network.event_sender.send(NetworkEvent::PeerListUpdated(peers));
                    }
                    Some(ChatCommand::GetPeerList) => {
                        let peers = network.get_peer_list();
                        let _ = network.event_sender.send(NetworkEvent::PeerListUpdated(peers));
                    }
                    None => break,
                }
            }
        }
    }
    
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
