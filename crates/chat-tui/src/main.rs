mod app;

use anyhow::Result;
use app::{run_network_task, ChatApp};
use chat_core::{init_with_dht, NetworkConfig};
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
    
    // Initialize network with DHT
    let (network, event_receiver) = init_with_dht(config).await?;
    
    // Subscribe to chat messages
    // Note: We need to handle this in the network task since network is moved
    
    println!("🚀 P2P Chat started!");
    println!("Username: {}", args.username);
    println!("Connecting to DHT and discovering peers...");
    println!("Type your messages and press Enter to send.");
    println!("Type 'quit' to exit.\n");
    
    // Create chat app
    let (app, command_receiver) = ChatApp::new(args.username.clone());
    
    // Start network task
    let network_handle = tokio::spawn(run_network_task(
        network,
        event_receiver,
        command_receiver,
        args.username.clone(),
    ));
    
    // Handle user input
    println!("Chat is ready! Start typing messages:");
    let _ = app.handle_user_input().await;
    
    // Wait a bit for cleanup
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Cancel network task
    network_handle.abort();
    
    println!("Goodbye!");
    Ok(())
}
