# ASUS Vivobook S 15 Battery Hardware Specification

## 1. Overview
The ASUS Vivobook S 15 OLED (S5506MA) exposes battery charge control and telemetry through standard Linux ACPI / power supply sysfs interfaces provided by the kernel's ACPI battery driver and `asus-wmi` / `asus-nb-wmi`.

- **Primary sysfs path**: `/sys/class/power_supply/BAT0`
- **AC Adapter path**: `/sys/class/power_supply/ADP0`

## 2. Charge Threshold Control
On ASUS modern laptops, battery health charging limits are controlled via the `charge_control_end_threshold` attribute.

| Attribute | Path | Permissions | Valid Values | Description |
| :--- | :--- | :--- | :--- | :--- |
| **End Threshold** | `/sys/class/power_supply/BAT0/charge_control_end_threshold` | `rw-r--r--` (root writable) | `60`, `80`, `100` | Target charge stop percentage. Writing `80` stops charging at 80%. |
| **Start Threshold** | `/sys/class/power_supply/BAT0/charge_control_start_threshold` | N/A (unsupported) | N/A | Not supported on ASUS hardware (firmware uses fixed hysteresis below end threshold). |

> [!NOTE]
> Some older Linux kernels or non-ASUS hardware exposed `charge_control_limit_max`. On kernel 6.x+ for ASUS laptops, `charge_control_end_threshold` is the canonical attribute. Alatus checks `charge_control_end_threshold` first, falling back to `charge_control_limit_max` if necessary.

## 3. Battery Telemetry & Health

| Attribute | Sysfs Path | Units | Calculation / Meaning |
| :--- | :--- | :--- | :--- |
| **Status** | `/sys/class/power_supply/BAT0/status` | String | `Charging`, `Discharging`, `Not charging`, `Full` |
| **Capacity** | `/sys/class/power_supply/BAT0/capacity` | Percentage (`%`) | Current State of Charge (0–100) |
| **Energy Now** | `/sys/class/power_supply/BAT0/energy_now` | Micro-watt hours (`µWh`) | Current remaining energy |
| **Energy Full** | `/sys/class/power_supply/BAT0/energy_full` | Micro-watt hours (`µWh`) | Full charge capacity with current wear |
| **Energy Full Design** | `/sys/class/power_supply/BAT0/energy_full_design` | Micro-watt hours (`µWh`) | Original factory capacity |
| **Power Now** | `/sys/class/power_supply/BAT0/power_now` | Micro-watts (`µW`) | Instantaneous discharge / charge rate |
| **Voltage Now** | `/sys/class/power_supply/BAT0/voltage_now` | Micro-volts (`µV`) | Current battery pack voltage |

### Battery Health Formula
$$\text{Health (\%)} = \min\left(100, \left\lfloor \frac{\text{energy\_full}}{\text{energy\_full\_design}} \times 100 \right\rfloor\right)$$

## 4. Hardware Reference & Licensing Notes
- **Reference**: `asusctl` battery controller (`asusctl/src/battery.rs`). Source: [OpenGamingCollective/asusctl](https://github.com/OpenGamingCollective/asusctl).
- **License**: MPL-2.0 / GPL-3.0 compatible. Facts extracted: sysfs attribute naming and standard threshold steps (60%, 80%, 100%).
- **Reference**: `Ayuz` battery care UI (`src/battery.rs`). Source: [Traciges/Ayuz](https://github.com/Traciges/Ayuz).
- **License**: GPL-3.0.
