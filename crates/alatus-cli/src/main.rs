use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "alatus", about = "Linux hardware control suite for ASUS laptops")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Battery,
    Thermal,
    Lighting,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _cli = Cli::parse();
    println!("Alatus CLI v0.1.0");
    Ok(())
}
