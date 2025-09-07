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
use std::{
    collections::hash_map::DefaultHasher,
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
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_port: 0, // Let the OS choose
            dht_config: DhtConfig::default(),
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
}

impl P2pNetwork {
    /// Create a new P2P network instance
    pub async fn new(config: NetworkConfig) -> Result<(Self, mpsc::UnboundedReceiver<NetworkEvent>)> {
        // Create a random keypair
        let local_key = libp2p::identity::Keypair::generate_ed25519();
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
        };

        Ok((network, event_receiver))
    }

    /// Start the network event loop
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting P2P network event loop");

        // Start DHT bootstrap
        if let Err(e) = self.swarm.behaviour_mut().kademlia.bootstrap() {
            warn!("Failed to bootstrap DHT: {}", e);
        }

        loop {
            match self.swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!("Listening on {address}");
                }
                SwarmEvent::Behaviour(event) => {
                    self.handle_behaviour_event(event).await;
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    info!("Connected to peer: {peer_id}");
                    let _ = self.event_sender.send(NetworkEvent::PeerConnected(peer_id.to_string()));
                }
                SwarmEvent::ConnectionClosed { peer_id, .. } => {
                    info!("Disconnected from peer: {peer_id}");
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

                // Send peer discovered event
                let peer_info = PeerInfo {
                    peer_id: peer_id.to_string(),
                    addresses,
                    last_seen: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                };
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
        let topic = gossipsub::IdentTopic::new("chat");
        let data = serde_json::to_vec(message)?;
        
        if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
            warn!("Failed to publish message: {e}");
            return Err(anyhow::anyhow!("Failed to publish message: {e}"));
        }
        
        info!("Published message: {}", message.content);
        Ok(())
    }

    /// Subscribe to chat messages
    pub fn subscribe_to_chat(&mut self) -> Result<()> {
        let topic = gossipsub::IdentTopic::new("chat");
        self.swarm.behaviour_mut().gossipsub.subscribe(&topic)?;
        info!("Subscribed to chat topic");
        Ok(())
    }

    /// Get connected peers
    pub fn connected_peers(&self) -> Vec<PeerId> {
        self.swarm.connected_peers().cloned().collect()
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
