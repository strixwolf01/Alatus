# ASUS Vivobook S 15 Hardware Topology & DMI Profile

## 1. Verified Target Machine DMI Identifiers
Extracted from live system `/sys/class/dmi/id/`:

- **System Vendor (`sys_vendor`)**: `ASUSTeK COMPUTER INC.`
- **Product Name (`product_name`)**: `ASUS Vivobook S 15 S5506MA_S5506MA`
- **Board Name (`board_name`)**: `S5506MA`
- **BIOS Version (`bios_version`)**: `S5506MA.318`

## 2. Device Tree & Node Mapping

```
/sys/
├── class/
│   ├── power_supply/
│   │   ├── BAT0/
│   │   │   ├── charge_control_end_threshold (supported: 60, 80, 100)
│   │   │   ├── status
│   │   │   ├── capacity
│   │   │   ├── energy_now
│   │   │   └── energy_full
│   │   └── ADP0/
│   │       └── online
│   ├── hidraw/
│   │   └── hidraw1 -> ITE5570 LampArray (0B05:19B6)
│   └── dmi/id/
│       ├── sys_vendor
│       ├── product_name
│       └── board_name
├── firmware/
│   └── acpi/
│       ├── platform_profile (choices: quiet balanced performance)
│       └── platform_profile_choices
└── devices/platform/
    └── asus-nb-wmi/
        ├── throttle_thermal_policy (0: balanced, 1: performance, 2: quiet)
        ├── hwmon/hwmon*/
        │   ├── fan1_input (CPU fan RPM)
        │   └── fan2_input (GPU fan RPM)
        └── input/input*/ (Asus WMI hotkeys)
```
