mod app;

use anyhow::Result;
use app::{handle_network_events, ChatApp};
use chat_core::{start_chat_client, NetworkConfig};
use clap::Parser;
use tracing::{info, warn};

#[derive(Parser)]
#[command(name = "chat-tui")]
#[command(about = "A P2P chat application using DHT for peer discovery")]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value = "0")]
    port: u16,
    
    /// Your username for the chat
    #[arg(short, long, default_value = "Anonymous")]
    username: String,
    
    /// Bootstrap node address (optional, uses default if not provided)
    #[arg(short, long)]
    bootstrap: Option<String>,
    
    /// Path to the peer keypair file (default: peer_key.dat)
    #[arg(short, long, default_value = "peer_key.dat")]
    key_file: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    let args = Args::parse();
    
    info!("Starting P2P Chat TUI");
    info!("Username: {}", args.username);
    info!("Port: {}", args.port);
    
    // Create network configuration
    let mut config = NetworkConfig::default();
    config.listen_port = args.port;
    config.key_file = args.key_file;
    
    // Override bootstrap node if provided
    if let Some(bootstrap_addr) = args.bootstrap {
        match bootstrap_addr.parse() {
            Ok(addr) => {
                config.dht_config.bootstrap_nodes = vec![addr];
                info!("Using custom bootstrap node: {}", bootstrap_addr);
            }
            Err(e) => {
                warn!("Invalid bootstrap address: {}. Using default.", e);
            }
        }
    }
    
    // Start chat client with DHT
    let (client, event_receiver) = start_chat_client(config, args.username.clone()).await?;
    
    println!("🚀 P2P Chat started!");
    println!("Username: {}", args.username);
    println!("Connecting to DHT and discovering peers...");
    println!();
    println!("📖 Commands:");
    println!("  • Type messages to broadcast to all peers");
    println!("  • /peers or /list - Show connected peers");
    println!("  • /dm <peer_id> <message> - Send direct message");
    println!("  • quit or exit - Exit the chat");
    println!();
    
    // Create chat app
    let app = ChatApp::new(client);
    
    // Start network event handler
    let event_handle = tokio::spawn(handle_network_events(event_receiver));
    
    // Handle user input
    println!("Chat is ready! Start typing messages:");
    let _ = app.handle_user_input().await;
    
    // Cancel event handler
    event_handle.abort();
    
    println!("Goodbye!");
    Ok(())
}
