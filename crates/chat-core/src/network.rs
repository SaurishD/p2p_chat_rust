//! Network-related functionality for P2P chat

use anyhow::Result;
use futures::stream::StreamExt;
use libp2p::{
    gossipsub::{self, MessageId, ValidationMode},
    identify,
    kad::{self, store::MemoryStore, Behaviour as KademliaBehaviour, Event as KademliaEvent},
    noise,
    ping::{self, Event as PingEvent},
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use std::fs;
use std::path::Path;
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    time::Duration,
};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::{types::*, DhtConfig, NetworkEvent};

/// Network configuration
pub struct NetworkConfig {
    pub listen_port: u16,
    pub dht_config: DhtConfig,
    pub key_file: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_port: 0, // Let the OS choose
            dht_config: DhtConfig::default(),
            key_file: "peer_key.dat".to_string(),
        }
    }
}

/// Combined network behavior for our P2P chat
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "ChatBehaviourEvent")]
pub struct ChatBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub kademlia: KademliaBehaviour<MemoryStore>,
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
}

#[derive(Debug)]
pub enum ChatBehaviourEvent {
    Gossipsub(gossipsub::Event),
    Kademlia(KademliaEvent),
    Identify(identify::Event),
    Ping(PingEvent),
}

impl From<gossipsub::Event> for ChatBehaviourEvent {
    fn from(event: gossipsub::Event) -> Self {
        ChatBehaviourEvent::Gossipsub(event)
    }
}

impl From<KademliaEvent> for ChatBehaviourEvent {
    fn from(event: KademliaEvent) -> Self {
        ChatBehaviourEvent::Kademlia(event)
    }
}

impl From<identify::Event> for ChatBehaviourEvent {
    fn from(event: identify::Event) -> Self {
        ChatBehaviourEvent::Identify(event)
    }
}

impl From<PingEvent> for ChatBehaviourEvent {
    fn from(event: PingEvent) -> Self {
        ChatBehaviourEvent::Ping(event)
    }
}

/// P2P Network manager
pub struct P2pNetwork {
    pub swarm: Swarm<ChatBehaviour>,
    pub event_sender: mpsc::UnboundedSender<NetworkEvent>,
    pub connected_peers: HashMap<PeerId, PeerInfo>,
}

impl P2pNetwork {
    /// Load or create a persistent keypair
    fn load_or_create_keypair(key_file: &str) -> Result<libp2p::identity::Keypair> {
        
        if Path::new(key_file).exists() {
            // Load existing keypair
            let key_bytes = fs::read(key_file)?;
            let keypair = libp2p::identity::Keypair::from_protobuf_encoding(&key_bytes)
                .map_err(|e| anyhow::anyhow!("Failed to decode keypair: {}", e))?;
            info!("Loaded existing keypair from {}", key_file);
            Ok(keypair)
        } else {
            // Create new keypair and save it
            let keypair = libp2p::identity::Keypair::generate_ed25519();
            let key_bytes = keypair.to_protobuf_encoding()
                .map_err(|e| anyhow::anyhow!("Failed to encode keypair: {}", e))?;
            fs::write(key_file, &key_bytes)?;
            info!("Created new keypair and saved to {}", key_file);
            Ok(keypair)
        }
    }

    /// Create a new P2P network instance
    pub async fn new(config: NetworkConfig) -> Result<(Self, mpsc::UnboundedReceiver<NetworkEvent>)> {
        // Load or create a persistent keypair
        let local_key = Self::load_or_create_keypair(&config.key_file)?;
        let local_peer_id = PeerId::from(local_key.public());
        info!("Local peer id: {local_peer_id}");

        // Create transport
        let transport = tcp::tokio::Transport::default()
            .upgrade(libp2p::core::upgrade::Version::V1Lazy)
            .authenticate(noise::Config::new(&local_key)?)
            .multiplex(yamux::Config::default())
            .boxed();

        // Create Gossipsub behavior
        let message_id_fn = |message: &gossipsub::Message| {
            let mut s = DefaultHasher::new();
            message.data.hash(&mut s);
            MessageId::from(s.finish().to_string())
        };

        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(ValidationMode::Strict)
            .message_id_fn(message_id_fn)
            .build()
            .expect("Valid config");

        let gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        ).expect("Valid gossipsub config");

        // Create Kademlia behavior
        let mut kademlia = KademliaBehaviour::new(local_peer_id, MemoryStore::new(local_peer_id));

        // Add bootstrap nodes to Kademlia
        for addr in &config.dht_config.bootstrap_nodes {
            if let Ok(peer_id) = addr.iter().find_map(|protocol| {
                if let libp2p::multiaddr::Protocol::P2p(peer_id) = protocol {
                    Some(peer_id)
                } else {
                    None
                }
            }).ok_or("No peer ID in bootstrap address") {
                kademlia.add_address(&peer_id, addr.clone());
                info!("Added bootstrap node: {} at {}", peer_id, addr);
            } else {
                // If no peer ID in multiaddr, we'll try to connect anyway
                warn!("Bootstrap address without peer ID: {}", addr);
            }
        }

        // Create Identify behavior
        let identify = identify::Behaviour::new(identify::Config::new(
            "/p2p-chat/1.0.0".to_string(),
            local_key.public(),
        ));

        // Create Ping behavior
        let ping = ping::Behaviour::new(ping::Config::new());

        // Combine behaviors
        let behaviour = ChatBehaviour {
            gossipsub,
            kademlia,
            identify,
            ping,
        };

        // Create swarm
        let mut swarm = Swarm::new(transport, behaviour, local_peer_id, libp2p::swarm::Config::with_tokio_executor());

        // Listen on all interfaces
        let listen_addr = format!("/ip4/0.0.0.0/tcp/{}", config.listen_port);
        swarm.listen_on(listen_addr.parse()?)?;

        // Create event channel
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        let network = P2pNetwork {
            swarm,
            event_sender,
            connected_peers: HashMap::new(),
        };

        Ok((network, event_receiver))
    }

    /// Handle a single swarm event
    pub async fn handle_swarm_event(&mut self, event: SwarmEvent<ChatBehaviourEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {address}");
            }
            SwarmEvent::Behaviour(event) => {
                self.handle_behaviour_event(event).await;
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                info!("Connected to peer: {peer_id}");
                
                // Add to connected peers if we have info about them
                if let Some(_peer_info) = self.connected_peers.get(&peer_id) {
                    let _ = self.event_sender.send(NetworkEvent::PeerConnected(peer_id.to_string()));
                } else {
                    // Create basic peer info for now
                    let peer_info = PeerInfo {
                        peer_id: peer_id.to_string(),
                        addresses: vec![],
                        last_seen: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    };
                    self.connected_peers.insert(peer_id, peer_info);
                    let _ = self.event_sender.send(NetworkEvent::PeerConnected(peer_id.to_string()));
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                info!("Disconnected from peer: {peer_id}");
                self.connected_peers.remove(&peer_id);
                let _ = self.event_sender.send(NetworkEvent::PeerDisconnected(peer_id.to_string()));
            }
            SwarmEvent::IncomingConnection { .. } => {
                debug!("Incoming connection");
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(peer_id) = peer_id {
                    warn!("Outgoing connection error to {peer_id}: {error}");
                } else {
                    warn!("Outgoing connection error: {error}");
                }
            }
            SwarmEvent::IncomingConnectionError { error, .. } => {
                warn!("Incoming connection error: {error}");
            }
            _ => {}
        }
    }

    /// Start the network event loop
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting P2P network event loop");

        // Start DHT bootstrap
        if let Err(e) = self.swarm.behaviour_mut().kademlia.bootstrap() {
            warn!("Failed to bootstrap DHT: {}", e);
        }

        loop {
            let event = self.swarm.select_next_some().await;
            self.handle_swarm_event(event).await;
        }
    }

    /// Handle behavior-specific events
    async fn handle_behaviour_event(&mut self, event: ChatBehaviourEvent) {
        match event {
            // Kademlia events
            ChatBehaviourEvent::Kademlia(KademliaEvent::OutboundQueryProgressed {
                result: kad::QueryResult::Bootstrap(Ok(kad::BootstrapOk { peer, .. })),
                ..
            }) => {
                info!("DHT bootstrap successful with peer: {peer}");
                let _ = self.event_sender.send(NetworkEvent::DhtBootstrapped);
            }
            ChatBehaviourEvent::Kademlia(KademliaEvent::OutboundQueryProgressed {
                result: kad::QueryResult::Bootstrap(Err(err)),
                ..
            }) => {
                warn!("DHT bootstrap failed: {err}");
            }
            ChatBehaviourEvent::Kademlia(KademliaEvent::RoutingUpdated { peer, .. }) => {
                debug!("DHT routing updated for peer: {peer}");
            }

            // Identify events
            ChatBehaviourEvent::Identify(identify::Event::Received { peer_id, info }) => {
                info!("Identified peer {peer_id}: {}", info.protocol_version);
                
                // Create address strings first
                let addresses: Vec<String> = info.listen_addrs.iter().map(|a| a.to_string()).collect();
                
                // Add peer to Kademlia
                for addr in &info.listen_addrs {
                    self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr.clone());
                    debug!("Added address for {peer_id}: {addr}");
                }

                // Send peer discovered event and store peer info
                let peer_info = PeerInfo {
                    peer_id: peer_id.to_string(),
                    addresses,
                    last_seen: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                };
                
                // Store peer info for later use
                self.connected_peers.insert(peer_id, peer_info.clone());
                let _ = self.event_sender.send(NetworkEvent::PeerDiscovered(peer_info));
            }

            // Ping events
            ChatBehaviourEvent::Ping(PingEvent { peer, result, .. }) => {
                match result {
                    Ok(rtt) => {
                        debug!("Ping to {peer}: {rtt:?}");
                    }
                    Err(err) => {
                        warn!("Ping to {peer} failed: {err}");
                    }
                }
            }

            // Gossipsub events
            ChatBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                propagation_source: _,
                message_id: _,
                message,
            }) => {
                if let Ok(chat_message) = serde_json::from_slice::<ChatMessage>(&message.data) {
                    info!("Received chat message from {}: {}", chat_message.sender, chat_message.content);
                    let _ = self.event_sender.send(NetworkEvent::MessageReceived(chat_message));
                }
            }

            _ => {}
        }
    }

    /// Connect to a specific peer
    pub fn connect_to_peer(&mut self, addr: Multiaddr) -> Result<()> {
        info!("Attempting to connect to peer at: {addr}");
        self.swarm.dial(addr)?;
        Ok(())
    }

    /// Publish a chat message
    pub fn publish_message(&mut self, message: &ChatMessage) -> Result<()> {
        match &message.message_type {
            MessageType::Broadcast => {
                // Send to all peers via gossipsub
                let topic = gossipsub::IdentTopic::new("chat");
                let data = serde_json::to_vec(message)?;
                
                if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
                    warn!("Failed to publish broadcast message: {e}");
                    return Err(anyhow::anyhow!("Failed to publish broadcast message: {e}"));
                }
                
                info!("Published broadcast message: {}", message.content);
            }
            MessageType::Direct { target_peer_id } => {
                // For direct messages, we'll use gossipsub with a specific topic for now
                // In a production system, you might want to use request-response protocol
                let topic = gossipsub::IdentTopic::new(&format!("direct-{}", target_peer_id));
                let data = serde_json::to_vec(message)?;
                
                if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
                    warn!("Failed to publish direct message: {e}");
                    return Err(anyhow::anyhow!("Failed to publish direct message: {e}"));
                }
                
                info!("Published direct message to {}: {}", target_peer_id, message.content);
            }
        }
        
        Ok(())
    }

    /// Subscribe to chat messages
    pub fn subscribe_to_chat(&mut self) -> Result<()> {
        // Subscribe to general chat topic for broadcasts
        let topic = gossipsub::IdentTopic::new("chat");
        self.swarm.behaviour_mut().gossipsub.subscribe(&topic)?;
        info!("Subscribed to chat topic");
        
        // Subscribe to direct messages for this peer
        let local_peer_id = *self.swarm.local_peer_id();
        let direct_topic = gossipsub::IdentTopic::new(&format!("direct-{}", local_peer_id));
        self.swarm.behaviour_mut().gossipsub.subscribe(&direct_topic)?;
        info!("Subscribed to direct message topic: direct-{}", local_peer_id);
        
    Ok(())
    }

    /// Get connected peers
    pub fn connected_peers(&self) -> Vec<PeerId> {
        self.swarm.connected_peers().cloned().collect()
    }
    
    /// Get peer list with information
    pub fn get_peer_list(&self) -> Vec<PeerInfo> {
        self.connected_peers.values().cloned().collect()
    }

    /// Start peer discovery in DHT
    pub fn start_peer_discovery(&mut self) {
        // Query for random peer IDs to discover peers
        let random_peer_id = PeerId::random();
        self.swarm.behaviour_mut().kademlia.get_closest_peers(random_peer_id);
        info!("Started peer discovery in DHT");
    }
}

/// Initialize network layer with DHT support
pub async fn init_network_with_dht(config: NetworkConfig) -> Result<(P2pNetwork, mpsc::UnboundedReceiver<NetworkEvent>)> {
    info!("Initializing network layer with DHT support");
    P2pNetwork::new(config).await
}
