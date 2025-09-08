# P2P Chat - DHT-Based Peer Discovery

A decentralized peer-to-peer chat application built with Rust and libp2p, featuring DHT (Distributed Hash Table) for peer discovery instead of mDNS.


## Quick Start

### Building the Application

```bash
# Clone the repository
git clone <repository-url>
cd p2p-chat

# Build in release mode
cargo build --release

# The binary will be available at ./target/release/chat-tui
```

### Running the Chat

#### Basic Usage
```bash
# Start with default settings
./target/release/chat-tui

# Start with custom username and port
./target/release/chat-tui --username "Alice" --port 8080
```

#### Custom Bootstrap Node
```bash
# Use a custom bootstrap node
./target/release/chat-tui --bootstrap "/ip4/192.168.1.100/tcp/4001"
```

### Configuration

The application supports the following command-line options:

- `--port, -p`: Local port to listen on (default: 0 - OS chooses)
- `--username, -u`: Your username in the chat (default: "Anonymous") 
- `--bootstrap, -b`: Bootstrap node address (default: uses built-in bootstrap node)

## How It Works

### 1. DHT Bootstrap Process

When the application starts:
1. **Keypair Generation**: Creates a unique Ed25519 keypair for the peer
2. **Transport Setup**: Establishes TCP transport with Noise encryption and Yamux multiplexing
3. **DHT Initialization**: Initializes Kademlia DHT with the configured bootstrap nodes
4. **Network Behaviors**: Combines Gossipsub, Kademlia, Identify, and Ping behaviors
5. **Bootstrap Connection**: Connects to bootstrap nodes to join the DHT network

### 2. Peer Discovery

- **DHT Queries**: Performs random peer ID queries to discover nearby peers
- **Identify Protocol**: Exchanges peer information and listening addresses
- **Address Storage**: Stores discovered peer addresses in the Kademlia routing table
- **Automatic Discovery**: Continuously discovers new peers as they join the network

### 3. Message Broadcasting

- **Topic Subscription**: All peers subscribe to the "chat" gossipsub topic
- **Message Propagation**: Messages are broadcast through the gossipsub mesh network
- **Redundant Delivery**: Multiple paths ensure message delivery even if some peers disconnect
- **Message Authentication**: All messages are cryptographically signed

## Network Configuration

### Default Bootstrap Node

The application uses your system's public IP (49.43.242.2) as the default bootstrap node at port 4001. This allows peers on your local network and potentially across the internet to discover each other.

### Custom Bootstrap Nodes

You can specify custom bootstrap nodes using multiaddr format:

```bash
# IPv4 with peer ID
--bootstrap "/ip4/192.168.1.100/tcp/4001/p2p/12D3KooWExample"

# IPv4 without peer ID (will attempt connection anyway)
--bootstrap "/ip4/192.168.1.100/tcp/4001"

# IPv6 example
--bootstrap "/ip6/::1/tcp/4001"
```

## Development

### Project Structure

```
p2p-chat/
├── crates/
│   ├── chat-core/          # Core P2P networking library
│   │   ├── src/
│   │   │   ├── lib.rs      # Library exports and client interface
│   │   │   ├── network.rs  # DHT and networking implementation
│   │   │   └── types.rs    # Core data structures
│   │   └── Cargo.toml
│   └── chat-tui/           # Terminal UI application
│       ├── src/
│       │   ├── main.rs     # Application entry point
│       │   └── app.rs      # Application state management
│       └── Cargo.toml
├── Cargo.toml              # Workspace configuration
└── README.md
```

### Key Dependencies

- **libp2p**: P2P networking framework with Kademlia DHT, Gossipsub, and transport protocols
- **tokio**: Async runtime for handling concurrent network operations
- **serde**: Serialization for message passing
- **tracing**: Structured logging for debugging and monitoring
- **clap**: Command-line argument parsing

### Adding New Features

The modular architecture makes it easy to extend:

1. **New Network Behaviors**: Add to `ChatBehaviour` in `network.rs`
2. **Message Types**: Extend `NetworkEvent` enum in `types.rs`
3. **UI Components**: Modify the TUI in `chat-tui/src/`
4. **Transport Protocols**: Add new transports in the network initialization

## Troubleshooting

### Common Issues

1. **Bootstrap Connection Failed**
   - Check if the bootstrap node is reachable
   - Verify firewall settings allow TCP connections
   - Try a different bootstrap node

2. **No Peers Discovered**
   - Ensure other peers are using the same bootstrap node
   - Check network connectivity
   - Wait a few moments for DHT propagation

3. **Messages Not Received**
   - Verify peers are connected (check connection events)
   - Ensure gossipsub topic subscription is working
   - Check for network partitions

### Debug Logging

Enable detailed logging with:

```bash
RUST_LOG=debug ./target/release/chat-tui
```


## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request



## Future Enhancements

- [ ] Web UI interface
- [ ] File sharing capabilities
- [ ] Private messaging between peers
- [ ] Message persistence and history
- [ ] Mobile app support
- [ ] Custom DHT routing strategies
- [ ] Network analytics and monitoring
