use anyhow::Result;
use clap::Parser;
use chat_core;

#[derive(Parser)]
#[command(name = "chat-tui")]
#[command(about = "A terminal UI for P2P chat")]
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
    
    tracing::info!("Starting chat TUI on port {}", args.port);
    
    // Initialize chat core
    chat_core::init()?;
    
    println!("🚀 Chat TUI started! (Hello world from chat-tui)");
    println!("This will be the terminal UI for P2P chat");
    
    Ok(())
}
