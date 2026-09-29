use alatus_ipc::BatteryProxy;
use clap::{Args, Parser, Subcommand};
use std::error::Error;
use zbus::Connection;

#[derive(Parser)]
#[command(
    name = "alatus",
    about = "Linux hardware control suite for ASUS laptops",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Battery controls and telemetry
    Battery(BatteryArgs),
}

#[derive(Args)]
struct BatteryArgs {
    #[command(subcommand)]
    action: BatteryAction,
}

#[derive(Subcommand)]
enum BatteryAction {
    /// Display current battery status, charge limit, and health
    Status,
    /// Set charging threshold limit percentage (e.g., 60, 80, 100)
    SetLimit {
        #[arg(help = "Threshold limit percentage (60, 80, 100)")]
        limit: u8,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let conn = Connection::system().await.map_err(|e| {
        format!("Failed to connect to system D-Bus. Is alatusd daemon running? Error: {e}")
    })?;

    match cli.command {
        Commands::Battery(args) => handle_battery(&conn, args.action).await?,
    }

    Ok(())
}

async fn handle_battery(conn: &Connection, action: BatteryAction) -> Result<(), Box<dyn Error>> {
    let proxy = BatteryProxy::new(conn).await?;

    match action {
        BatteryAction::Status => {
            let info = proxy.get_info().await?;
            println!("--- ASUS Battery Telemetry ---");
            println!("  State of Charge : {}%", info.percentage);
            println!("  Status          : {}", info.status);
            if let Some(limit) = info.charge_limit {
                println!("  Charge Limit    : {}%", limit);
            } else {
                println!("  Charge Limit    : Not set / Unsupported");
            }
            if let Some(health) = info.health_percentage {
                println!("  Battery Health  : {}%", health);
            }
            if let Some(microwatts) = info.power_now_microwatts {
                let watts = microwatts as f64 / 1_000_000.0;
                println!("  Discharge/Rate  : {:.2} W", watts);
            }
        }
        BatteryAction::SetLimit { limit } => {
            println!("Setting battery charge limit to {}%...", limit);
            proxy.set_charge_limit(limit).await?;
            println!("✓ Successfully updated battery charge limit to {}%.", limit);
        }
    }

    Ok(())
}
