use clap::Parser;

#[derive(Parser)]
#[command(name = "eltris")]
#[command(about = "Eltris Bitcoin Arbitrage Engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Parser)]
enum Commands {
    Start,
    Stop,
    Status,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Start => println!("Starting Eltris..."),
        Commands::Stop => println!("Stopping Eltris..."),
        Commands::Status => println!("Eltris status..."),
    }
} 