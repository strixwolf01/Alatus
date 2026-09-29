use alatus_ipc::{BatteryProxy, ThermalProxy};
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
    /// Thermal profile switching and fan telemetry
    Thermal(ThermalArgs),
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

#[derive(Args)]
struct ThermalArgs {
    #[command(subcommand)]
    action: ThermalAction,
}

#[derive(Subcommand)]
enum ThermalAction {
    /// Display current thermal profile, supported modes, and fan speeds
    Status,
    /// Set thermal profile mode (Quiet, Balanced, Performance, FullSpeed)
    Set {
        #[arg(help = "Profile mode: quiet, balanced, performance, fullspeed")]
        mode: String,
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
        Commands::Thermal(args) => handle_thermal(&conn, args.action).await?,
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

async fn handle_thermal(conn: &Connection, action: ThermalAction) -> Result<(), Box<dyn Error>> {
    let proxy = ThermalProxy::new(conn).await?;

    match action {
        ThermalAction::Status => {
            let current = proxy.get_current_profile().await?;
            let available = proxy.list_profiles().await?;
            let fans = proxy.get_fans().await?;

            println!("--- ASUS Thermal & Cooling Status ---");
            println!("  Current Profile    : {}", current);
            println!("  Available Profiles : {}", available.join(", "));

            if !fans.is_empty() {
                println!("\n  Fans Telemetry:");
                for fan in fans {
                    println!("    • {:<12} : {} RPM", fan.label, fan.current_rpm);
                }
            }
        }
        ThermalAction::Set { mode } => {
            println!("Switching thermal profile to '{}'...", mode);
            proxy.set_profile(mode.clone()).await?;
            println!("✓ Successfully updated thermal profile to {}.", mode);
        }
    }

    Ok(())
}
