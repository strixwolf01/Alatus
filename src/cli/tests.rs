// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use super::*;
use crate::services::firmware_mode::FirmwareMode;

#[test]
fn test_cli_parse_gui() {
    let cli_gui = Cli::try_parse_from(["alatus", "gui"]).unwrap();
    assert!(matches!(
        cli_gui.command,
        Some(Commands::Gui { minimized: false })
    ));

    let cli_gui_min = Cli::try_parse_from(["alatus", "gui", "-m"]).unwrap();
    assert!(matches!(
        cli_gui_min.command,
        Some(Commands::Gui { minimized: true })
    ));

    let cli_gui_tray = Cli::try_parse_from(["alatus", "gui", "--tray"]).unwrap();
    assert!(matches!(
        cli_gui_tray.command,
        Some(Commands::Gui { minimized: true })
    ));

    let cli_gui_flag = Cli::try_parse_from(["alatus", "--gui"]).unwrap();
    assert!(cli_gui_flag.gui);

    let cli_tray_flag = Cli::try_parse_from(["alatus", "--tray"]).unwrap();
    assert!(cli_tray_flag.tray);
}

#[test]
fn test_cli_parse_status() {
    let cli = Cli::try_parse_from(["alatus", "status"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Commands::Status {
            json: false,
            watch: false
        })
    ));

    let cli_json = Cli::try_parse_from(["alatus", "status", "--json"]).unwrap();
    assert!(matches!(
        cli_json.command,
        Some(Commands::Status {
            json: true,
            watch: false
        })
    ));

    let cli_watch = Cli::try_parse_from(["alatus", "status", "--watch"]).unwrap();
    assert!(matches!(
        cli_watch.command,
        Some(Commands::Status {
            json: false,
            watch: true
        })
    ));

    let cli_json_watch = Cli::try_parse_from(["alatus", "status", "--json", "--watch"]).unwrap();
    assert!(matches!(
        cli_json_watch.command,
        Some(Commands::Status {
            json: true,
            watch: true
        })
    ));
}

#[test]
fn test_cli_parse_capabilities() {
    let cli = Cli::try_parse_from(["alatus", "capabilities"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Commands::Capabilities { json: false })
    ));

    let cli_json = Cli::try_parse_from(["alatus", "capabilities", "--json"]).unwrap();
    assert!(matches!(
        cli_json.command,
        Some(Commands::Capabilities { json: true })
    ));
}

#[test]
fn test_cli_parse_monitor() {
    let cli = Cli::try_parse_from(["alatus", "monitor"]).unwrap();
    match cli.command {
        Some(Commands::Monitor { interval_ms }) => {
            assert_eq!(interval_ms, 1000);
        }
        _ => panic!("Expected monitor command"),
    }

    let cli_custom = Cli::try_parse_from(["alatus", "monitor", "-i", "500"]).unwrap();
    match cli_custom.command {
        Some(Commands::Monitor { interval_ms }) => {
            assert_eq!(interval_ms, 500);
        }
        _ => panic!("Expected monitor command with custom interval"),
    }
}

#[test]
fn test_cli_parse_no_args_defaults_to_status() {
    let cli = Cli::try_parse_from(["alatus"]).unwrap();
    assert!(cli.command.is_none());
}

#[test]
fn test_cli_parse_mode_get() {
    let cli = Cli::try_parse_from(["alatus", "mode", "get"]).unwrap();
    match cli.command {
        Some(Commands::Mode { action }) => {
            assert_eq!(action, Some(ModeAction::Get));
        }
        _ => panic!("Expected Commands::Mode"),
    }
}

#[test]
fn test_cli_parse_mode_cycle() {
    let cli = Cli::try_parse_from(["alatus", "mode", "cycle"]).unwrap();
    match cli.command {
        Some(Commands::Mode { action }) => {
            assert_eq!(action, Some(ModeAction::Cycle));
        }
        _ => panic!("Expected Commands::Mode Cycle"),
    }
}

#[test]
fn test_cli_parse_fan_cycle() {
    let cli = Cli::try_parse_from(["alatus", "fan", "cycle"]).unwrap();
    match cli.command {
        Some(Commands::Fan { action }) => {
            assert_eq!(action, Some(FanAction::Cycle));
        }
        _ => panic!("Expected Commands::Fan Cycle"),
    }
}

#[test]
fn test_cli_parse_mode_set_performance_and_high_alias() {
    let cli1 = Cli::try_parse_from(["alatus", "mode", "set", "performance"]).unwrap();
    match cli1.command {
        Some(Commands::Mode {
            action: Some(ModeAction::Set { name }),
        }) => {
            assert_eq!(name, ModeChoice::Performance);
            assert_eq!(FirmwareMode::from(name), FirmwareMode::High);
        }
        _ => panic!("Expected mode set performance"),
    }

    let cli2 = Cli::try_parse_from(["alatus", "mode", "set", "high"]).unwrap();
    match cli2.command {
        Some(Commands::Mode {
            action: Some(ModeAction::Set { name }),
        }) => {
            assert_eq!(name, ModeChoice::Performance);
        }
        _ => panic!("Expected high alias to parse as Performance"),
    }

    let cli3 = Cli::try_parse_from(["alatus", "mode", "set", "full"]).unwrap();
    match cli3.command {
        Some(Commands::Mode {
            action: Some(ModeAction::Set { name }),
        }) => {
            assert_eq!(name, ModeChoice::Full);
            assert_eq!(FirmwareMode::from(name), FirmwareMode::Full);
        }
        _ => panic!("Expected mode set full"),
    }
}

#[test]
fn test_cli_parse_fan_get_and_set() {
    let cli_get = Cli::try_parse_from(["alatus", "fan", "get"]).unwrap();
    match cli_get.command {
        Some(Commands::Fan {
            action: Some(FanAction::Get),
        }) => {}
        _ => panic!("Expected fan get"),
    }

    let cli_set = Cli::try_parse_from(["alatus", "fan", "set", "quiet"]).unwrap();
    match cli_set.command {
        Some(Commands::Fan {
            action: Some(FanAction::Set { profile }),
        }) => {
            assert_eq!(profile, ModeChoice::Quiet);
            assert_eq!(FirmwareMode::from(profile), FirmwareMode::Quiet);
        }
        _ => panic!("Expected fan set quiet"),
    }
}

#[test]
fn test_cli_parse_charge_limit() {
    let cli = Cli::try_parse_from(["alatus", "charge-limit", "80"]).unwrap();
    match cli.command {
        Some(Commands::ChargeLimit { percentage }) => {
            assert_eq!(percentage, 80);
        }
        _ => panic!("Expected charge-limit 80"),
    }

    let err = Cli::try_parse_from(["alatus", "charge-limit", "invalid"]);
    assert!(err.is_err());
}

#[test]
fn test_cli_parse_rgb_subcommands() {
    let cli_status = Cli::try_parse_from(["alatus", "rgb", "status"]).unwrap();
    assert!(matches!(
        cli_status.command,
        Some(Commands::Rgb {
            action: RgbAction::Status
        })
    ));

    let cli_color = Cli::try_parse_from(["alatus", "rgb", "set-color", "#FF00FF"]).unwrap();
    match cli_color.command {
        Some(Commands::Rgb {
            action: RgbAction::SetColor { color },
        }) => {
            assert_eq!(color, "#FF00FF");
        }
        _ => panic!("Expected rgb set-color"),
    }

    let cli_color_alias = Cli::try_parse_from(["alatus", "rgb", "color", "ff5500"]).unwrap();
    match cli_color_alias.command {
        Some(Commands::Rgb {
            action: RgbAction::SetColor { color },
        }) => {
            assert_eq!(color, "ff5500");
        }
        _ => panic!("Expected rgb color alias"),
    }

    let cli_bright = Cli::try_parse_from(["alatus", "rgb", "set-brightness", "75"]).unwrap();
    match cli_bright.command {
        Some(Commands::Rgb {
            action: RgbAction::SetBrightness { brightness },
        }) => {
            assert_eq!(brightness, 75);
        }
        _ => panic!("Expected rgb set-brightness"),
    }

    let cli_off = Cli::try_parse_from(["alatus", "rgb", "off"]).unwrap();
    assert!(matches!(
        cli_off.command,
        Some(Commands::Rgb {
            action: RgbAction::Off
        })
    ));

    let cli_on = Cli::try_parse_from(["alatus", "rgb", "on"]).unwrap();
    assert!(matches!(
        cli_on.command,
        Some(Commands::Rgb {
            action: RgbAction::On
        })
    ));

    let cli_wake = Cli::try_parse_from(["alatus", "rgb", "wake"]).unwrap();
    assert!(matches!(
        cli_wake.command,
        Some(Commands::Rgb {
            action: RgbAction::Wake
        })
    ));

    let cli_timeout = Cli::try_parse_from(["alatus", "rgb", "timeout", "45"]).unwrap();
    match cli_timeout.command {
        Some(Commands::Rgb {
            action: RgbAction::Timeout { seconds, .. },
        }) => {
            assert_eq!(seconds, Some(45));
        }
        _ => panic!("Expected rgb timeout 45"),
    }

    let cli_timeout_policy =
        Cli::try_parse_from(["alatus", "rgb", "timeout", "policy", "battery"]).unwrap();
    match cli_timeout_policy.command {
        Some(Commands::Rgb {
            action:
                RgbAction::Timeout {
                    subcommand: Some(RgbTimeoutSubcommand::Policy { policy }),
                    ..
                },
        }) => {
            assert_eq!(policy, "battery");
        }
        _ => panic!("Expected rgb timeout policy battery"),
    }

    let cli_timeout_delay =
        Cli::try_parse_from(["alatus", "rgb", "timeout", "delay", "30"]).unwrap();
    match cli_timeout_delay.command {
        Some(Commands::Rgb {
            action:
                RgbAction::Timeout {
                    subcommand: Some(RgbTimeoutSubcommand::Delay { seconds }),
                    ..
                },
        }) => {
            assert_eq!(seconds, 30);
        }
        _ => panic!("Expected rgb timeout delay 30"),
    }

    let cli_timeout_flags = Cli::try_parse_from([
        "alatus", "rgb", "timeout", "--policy", "always", "--delay", "120",
    ])
    .unwrap();
    match cli_timeout_flags.command {
        Some(Commands::Rgb {
            action: RgbAction::Timeout { policy, delay, .. },
        }) => {
            assert_eq!(policy.as_deref(), Some("always"));
            assert_eq!(delay, Some(120));
        }
        _ => panic!("Expected rgb timeout flags"),
    }
}

#[test]
fn test_parse_color() {
    assert_eq!(parse_color("#FF5500").unwrap(), (255, 85, 0));
    assert_eq!(parse_color("00AAFF").unwrap(), (0, 170, 255));
    assert_eq!(parse_color("255,85,0").unwrap(), (255, 85, 0));
    assert_eq!(parse_color("255 85 0").unwrap(), (255, 85, 0));
    assert!(parse_color("invalid").is_err());
    assert!(parse_color("300,0,0").is_err());
}

#[test]
fn test_cli_parse_session() {
    let cli = Cli::try_parse_from(["alatus", "session"]).unwrap();
    match cli.command {
        Some(Commands::Session {
            action,
            sync_accent,
            no_accent,
            auto_refresh,
            no_refresh,
            oled_care,
            no_oled_care,
        }) => {
            assert!(action.is_none());
            assert!(!sync_accent);
            assert!(!no_accent);
            assert!(!auto_refresh);
            assert!(!no_refresh);
            assert!(!oled_care);
            assert!(!no_oled_care);
        }
        _ => panic!("Expected session"),
    }

    let cli_flags = Cli::try_parse_from([
        "alatus",
        "session",
        "--no-accent",
        "--no-refresh",
        "--no-oled-care",
    ])
    .unwrap();
    match cli_flags.command {
        Some(Commands::Session {
            action,
            sync_accent: _,
            no_accent,
            auto_refresh: _,
            no_refresh,
            oled_care: _,
            no_oled_care,
        }) => {
            assert!(action.is_none());
            assert!(no_accent);
            assert!(no_refresh);
            assert!(no_oled_care);
        }
        _ => panic!("Expected session with flags"),
    }

    let cli_status = Cli::try_parse_from(["alatus", "session", "status"]).unwrap();
    match cli_status.command {
        Some(Commands::Session {
            action: Some(SessionAction::Status),
            ..
        }) => {}
        _ => panic!("Expected session status"),
    }

    let cli_set = Cli::try_parse_from([
        "alatus",
        "session",
        "set",
        "--accent",
        "off",
        "--refresh",
        "on",
        "--oled-care",
        "on",
    ])
    .unwrap();
    match cli_set.command {
        Some(Commands::Session {
            action:
                Some(SessionAction::Set {
                    accent,
                    refresh,
                    oled_care,
                }),
            ..
        }) => {
            assert_eq!(accent, Some(false));
            assert_eq!(refresh, Some(true));
            assert_eq!(oled_care, Some(true));
        }
        _ => panic!("Expected session set"),
    }

    let cli_refresh = Cli::try_parse_from(["alatus", "session", "pixel-refresh"]).unwrap();
    match cli_refresh.command {
        Some(Commands::Session {
            action: Some(SessionAction::PixelRefresh),
            ..
        }) => {}
        _ => panic!("Expected session pixel-refresh"),
    }

    let cli_refresh_alias = Cli::try_parse_from(["alatus", "session", "refresh-pixels"]).unwrap();
    match cli_refresh_alias.command {
        Some(Commands::Session {
            action: Some(SessionAction::PixelRefresh),
            ..
        }) => {}
        _ => panic!("Expected session refresh-pixels alias"),
    }
}

#[test]
fn test_cli_parse_mode_auto() {
    let cli_auto = Cli::try_parse_from(["alatus", "mode", "auto"]).unwrap();
    match cli_auto.command {
        Some(Commands::Mode {
            action: Some(ModeAction::Auto { enable }),
        }) => {
            assert!(enable.is_none());
        }
        _ => panic!("Expected mode auto"),
    }

    let cli_auto_on = Cli::try_parse_from(["alatus", "mode", "auto", "on"]).unwrap();
    match cli_auto_on.command {
        Some(Commands::Mode {
            action: Some(ModeAction::Auto { enable }),
        }) => {
            assert_eq!(enable, Some(true));
        }
        _ => panic!("Expected mode auto on"),
    }
}

#[test]
fn test_mode_format_and_mapping() {
    assert_eq!(format_mode(FirmwareMode::Balanced), "Balanced");
    assert_eq!(format_mode(FirmwareMode::Quiet), "Quiet");
    assert_eq!(format_mode(FirmwareMode::High), "Performance");
    assert_eq!(format_mode(FirmwareMode::Full), "Full");
    assert_eq!(format_mode(FirmwareMode::Unknown(42)), "Unknown");
}

#[test]
fn test_cli_parse_oled() {
    let cli_oled = Cli::try_parse_from(["alatus", "oled"]).unwrap();
    assert!(matches!(
        cli_oled.command,
        Some(Commands::Oled { action: None })
    ));

    let cli_status = Cli::try_parse_from(["alatus", "oled", "status"]).unwrap();
    assert!(matches!(
        cli_status.command,
        Some(Commands::Oled {
            action: Some(OledAction::Status)
        })
    ));

    let cli_dim_on = Cli::try_parse_from(["alatus", "oled", "dim", "on"]).unwrap();
    match cli_dim_on.command {
        Some(Commands::Oled {
            action: Some(OledAction::Dim { state, dim, level }),
        }) => {
            assert_eq!(state, Some(true));
            assert_eq!(dim, None);
            assert_eq!(level, None);
        }
        _ => panic!("Expected oled dim on"),
    }

    let cli_dim_off = Cli::try_parse_from(["alatus", "oled", "dim", "off"]).unwrap();
    match cli_dim_off.command {
        Some(Commands::Oled {
            action: Some(OledAction::Dim { state, dim, level }),
        }) => {
            assert_eq!(state, Some(false));
            assert_eq!(dim, None);
            assert_eq!(level, None);
        }
        _ => panic!("Expected oled dim off"),
    }

    let cli_dim_level = Cli::try_parse_from(["alatus", "oled", "dim", "--level", "60"]).unwrap();
    match cli_dim_level.command {
        Some(Commands::Oled {
            action: Some(OledAction::Dim { state, dim, level }),
        }) => {
            assert_eq!(state, None);
            assert_eq!(dim, None);
            assert_eq!(level, Some(60));
        }
        _ => panic!("Expected oled dim --level 60"),
    }

    let cli_disp_dim = Cli::try_parse_from(["alatus", "display", "dim", "75"]).unwrap();
    match cli_disp_dim.command {
        Some(Commands::Display {
            action: Some(DisplayAction::Dim { level }),
        }) => {
            assert_eq!(level, 75);
        }
        _ => panic!("Expected display dim 75"),
    }

    let cli_refresh = Cli::try_parse_from(["alatus", "oled", "refresh"]).unwrap();
    assert!(matches!(
        cli_refresh.command,
        Some(Commands::Oled {
            action: Some(OledAction::Refresh)
        })
    ));

    let cli_clean = Cli::try_parse_from(["alatus", "oled", "clean"]).unwrap();
    assert!(matches!(
        cli_clean.command,
        Some(Commands::Oled {
            action: Some(OledAction::Refresh)
        })
    ));
}

#[test]
fn test_cli_parse_display() {
    let cli_disp = Cli::try_parse_from(["alatus", "display"]).unwrap();
    assert!(matches!(
        cli_disp.command,
        Some(Commands::Display { action: None })
    ));

    let cli_status = Cli::try_parse_from(["alatus", "display", "status"]).unwrap();
    assert!(matches!(
        cli_status.command,
        Some(Commands::Display {
            action: Some(DisplayAction::Status)
        })
    ));

    let cli_rate_num = Cli::try_parse_from(["alatus", "display", "rate", "120"]).unwrap();
    match cli_rate_num.command {
        Some(Commands::Display {
            action: Some(DisplayAction::Rate { target, enable }),
        }) => {
            assert_eq!(target, "120");
            assert_eq!(enable, None);
        }
        _ => panic!("Expected display rate 120"),
    }

    let cli_rate_auto = Cli::try_parse_from(["alatus", "display", "rate", "auto"]).unwrap();
    match cli_rate_auto.command {
        Some(Commands::Display {
            action: Some(DisplayAction::Rate { target, enable }),
        }) => {
            assert_eq!(target, "auto");
            assert_eq!(enable, None);
        }
        _ => panic!("Expected display rate auto"),
    }

    let cli_rate_auto_off =
        Cli::try_parse_from(["alatus", "display", "rate", "auto", "off"]).unwrap();
    match cli_rate_auto_off.command {
        Some(Commands::Display {
            action: Some(DisplayAction::Rate { target, enable }),
        }) => {
            assert_eq!(target, "auto");
            assert_eq!(enable, Some(false));
        }
        _ => panic!("Expected display rate auto off"),
    }
}

#[test]
fn test_cli_parse_daemon() {
    let cli_sess_start = Cli::try_parse_from(["alatus", "daemon", "session", "start"]).unwrap();
    assert!(matches!(
        cli_sess_start.command,
        Some(Commands::Daemon {
            target: DaemonTarget::Session {
                action: Some(DaemonServiceAction::Start)
            }
        })
    ));

    let cli_sess_status = Cli::try_parse_from(["alatus", "daemon", "session", "status"]).unwrap();
    assert!(matches!(
        cli_sess_status.command,
        Some(Commands::Daemon {
            target: DaemonTarget::Session {
                action: Some(DaemonServiceAction::Status)
            }
        })
    ));

    let cli_sys_restart = Cli::try_parse_from(["alatus", "daemon", "system", "restart"]).unwrap();
    assert!(matches!(
        cli_sys_restart.command,
        Some(Commands::Daemon {
            target: DaemonTarget::System {
                action: Some(DaemonServiceAction::Restart)
            }
        })
    ));

    let cli_daemon_status = Cli::try_parse_from(["alatus", "daemon", "status"]).unwrap();
    assert!(matches!(
        cli_daemon_status.command,
        Some(Commands::Daemon {
            target: DaemonTarget::Status
        })
    ));
}

#[test]
fn test_cli_parse_rgb_sync() {
    let cli_sync_on = Cli::try_parse_from(["alatus", "rgb", "sync", "on"]).unwrap();
    assert!(matches!(
        cli_sync_on.command,
        Some(Commands::Rgb {
            action: RgbAction::Sync { enable: Some(true) }
        })
    ));

    let cli_sync_off = Cli::try_parse_from(["alatus", "rgb", "sync", "off"]).unwrap();
    assert!(matches!(
        cli_sync_off.command,
        Some(Commands::Rgb {
            action: RgbAction::Sync {
                enable: Some(false)
            }
        })
    ));
}

#[test]
fn test_cli_parse_shortcuts_and_aliases() {
    // Direct mode switches
    let cli_bal = Cli::try_parse_from(["alatus", "mode", "balanced"]).unwrap();
    assert!(matches!(
        cli_bal.command,
        Some(Commands::Mode {
            action: Some(ModeAction::Balanced)
        })
    ));

    let cli_bal_alias = Cli::try_parse_from(["alatus", "mode", "bal"]).unwrap();
    assert!(matches!(
        cli_bal_alias.command,
        Some(Commands::Mode {
            action: Some(ModeAction::Balanced)
        })
    ));

    let cli_quiet = Cli::try_parse_from(["alatus", "mode", "quiet"]).unwrap();
    assert!(matches!(
        cli_quiet.command,
        Some(Commands::Mode {
            action: Some(ModeAction::Quiet)
        })
    ));

    let cli_perf = Cli::try_parse_from(["alatus", "mode", "perf"]).unwrap();
    assert!(matches!(
        cli_perf.command,
        Some(Commands::Mode {
            action: Some(ModeAction::Performance)
        })
    ));

    let cli_full = Cli::try_parse_from(["alatus", "mode", "full"]).unwrap();
    assert!(matches!(
        cli_full.command,
        Some(Commands::Mode {
            action: Some(ModeAction::Full)
        })
    ));

    // Direct fan switches
    let cli_fan_bal = Cli::try_parse_from(["alatus", "fan", "bal"]).unwrap();
    assert!(matches!(
        cli_fan_bal.command,
        Some(Commands::Fan {
            action: Some(FanAction::Balanced)
        })
    ));

    let cli_fan_perf = Cli::try_parse_from(["alatus", "fan", "perf"]).unwrap();
    assert!(matches!(
        cli_fan_perf.command,
        Some(Commands::Fan {
            action: Some(FanAction::Performance)
        })
    ));

    // Top level cycle & watch
    let cli_cycle = Cli::try_parse_from(["alatus", "cycle"]).unwrap();
    assert!(matches!(cli_cycle.command, Some(Commands::Cycle)));

    let cli_watch = Cli::try_parse_from(["alatus", "watch"]).unwrap();
    assert!(matches!(cli_watch.command, Some(Commands::Watch)));

    let cli_status_w = Cli::try_parse_from(["alatus", "status", "-w"]).unwrap();
    assert!(matches!(
        cli_status_w.command,
        Some(Commands::Status {
            json: false,
            watch: true
        })
    ));

    let cli_status_jw = Cli::try_parse_from(["alatus", "status", "-j", "-w"]).unwrap();
    assert!(matches!(
        cli_status_jw.command,
        Some(Commands::Status {
            json: true,
            watch: true
        })
    ));

    // Visible aliases: thermal, profile -> Mode
    let cli_thermal = Cli::try_parse_from(["alatus", "thermal", "balanced"]).unwrap();
    assert!(matches!(
        cli_thermal.command,
        Some(Commands::Mode {
            action: Some(ModeAction::Balanced)
        })
    ));

    let cli_profile = Cli::try_parse_from(["alatus", "profile", "quiet"]).unwrap();
    assert!(matches!(
        cli_profile.command,
        Some(Commands::Mode {
            action: Some(ModeAction::Quiet)
        })
    ));

    // Visible alias: performance -> Perf
    let cli_perf_long = Cli::try_parse_from(["alatus", "performance"]).unwrap();
    assert!(matches!(cli_perf_long.command, Some(Commands::Perf)));

    // Visible aliases: battery, charge -> ChargeLimit
    let cli_bat = Cli::try_parse_from(["alatus", "battery", "80"]).unwrap();
    assert!(matches!(
        cli_bat.command,
        Some(Commands::ChargeLimit { percentage: 80 })
    ));

    let cli_chg = Cli::try_parse_from(["alatus", "charge", "75"]).unwrap();
    assert!(matches!(
        cli_chg.command,
        Some(Commands::ChargeLimit { percentage: 75 })
    ));

    // Completions subcommand
    let cli_comp_bash = Cli::try_parse_from(["alatus", "completions", "bash"]).unwrap();
    assert!(matches!(
        cli_comp_bash.command,
        Some(Commands::Completions {
            shell: clap_complete::Shell::Bash
        })
    ));

    let cli_comp_fish = Cli::try_parse_from(["alatus", "completions", "fish"]).unwrap();
    assert!(matches!(
        cli_comp_fish.command,
        Some(Commands::Completions {
            shell: clap_complete::Shell::Fish
        })
    ));

    let cli_comp_zsh = Cli::try_parse_from(["alatus", "completions", "zsh"]).unwrap();
    assert!(matches!(
        cli_comp_zsh.command,
        Some(Commands::Completions {
            shell: clap_complete::Shell::Zsh
        })
    ));
}
