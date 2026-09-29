# ASUS Vivobook S 15 Thermal & Fan Control Specification

## 1. Overview & Dual-Path Architecture
The ASUS Vivobook S 15 OLED (S5506MA) features dual cooling fans (`fan1` and `fan2`) managed by the ASUS ACPI EC firmware and `asus-nb-wmi`.

Alatus implements a dual-path control strategy:
1. **Primary Standard Path**: ACPI `platform_profile` and `throttle_thermal_policy`. Fully supported under Linux Kernel Lockdown / Secure Boot.
2. **Enhanced Full Speed Path**: Direct ACPI WMI EC register write via DebugFS (`/sys/kernel/debug/asus-nb-wmi/devs`) for unlocking 100% full fan RPM boost. Gracefully omitted if DebugFS is unavailable.

## 2. Standard ACPI Platform Profile
Path: `/sys/firmware/acpi/platform_profile`
Choices: `/sys/firmware/acpi/platform_profile_choices`

| Profile Name | `platform_profile` String | `throttle_thermal_policy` Integer | Behavior |
| :--- | :--- | :--- | :--- |
| **Quiet / Whisper** | `quiet` | `2` | Conservative fan acoustic curve, throttles power limits (PL1/PL2) for silence. |
| **Balanced / Standard** | `balanced` | `0` | Default factory dynamic fan curve and balanced TDP limits. |
| **Performance** | `performance` | `1` | Aggressive fan ramp, raised TDP limits, highest sustained performance. |

On Linux 6.x+, writing to `/sys/firmware/acpi/platform_profile` automatically updates `throttle_thermal_policy` in `asus-nb-wmi`. Alatus synchronizes both.

## 3. Fan Telemetry (hwmon)
Path: `/sys/devices/platform/asus-nb-wmi/hwmon/hwmon*/`

| Attribute | Sysfs Node | Format | Description |
| :--- | :--- | :--- | :--- |
| **Fan 1 Speed** | `fan1_input` | Integer (RPM) | Real-time RPM of primary CPU cooling fan. |
| **Fan 2 Speed** | `fan2_input` | Integer (RPM) | Real-time RPM of secondary GPU/system cooling fan. |
| **Fan 1 Label** | `fan1_label` | String (`cpu_fan`) | Fan identifier label. |
| **Fan 2 Label** | `fan2_label` | String (`gpu_fan`) | Fan identifier label. |

## 4. Full Speed Mode via ACPI DebugFS

### DebugFS Device Path
`/sys/kernel/debug/asus-nb-wmi/devs`

### Register & Value Mapping
Based on ACPI method `DEVS` in DSDT / `asus-5606-fan-state`:
- **Device Register ID**: `0x00110013` (ASUS WMI Fan Boost / Profile Register)
- **Normal Modes**:
  - Balanced: `0x00`
  - Performance: `0x01`
  - Quiet: `0x02`
- **Full Speed Boost Mode**:
  - Full Speed: `0x01` or specific register sequencing detailed below.

### Register Write Sequence
To write to `devs` via DebugFS:
```bash
# Format: echo "0x<DEVICE_ID> 0x<VALUE>" > /sys/kernel/debug/asus-nb-wmi/devs
echo "0x00110013 0x1" > /sys/kernel/debug/asus-nb-wmi/devs
```

### Kernel Lockdown & Secure Boot Constraints
When the Linux kernel boots with UEFI Secure Boot enabled, Linux Lockdown activates in `integrity` mode:
- `/sys/kernel/debug/*` access is restricted (returns `-EPERM` or `-EACCES`).
- **Alatus Strategy**:
  - Probe read/write access to `/sys/kernel/debug/asus-nb-wmi/devs` during daemon initialization.
  - If accessible: Expose `ThermalCapabilities::FULL_SPEED_FAN` and include `FullSpeed` in available profiles.
  - If blocked: Exclude `FULL_SPEED_FAN` capability. The daemon gracefully limits available profiles to `[Quiet, Balanced, Performance]`.

## 5. Hardware Reference & Licensing Notes
- **Reference**: `asus-5606-fan-state`. Source: [ThatOneCalculator/asus-5606-fan-state](https://github.com/ThatOneCalculator/asus-5606-fan-state).
  - Facts extracted: S5506 target compatibility, DebugFS `devs` interface, fan register `0x00110013`.
- **Reference**: `asusctl` thermal profiles & fan curves (`daemon/src/ctrl_fan_curves.rs`, `asusctl/src/thermal_policy.rs`). Source: [OpenGamingCollective/asusctl](https://github.com/OpenGamingCollective/asusctl).
- **Reference**: Linux kernel `drivers/platform/x86/asus-wmi.c`.
