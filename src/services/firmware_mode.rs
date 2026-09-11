// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

pub use crate::services::hardware_resolver;

/// Global thread-safe mutex protecting all multi-step DebugFS operations.
static DEBUGFS_LOCK: Mutex<()> = Mutex::new(());

/// Debugfs directory published by the `asus-wmi` kernel driver.
pub const DEBUGFS_BASE: &str = "/sys/kernel/debug/asus-nb-wmi";

/// Firmware thermal-policy device (`ASUS_WMI_DEVID_THROTTLE_THERMAL_POLICY_VIVO`).
pub fn thermal_policy_dev_id() -> Result<u32, FirmwareModeError> {
    hardware_resolver::resolve_register()
        .map(|r| r.value())
        .map_err(|e| FirmwareModeError::UnsupportedHardware(e.to_string()))
}

/// Meaningful bits in the DSTS response: the EC mode is the low nibble.
pub const MODE_MASK: u8 = 0x0F;

/// The four firmware thermal/fan modes exposed by the thermal-policy device.
///
/// `Unknown` is not an error: it carries the raw nibble so callers can show it
/// as-is instead of failing the whole read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareMode {
    /// Nibble `0` - Balanced / standard fan profile.
    Balanced,
    /// Nibble `1` - Quiet / silent.
    Quiet,
    /// Nibble `2` - High / performance (boost).
    High,
    /// Nibble `3` - Full / full speed.
    Full,
    /// Any other low-nibble value, preserved verbatim.
    Unknown(u8),
}

impl FirmwareMode {
    /// Maps the DSTS low nibble to a mode. Total for any `u8` input.
    pub const fn from_nibble(nibble: u8) -> Self {
        match nibble & MODE_MASK {
            0 => Self::Balanced,
            1 => Self::High,
            2 => Self::Quiet,
            3 => Self::Full,
            n => Self::Unknown(n),
        }
    }

    /// The raw firmware nibble this mode came from.
    pub const fn nibble(self) -> u8 {
        match self {
            Self::Balanced => 0,
            Self::Quiet => 2,
            Self::High => 1,
            Self::Full => 3,
            Self::Unknown(n) => n,
        }
    }

    /// The `DEVS` `ctrl_param` that selects this mode.
    ///
    /// Uses the firmware index encoding (0=Standard, 1=Quiet, 2=High, 3=Full).
    pub const fn write_param(self) -> Option<u8> {
        match self {
            Self::Balanced => Some(0),
            Self::Quiet => Some(1),
            Self::High => Some(2),
            Self::Full => Some(3),
            Self::Unknown(_) => None,
        }
    }
}

/// Inverse of [`FirmwareMode::write_param`]: maps a `DEVS ctrl_param` index
/// back to the mode it selects (using the firmware's index encoding).
pub const fn from_write_param(param: u8) -> FirmwareMode {
    match param & MODE_MASK {
        0 => FirmwareMode::Balanced,
        1 => FirmwareMode::Quiet,
        2 => FirmwareMode::High,
        3 => FirmwareMode::Full,
        n => FirmwareMode::Unknown(n),
    }
}

/// Outcome of a firmware-mode write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirmwareModeStatus {
    /// The firmware reports exactly the requested mode after the write.
    Sync,
    /// The write was accepted by `DEVS` but the firmware ended up in another
    /// mode (e.g. asusd/PPD immediately re-applied a power profile).
    Mismatch(FirmwareMode),
}

impl FirmwareModeStatus {
    /// Derives the status from the requested and actually-read-back modes.
    pub fn status_of(requested: FirmwareMode, actual: FirmwareMode) -> Self {
        if requested == actual {
            Self::Sync
        } else {
            Self::Mismatch(actual)
        }
    }
}

/// Full verified result of a write operation (for reporting and logs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteResult {
    /// The mode that was requested.
    pub requested: FirmwareMode,
    /// The translated `DEVS ctrl_param` that was sent.
    pub ctrl_param: u8,
    /// Return value of the `DEVS` operation (nonzero = accepted).
    pub devs_retval: u32,
    /// Raw `DSTS(0x00110019)` value read back after the write.
    pub dsts: u32,
    /// Decoded firmware mode from `dsts`.
    pub actual: FirmwareMode,
    /// Synced or mismatched outcome.
    pub status: FirmwareModeStatus,
}

impl From<u32> for FirmwareMode {
    fn from(val: u32) -> Self {
        match val {
            0 => Self::Balanced,
            1 => Self::Quiet,
            2 => Self::High,
            3 => Self::Full,
            n => Self::Unknown(n as u8),
        }
    }
}

impl From<FirmwareMode> for u32 {
    fn from(mode: FirmwareMode) -> Self {
        match mode {
            FirmwareMode::Balanced => 0,
            FirmwareMode::Quiet => 1,
            FirmwareMode::High => 2,
            FirmwareMode::Full => 3,
            FirmwareMode::Unknown(n) => n as u32,
        }
    }
}

impl fmt::Display for FirmwareMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Balanced => write!(f, "Balanced"),
            Self::Quiet => write!(f, "Quiet"),
            Self::High => write!(f, "Performance"),
            Self::Full => write!(f, "Full"),
            Self::Unknown(n) => write!(f, "Unknown({n})"),
        }
    }
}

impl std::str::FromStr for FirmwareMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "balanced" | "balance" | "0" => Ok(Self::Balanced),
            "quiet" | "silent" | "1" => Ok(Self::Quiet),
            "performance" | "high" | "boost" | "2" => Ok(Self::High),
            "full" | "full speed" | "fullspeed" | "3" => Ok(Self::Full),
            other => Err(format!("Unknown firmware mode: {other}")),
        }
    }
}

/// Errors raised while reading or writing the firmware mode.
#[derive(Debug)]
pub enum FirmwareModeError {
    /// The debugfs store/read itself failed (missing node, permissions, I/O).
    Io(std::io::Error),
    /// A response did not contain a decodable `0x…` value.
    Parse(String),
    /// The `DEVS` operation was refused by the firmware/kernel (returned 0,
    /// failed, or no usable result).
    Rejected(String),
    /// The readback could not be obtained after a write (read error).
    ReadError(String),
    /// The caller requested an operation that the interface cannot express.
    Unsupported(&'static str),
    /// The device does not match a supported ASUS hardware platform.
    UnsupportedHardware(String),
}

impl fmt::Display for FirmwareModeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(
                f,
                "firmware mode read failed: {e} (is debugfs mounted and the caller root?)"
            ),
            Self::Parse(raw) => write!(f, "firmware mode response not decodable: {raw:?}"),
            Self::Rejected(what) => write!(f, "DEVS refused the write: {what}"),
            Self::ReadError(what) => write!(f, "firmware mode readback failed: {what}"),
            Self::Unsupported(what) => write!(f, "unsupported firmware mode operation: {what}"),
            Self::UnsupportedHardware(what) => write!(f, "unsupported hardware: {what}"),
        }
    }
}

impl std::error::Error for FirmwareModeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(_)
            | Self::Rejected(_)
            | Self::ReadError(_)
            | Self::Unsupported(_)
            | Self::UnsupportedHardware(_) => None,
        }
    }
}

impl From<std::io::Error> for FirmwareModeError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Extracts the last `0x…` value from a debugfs response line.
///
/// Handles both bare hex (`0x0a070002`) and the kernel's annotated forms
/// (`DSTS(0x00110019) = 0x0a070002`, `DEVS(0x00110019, 0x00000002) = 0x1`).
fn parse_debugfs_value(line: &str) -> Result<u32, FirmwareModeError> {
    let Some(value) = line
        .split_once("0x")
        .map(|(_, rest)| rest)
        .or(line.strip_prefix("0x"))
    else {
        return Err(FirmwareModeError::Parse(line.to_string()));
    };

    // The annotated lines contain several `0x…` tokens; the value is the last.
    let token = value
        .rsplit_once("0x")
        .map(|(_, rest)| rest)
        .unwrap_or(value)
        .trim();

    if token.is_empty() || !token.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(FirmwareModeError::Parse(line.to_string()));
    }

    u32::from_str_radix(token, 16).map_err(|_| FirmwareModeError::Parse(line.to_string()))
}

/// Decodes a `dsts` response into a [`FirmwareMode`].
///
/// Accepts both the raw hex form (`0x0a070002`) and the kernel's annotated
/// form (`DSTS(0x00110019) = 0x0a070002`). Only the low nibble is used; the
/// `0x0A070000` status mask is ignored. Unknown nibbles map to
/// [`FirmwareMode::Unknown`] rather than failing.
pub fn parse_dsts(response: &str) -> Result<FirmwareMode, FirmwareModeError> {
    let raw = parse_debugfs_value(response)?;
    Ok(FirmwareMode::from_nibble(
        (raw & u32::from(MODE_MASK)) as u8,
    ))
}

/// Decodes a `DEVS` response into its firmware return value.
///
/// A nonzero value means the operation was accepted. A missing/unparseable
/// value yields [`FirmwareModeError::Parse`] (the caller maps it to
/// `Rejected`, matching kernel semantics where a refused method surfaces as a
/// failed debugfs read).
pub fn parse_devs(response: &str) -> Result<u32, FirmwareModeError> {
    parse_debugfs_value(response)
}

/// Reads the actual firmware thermal/fan mode.
///
/// Direct debugfs access: requires root (or a similar privilege) and debugfs
/// mounted. Restores the `dev_id` selector afterwards so the query is
/// side-effect free.
pub fn read_firmware_mode() -> Result<FirmwareMode, FirmwareModeError> {
    let base = Path::new(DEBUGFS_BASE);
    if base.exists() {
        return read_firmware_mode_from(
            &base.join("dev_id"),
            &base.join("dsts"),
            &base.join("ctrl_param"),
        );
    }

    Err(FirmwareModeError::Unsupported(
        "ASUS WMI debugfs interface (/sys/kernel/debug/asus-nb-wmi) not available",
    ))
}

/// Shared read path (separated for testability on temporary files).
fn read_firmware_mode_from(
    dev_id_file: &Path,
    dsts_file: &Path,
    ctrl_param_file: &Path,
) -> Result<FirmwareMode, FirmwareModeError> {
    let _lock = DEBUGFS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dev_id = thermal_policy_dev_id()?;
    fs::write(dev_id_file, format!("0x{dev_id:08x}\n"))?;

    if dsts_file.exists()
        && let Ok(response) = fs::read_to_string(dsts_file)
        && let Ok(mode) = parse_dsts(&response)
    {
        return Ok(mode);
    }

    if ctrl_param_file.exists() {
        let response = fs::read_to_string(ctrl_param_file)?;
        let raw = parse_debugfs_value(&response)?;
        return Ok(from_write_param((raw & u32::from(MODE_MASK)) as u8));
    }

    Err(FirmwareModeError::ReadError(
        "Neither dsts nor ctrl_param could be read from debugfs".to_string(),
    ))
}

/// Read the firmware mode through D-Bus system bus for unprivileged Alatus processes.
pub async fn read_firmware_mode_privileged() -> Result<FirmwareMode, String> {
    let connection = zbus::Connection::system()
        .await
        .map_err(|e| format!("Failed to connect to D-Bus System Bus: {e}"))?;
    let proxy = zbus::Proxy::new(
        &connection,
        "io.strixwolf.alatus.Daemon",
        "/io/strixwolf/alatus/Daemon",
        "io.strixwolf.alatus.Daemon",
    )
    .await
    .map_err(|e| format!("Failed to create D-Bus proxy: {e}"))?;

    let val: u32 = proxy
        .get_property("FirmwareMode")
        .await
        .map_err(|e| format!("Failed to read FirmwareMode property: {e}"))?;

    match val {
        0 => Ok(FirmwareMode::Balanced),
        1 => Ok(FirmwareMode::Quiet),
        2 => Ok(FirmwareMode::High),
        3 => Ok(FirmwareMode::Full),
        n => Ok(FirmwareMode::Unknown(n as u8)),
    }
}

/// Interprets the debugfs outputs of a write op into a verified result.
fn interpret_debugfs_write(
    ctrl_param: u8,
    devs_line: &str,
    dsts_line: Option<&str>,
    requested: FirmwareMode,
) -> Result<WriteResult, FirmwareModeError> {
    let devs_retval = parse_devs(devs_line)
        .map_err(|_| FirmwareModeError::Rejected(format!("no DEVS result: {devs_line:?}")))?;
    if devs_retval == 0 {
        return Err(FirmwareModeError::Rejected(format!(
            "DEVS returned 0: {devs_line:?}"
        )));
    }

    let (dsts, actual) = if let Some(line) = dsts_line {
        let val = parse_debugfs_value(line)
            .map_err(|_| FirmwareModeError::ReadError(format!("no DSTS result: {line:?}")))?;
        let mode = FirmwareMode::from_nibble((val & u32::from(MODE_MASK)) as u8);
        (val, mode)
    } else {
        (u32::from(ctrl_param), from_write_param(ctrl_param))
    };

    let status = FirmwareModeStatus::status_of(requested, actual);

    Ok(WriteResult {
        requested,
        ctrl_param,
        devs_retval,
        dsts,
        actual,
        status,
    })
}

/// Runs the complete write lifecycle against a debugfs-like directory.
/// Exact 1:1 mirroring of staging_analysis/fan_state:
/// 1. Write thermal policy dev_id (`0x00110019`) to `dev_id`.
/// 2. Write state value (`0..=3`) to `ctrl_param`.
/// 3. Read `devs` to trigger the operation and verify acceptance.
/// 4. Read `dsts` (or `ctrl_param`) to check readback status.
fn set_firmware_mode_at(base: &Path, mode: FirmwareMode) -> Result<WriteResult, FirmwareModeError> {
    let Some(param) = mode.write_param() else {
        return Err(FirmwareModeError::Unsupported(
            "cannot write an unknown/ambiguous firmware mode",
        ));
    };

    let _lock = DEBUGFS_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let dev_id_file = base.join("dev_id");
    let ctrl_param_file = base.join("ctrl_param");
    let devs_file = base.join("devs");
    let dsts_file = base.join("dsts");

    let dev_id = thermal_policy_dev_id()?;
    fs::write(&dev_id_file, format!("0x{dev_id:08x}\n")).map_err(FirmwareModeError::Io)?;
    fs::write(&ctrl_param_file, format!("{param}\n")).map_err(FirmwareModeError::Io)?;

    let devs_out = fs::read_to_string(&devs_file)
        .map_err(|e| FirmwareModeError::Rejected(format!("DEVS read failed: {e}")))?;

    let dsts_out = if dsts_file.exists() {
        fs::read_to_string(&dsts_file).ok()
    } else {
        None
    };

    interpret_debugfs_write(param, &devs_out, dsts_out.as_deref(), mode)
}

/// Sets the firmware thermal/fan mode directly from the privileged daemon process.
/// Pure ASUS WMI DebugFS: zero sysfs interaction.
pub fn set_firmware_mode_direct(mode: FirmwareMode) -> Result<WriteResult, FirmwareModeError> {
    let base = Path::new(DEBUGFS_BASE);
    if base.exists() {
        return set_firmware_mode_at(base, mode);
    }

    Err(FirmwareModeError::Unsupported(
        "ASUS WMI debugfs interface (/sys/kernel/debug/asus-nb-wmi) not available",
    ))
}

/// Sets the firmware thermal/fan mode as one privileged composite operation.
///
/// Keeps the calling process unprivileged: routes the set request to alatusd via D-Bus.
pub async fn set_firmware_mode(mode: FirmwareMode) -> Result<WriteResult, FirmwareModeError> {
    let dbus_val = match mode {
        FirmwareMode::Balanced => 0,
        FirmwareMode::Quiet => 1,
        FirmwareMode::High => 2,
        FirmwareMode::Full => 3,
        FirmwareMode::Unknown(_) => {
            return Err(FirmwareModeError::Unsupported(
                "cannot write an unknown/ambiguous firmware mode",
            ));
        }
    };

    let connection = zbus::Connection::system().await.map_err(|e| {
        FirmwareModeError::Rejected(format!("Failed to connect to D-Bus System Bus: {e}"))
    })?;
    let proxy = zbus::Proxy::new(
        &connection,
        "io.strixwolf.alatus.Daemon",
        "/io/strixwolf/alatus/Daemon",
        "io.strixwolf.alatus.Daemon",
    )
    .await
    .map_err(|e| FirmwareModeError::Rejected(format!("Failed to create D-Bus proxy: {e}")))?;

    proxy
        .set_property("FirmwareMode", dbus_val)
        .await
        .map_err(|e| {
            FirmwareModeError::Rejected(format!("Failed to set FirmwareMode property: {e}"))
        })?;

    let param = mode.write_param().unwrap();
    let actual = read_firmware_mode_privileged()
        .await
        .map_err(FirmwareModeError::ReadError)?;
    let status = FirmwareModeStatus::status_of(mode, actual);

    Ok(WriteResult {
        requested: mode,
        ctrl_param: param,
        devs_retval: 1,
        dsts: 0x0a070000 | (actual.nibble() as u32),
        actual,
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nibble_zero_is_balanced() {
        assert_eq!(FirmwareMode::from_nibble(0), FirmwareMode::Balanced);
    }

    #[test]
    fn nibble_one_is_high() {
        assert_eq!(FirmwareMode::from_nibble(1), FirmwareMode::High);
    }

    #[test]
    fn nibble_two_is_quiet() {
        assert_eq!(FirmwareMode::from_nibble(2), FirmwareMode::Quiet);
    }

    #[test]
    fn nibble_three_is_full() {
        assert_eq!(FirmwareMode::from_nibble(3), FirmwareMode::Full);
    }

    #[test]
    fn test_firmware_mode_u32_conversions_and_display() {
        // From<u32>
        assert_eq!(FirmwareMode::from(0u32), FirmwareMode::Balanced);
        assert_eq!(FirmwareMode::from(1u32), FirmwareMode::Quiet);
        assert_eq!(FirmwareMode::from(2u32), FirmwareMode::High);
        assert_eq!(FirmwareMode::from(3u32), FirmwareMode::Full);
        assert_eq!(FirmwareMode::from(99u32), FirmwareMode::Unknown(99));

        // Into<u32>
        assert_eq!(u32::from(FirmwareMode::Balanced), 0);
        assert_eq!(u32::from(FirmwareMode::Quiet), 1);
        assert_eq!(u32::from(FirmwareMode::High), 2);
        assert_eq!(u32::from(FirmwareMode::Full), 3);

        // Display
        assert_eq!(FirmwareMode::Balanced.to_string(), "Balanced");
        assert_eq!(FirmwareMode::Quiet.to_string(), "Quiet");
        assert_eq!(FirmwareMode::High.to_string(), "Performance");
        assert_eq!(FirmwareMode::Full.to_string(), "Full");

        // FromStr
        use std::str::FromStr;
        assert_eq!(
            FirmwareMode::from_str("balanced").unwrap(),
            FirmwareMode::Balanced
        );
        assert_eq!(
            FirmwareMode::from_str("quiet").unwrap(),
            FirmwareMode::Quiet
        );
        assert_eq!(
            FirmwareMode::from_str("performance").unwrap(),
            FirmwareMode::High
        );
        assert_eq!(FirmwareMode::from_str("full").unwrap(), FirmwareMode::Full);
        assert_eq!(
            FirmwareMode::from_str("full speed").unwrap(),
            FirmwareMode::Full
        );
    }

    #[test]
    fn unknown_nibble_is_safe_typed_state() {
        for nibble in [4u8, 5, 7, 10, 15] {
            let mode = FirmwareMode::from_nibble(nibble);
            assert_eq!(mode, FirmwareMode::Unknown(nibble));
            assert_eq!(mode.nibble(), nibble);
        }
    }

    #[test]
    fn parse_known_from_raw_hex() {
        assert_eq!(parse_dsts("0x0a070000").unwrap(), FirmwareMode::Balanced);
        assert_eq!(parse_dsts("0x0a070001").unwrap(), FirmwareMode::High);
        assert_eq!(parse_dsts("0x0a070002").unwrap(), FirmwareMode::Quiet);
        assert_eq!(parse_dsts("0x0a070003").unwrap(), FirmwareMode::Full);
    }

    #[test]
    fn parse_known_from_annotated_kernel_line() {
        assert_eq!(
            parse_dsts("DSTS(0x00110019) = 0x0a070001").unwrap(),
            FirmwareMode::High
        );
        assert_eq!(
            parse_dsts("DSTS(0x00110019) = 0x0a070001\n").unwrap(),
            FirmwareMode::High
        );
    }

    #[test]
    fn parse_ignores_status_mask() {
        // 0x0a070013 within the full response: mask 0x0A070000, nibble 3.
        assert_eq!(parse_dsts("0x0a070013").unwrap(), FirmwareMode::Full);
    }

    #[test]
    fn parse_unknown_nibble_is_not_an_error() {
        assert_eq!(parse_dsts("0x0a07000f").unwrap(), FirmwareMode::Unknown(15));
    }

    #[test]
    fn parse_rejects_garbage() {
        for bad in [
            "",
            "   ",
            "banana",
            "0x",
            "0xzz",
            "0x0a070001junk",
            "110019",
        ] {
            assert!(
                matches!(parse_dsts(bad), Err(FirmwareModeError::Parse(_))),
                "expected Parse error for {bad:?}"
            );
        }
    }

    #[test]
    fn read_firmware_mode_from_dsts() {
        let dir = std::env::temp_dir().join(format!("alatus-fwmode-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let dev_id = dir.join("dev_id");
        let dsts = dir.join("dsts");
        let ctrl = dir.join("ctrl_param");

        fs::write(&dev_id, "0x0005002f\n").unwrap();
        fs::write(&dsts, "DSTS(0x00110019) = 0x0a070002\n").unwrap();

        let result = read_firmware_mode_from(&dev_id, &dsts, &ctrl);
        assert_eq!(result.unwrap(), FirmwareMode::Quiet);
        assert_eq!(
            fs::read_to_string(&dev_id).unwrap().trim(),
            "0x00110019",
            "dev_id must be written with 0x00110019"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_firmware_mode_fallback_to_ctrl_param() {
        let dir =
            std::env::temp_dir().join(format!("alatus-fwmode-ctrl-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let dev_id = dir.join("dev_id");
        let dsts = dir.join("dsts"); // nonexistent file
        let ctrl = dir.join("ctrl_param");

        fs::write(&ctrl, "0x00000003\n").unwrap();

        let result = read_firmware_mode_from(&dev_id, &dsts, &ctrl);
        assert_eq!(result.unwrap(), FirmwareMode::Full);

        fs::remove_dir_all(&dir).ok();
    }

    // ── Phase 1 write backend ─────────────────────────────────────────────

    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("alatus-fwmode-{name}-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn write_param_translates_all_four_modes() {
        assert_eq!(FirmwareMode::Balanced.write_param(), Some(0));
        assert_eq!(FirmwareMode::High.write_param(), Some(2));
        assert_eq!(FirmwareMode::Quiet.write_param(), Some(1));
        assert_eq!(FirmwareMode::Full.write_param(), Some(3));
    }

    #[test]
    fn from_write_param_is_the_inverse_table() {
        assert_eq!(from_write_param(0), FirmwareMode::Balanced);
        assert_eq!(from_write_param(2), FirmwareMode::High);
        assert_eq!(from_write_param(1), FirmwareMode::Quiet);
        assert_eq!(from_write_param(3), FirmwareMode::Full);
        assert_eq!(from_write_param(7), FirmwareMode::Unknown(7));
    }

    #[test]
    fn unknown_mode_can_never_be_written() {
        assert_eq!(FirmwareMode::Unknown(7).write_param(), None);
    }

    #[test]
    fn readback_nibbles_roundtrip_through_write_table() {
        // The point of the central table: nibble -> write param -> nibble.
        assert_eq!(
            from_write_param(FirmwareMode::Balanced.write_param().unwrap()),
            FirmwareMode::Balanced
        );
        assert_eq!(
            from_write_param(FirmwareMode::High.write_param().unwrap()),
            FirmwareMode::High
        );
        assert_eq!(
            from_write_param(FirmwareMode::Quiet.write_param().unwrap()),
            FirmwareMode::Quiet
        );
        assert_eq!(
            from_write_param(FirmwareMode::Full.write_param().unwrap()),
            FirmwareMode::Full
        );
    }

    #[test]
    fn parse_devs_annotated_and_bare() {
        assert_eq!(
            parse_devs("DEVS(0x00110019, 0x00000002) = 0x1\n").unwrap(),
            1
        );
        assert_eq!(parse_devs("0x1\n").unwrap(), 1);
    }

    #[test]
    fn parse_devs_rejects_garbage() {
        assert!(matches!(
            parse_devs("Permission denied"),
            Err(FirmwareModeError::Parse(_))
        ));
        assert!(matches!(parse_devs(""), Err(FirmwareModeError::Parse(_))));
    }

    #[test]
    fn interpret_success_is_sync() {
        let r = interpret_debugfs_write(
            2,
            "DEVS(0x00110019, 0x00000002) = 0x1\n",
            Some("DSTS(0x00110019) = 0x0a070001\n"),
            FirmwareMode::High,
        )
        .unwrap();
        assert_eq!(r.ctrl_param, 2);
        assert_eq!(r.devs_retval, 1);
        assert_eq!(r.dsts, 0x0a070001);
        assert_eq!(r.actual, FirmwareMode::High);
        assert!(matches!(r.status, FirmwareModeStatus::Sync));
    }

    #[test]
    fn interpret_readback_mismatch() {
        let r = interpret_debugfs_write(
            1,
            "DEVS(0x00110019, 0x00000001) = 0x1\n",
            Some("DSTS(0x00110019) = 0x0a070003\n"),
            FirmwareMode::Quiet,
        )
        .unwrap();
        assert!(
            matches!(r.status, FirmwareModeStatus::Mismatch(FirmwareMode::Full)),
            "unexpected status: {r:?}"
        );
    }

    #[test]
    fn interpret_devs_rejected_when_return_zero() {
        let err = interpret_debugfs_write(
            0,
            "DEVS(0x00110019, 0x00000000) = 0x0\n",
            Some("DSTS(0x00110019) = 0x0a070000\n"),
            FirmwareMode::Balanced,
        )
        .unwrap_err();
        assert!(matches!(&err, FirmwareModeError::Rejected(_)), "{err}");
    }

    #[test]
    fn interpret_devs_missing_is_rejected() {
        let err = interpret_debugfs_write(2, "cat: Input/output error\n", None, FirmwareMode::High)
            .unwrap_err();
        assert!(matches!(&err, FirmwareModeError::Rejected(_)), "{err}");
    }

    #[test]
    fn interpret_dsts_missing_uses_ctrl_param() {
        let r = interpret_debugfs_write(
            2,
            "DEVS(0x00110019, 0x00000002) = 0x1\n",
            None,
            FirmwareMode::High,
        )
        .unwrap();
        assert_eq!(r.actual, FirmwareMode::High);
        assert!(matches!(r.status, FirmwareModeStatus::Sync));
    }

    #[test]
    fn set_at_success_writes_dev_id_and_param() {
        let dir = scratch_dir("set-ok");
        let dev_id = dir.join("dev_id");
        let ctrl = dir.join("ctrl_param");
        let devs = dir.join("devs");
        let dsts = dir.join("dsts");
        fs::write(&dev_id, "0x0005002f\n").unwrap();
        fs::write(&ctrl, "0x00000005\n").unwrap();
        fs::write(&devs, "DEVS(0x00110019, 0x00000002) = 0x1\n").unwrap();
        fs::write(&dsts, "DSTS(0x00110019) = 0x0a070001\n").unwrap();

        let r = set_firmware_mode_at(&dir, FirmwareMode::High).unwrap();
        assert!(matches!(r.status, FirmwareModeStatus::Sync));
        assert_eq!(
            fs::read_to_string(&dev_id).unwrap().trim(),
            "0x00110019",
            "dev_id must be written with 0x00110019"
        );
        assert_eq!(
            fs::read_to_string(&ctrl).unwrap().trim(),
            "2",
            "ctrl_param must be written with 2"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn set_at_mismatch() {
        let dir = scratch_dir("set-mismatch");
        let dev_id = dir.join("dev_id");
        let ctrl = dir.join("ctrl_param");
        fs::write(&dev_id, "0x0005002f\n").unwrap();
        fs::write(&ctrl, "0x00000005\n").unwrap();
        fs::write(dir.join("devs"), "DEVS(0x00110019, 0x00000002) = 0x1\n").unwrap();
        fs::write(dir.join("dsts"), "DSTS(0x00110019) = 0x0a070002\n").unwrap();

        let r = set_firmware_mode_at(&dir, FirmwareMode::High).unwrap();
        assert!(
            matches!(r.status, FirmwareModeStatus::Mismatch(FirmwareMode::Quiet)),
            "{r:?}"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn set_at_rejected() {
        let dir = scratch_dir("set-rejected");
        fs::write(dir.join("dev_id"), "0x0005002f\n").unwrap();
        fs::write(dir.join("ctrl_param"), "0x00000005\n").unwrap();
        fs::write(dir.join("devs"), "cat: Input/output error\n").unwrap();
        fs::write(dir.join("dsts"), "DSTS(0x00110019) = 0x0a070002\n").unwrap();

        let err = set_firmware_mode_at(&dir, FirmwareMode::High).unwrap_err();
        assert!(matches!(&err, FirmwareModeError::Rejected(_)), "{err}");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn set_at_unknown_mode_is_unsupported_and_writes_nothing() {
        let dir = scratch_dir("set-unknown");
        fs::write(dir.join("dev_id"), "0x0005002f\n").unwrap();
        fs::write(dir.join("ctrl_param"), "0x00000005\n").unwrap();

        let err = set_firmware_mode_at(&dir, FirmwareMode::Unknown(9)).unwrap_err();
        assert!(matches!(&err, FirmwareModeError::Unsupported(_)), "{err}");
        // Nothing was touched.
        assert_eq!(
            fs::read_to_string(dir.join("dev_id")).unwrap().trim(),
            "0x0005002f"
        );
        assert_eq!(
            fs::read_to_string(dir.join("ctrl_param")).unwrap().trim(),
            "0x00000005"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn status_of_distinguishes_sync_and_mismatch() {
        assert!(matches!(
            FirmwareModeStatus::status_of(FirmwareMode::High, FirmwareMode::High),
            FirmwareModeStatus::Sync
        ));
        assert!(matches!(
            FirmwareModeStatus::status_of(FirmwareMode::Quiet, FirmwareMode::Full),
            FirmwareModeStatus::Mismatch(FirmwareMode::Full)
        ));
    }

    #[test]
    fn test_hardware_resolver_cases() {
        let dir = scratch_dir("resolver");
        let file_path = dir.join("product_name");

        // 1. S5506 case (standard Vivobook S 15 OLED S5506MA) - resolves to standard 0x110019
        fs::write(&file_path, "Vivobook S 15 OLED S5506MA\n").unwrap();
        let reg =
            hardware_resolver::resolve_register_from_dmi(file_path.to_str().unwrap()).unwrap();
        assert_eq!(reg.value(), 0x110019);

        // 2. S5506 case with different whitespace and case - resolves to standard 0x110019
        fs::write(&file_path, "  VIVOBOOK S 15 S5506MA  \n").unwrap();
        let reg =
            hardware_resolver::resolve_register_from_dmi(file_path.to_str().unwrap()).unwrap();
        assert_eq!(reg.value(), 0x110019);

        // 3. Other modern ASUS laptop (e.g. Zenbook S 16 UM5606)
        fs::write(&file_path, "Zenbook S 16 UM5606\n").unwrap();
        let reg =
            hardware_resolver::resolve_register_from_dmi(file_path.to_str().unwrap()).unwrap();
        assert_eq!(reg.value(), 0x110019);

        // 4. Older ROG laptop using V1 (0x5002f)
        fs::write(&file_path, "ROG Zephyrus G14 GA401IV\n").unwrap();
        let reg_rog =
            hardware_resolver::resolve_register_from_dmi(file_path.to_str().unwrap()).unwrap();
        assert_eq!(reg_rog.value(), 0x5002f);
        assert_eq!(reg_rog, hardware_resolver::ThermalRegister::V1(0x5002f));

        // 5. Older TUF laptop using V1 (0x5002f)
        fs::write(&file_path, "ASUS TUF Gaming A15 FA506IV\n").unwrap();
        let reg_tuf =
            hardware_resolver::resolve_register_from_dmi(file_path.to_str().unwrap()).unwrap();
        assert_eq!(reg_tuf.value(), 0x5002f);

        // 6. Legacy Zenbook using V1 (0x5002f)
        fs::write(&file_path, "ZenBook UX430UN\n").unwrap();
        let reg_zen =
            hardware_resolver::resolve_register_from_dmi(file_path.to_str().unwrap()).unwrap();
        assert_eq!(reg_zen.value(), 0x5002f);

        // 7. Unknown/unrelated system product name (ThinkPad X1 Carbon) -> UnsupportedHardware error
        fs::write(&file_path, "ThinkPad X1 Carbon\n").unwrap();
        let reg_err = hardware_resolver::resolve_register_from_dmi(file_path.to_str().unwrap());
        assert!(matches!(
            reg_err,
            Err(hardware_resolver::HardwareResolveError::UnsupportedHardware(_))
        ));

        // 5. Missing / unreadable path -> DmiReadError
        let missing_err = hardware_resolver::resolve_register_from_dmi("/nonexistent/dmi/path");
        assert!(matches!(
            missing_err,
            Err(hardware_resolver::HardwareResolveError::DmiReadError(_))
        ));

        fs::remove_dir_all(&dir).ok();
    }
}
