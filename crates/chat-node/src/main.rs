use anyhow::Result;
use clap::Parser;
use chat_core;

#[derive(Parser)]
#[command(name = "chat-node")]
#[command(about = "A headless P2P chat node")]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value = "0")]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    let args = Args::parse();
    
    tracing::info!("Starting chat node on port {}", args.port);
    
    // Initialize chat core
    chat_core::init()?;
    
    println!("🚀 Chat node started! (Hello world from chat-node)");
    println!("This will be the headless libp2p node");
    
    Ok(())
}
