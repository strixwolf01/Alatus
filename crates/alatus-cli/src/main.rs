use alatus_ipc::{BatteryProxy, LightingProxy, ThermalProxy};
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
    /// Keyboard backlight and RGB lighting controls
    Lighting(LightingArgs),
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

#[derive(Args)]
struct LightingArgs {
    #[command(subcommand)]
    action: LightingAction,
}

#[derive(Subcommand)]
enum LightingAction {
    /// Display current keyboard lighting state
    Status,
    /// Set backlight brightness level (0 = off, 1 = low, 2 = med, 3 = high)
    Brightness {
        #[arg(help = "Brightness level (0 - 3)")]
        level: u8,
    },
    /// Set static RGB color via hex code (e.g. #FF5500 or FF5500)
    SetColor {
        #[arg(help = "Hex color string (e.g. #00FFCC or FF0000)")]
        hex: String,
    },
    /// Set lighting animation mode (static, breathing, strobe, rainbow, off)
    Mode {
        #[arg(help = "Lighting mode: static, breathing, strobe, rainbow, off")]
        mode: String,
        #[arg(short, long, default_value_t = 1, help = "Animation speed (0 - 2)")]
        speed: u8,
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
        Commands::Lighting(args) => handle_lighting(&conn, args.action).await?,
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

async fn handle_lighting(conn: &Connection, action: LightingAction) -> Result<(), Box<dyn Error>> {
    let proxy = LightingProxy::new(conn).await?;

    match action {
        LightingAction::Status => {
            let state = proxy.get_state().await?;
            println!("--- ASUS Keyboard RGB Lighting ---");
            println!("  Mode       : {}", state.mode);
            println!("  Brightness : {} / 3", state.brightness);
            println!(
                "  Color      : RGB({}, {}, {}) [#{:02X}{:02X}{:02X}]",
                state.r, state.g, state.b, state.r, state.g, state.b
            );
            println!("  Speed      : {}", state.speed);
        }
        LightingAction::Brightness { level } => {
            let clamped = level.min(3);
            println!("Setting keyboard backlight brightness to {}...", clamped);
            proxy.set_brightness(clamped).await?;
            println!("✓ Successfully updated brightness to {}.", clamped);
        }
        LightingAction::SetColor { hex } => {
            let cleaned = hex.trim().trim_start_matches('#');
            if cleaned.len() != 6 {
                return Err("Hex color must be 6 hex characters (e.g. #FF5500 or FF5500)".into());
            }
            let r = u8::from_str_radix(&cleaned[0..2], 16)
                .map_err(|_| "Invalid red component in hex string")?;
            let g = u8::from_str_radix(&cleaned[2..4], 16)
                .map_err(|_| "Invalid green component in hex string")?;
            let b = u8::from_str_radix(&cleaned[4..6], 16)
                .map_err(|_| "Invalid blue component in hex string")?;

            println!(
                "Setting keyboard static color to RGB({}, {}, {})...",
                r, g, b
            );
            proxy.set_color(r, g, b).await?;
            println!("✓ Successfully updated keyboard color.");
        }
        LightingAction::Mode { mode, speed } => {
            println!(
                "Setting keyboard lighting mode to '{}' (speed {})...",
                mode, speed
            );
            proxy.set_mode(mode.clone(), speed).await?;
            println!("✓ Successfully set lighting mode to {}.", mode);
        }
    }

    Ok(())
}
