# Alatus

[![License: GPL-3.0-or-later](https://img.shields.io/badge/License-GPL--3.0--or--later-blue.svg)](LICENSE)
[![Release: 1.0.0](https://img.shields.io/badge/release-1.0.0-blue.svg)](https://github.com/strixwolf01/Alatus/releases)
[![Rust: 2024 Edition](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org)

**Alatus** is an authoritative, memory-safe hardware orchestration suite engineered in Rust for ASUS laptops running Linux. It unifies low-level ACPI WMI DebugFS thermal management, ITE5570 LampArray keyboard backlighting, OLED Flicker-Free dimming, battery charge thresholds, and an intuitive Material 3 desktop application into a cohesive, zero-overhead architecture.

---

## Critical Prerequisite: Linux Kernel Lockdown Notice

> [!CAUTION]
> **Kernel Lockdown Mode Blocks Hardware DebugFS Registers**:
> Alatus achieves direct, pure ACPI register orchestration by reading and writing to `/sys/kernel/debug/asus-nb-wmi/` (`dev_id`, `dsts`, `devs`).
>
> When Linux **Kernel Lockdown** mode is active (enforced by default under UEFI Secure Boot with `lockdown=integrity` or `lockdown=confidentiality`), the Linux kernel strictly prohibits all processes—**including root and `systemd` system services**—from accessing DebugFS nodes.
>
> Under Lockdown, hardware thermal profile switches, fan RPM controls, and full-speed fan modes will fail with `Permission denied (os error 13)`.

### Actionable Remediation
To enable Alatus to orchestrate hardware registers, you must allow DebugFS access using one of the following methods:

1. **Option A: Boot with `lockdown=none` (Recommended)**:
   Add `lockdown=none` to your kernel command line via GRUB or systemd-boot:
   - For GRUB: Edit `/etc/default/grub`, append `lockdown=none` to `GRUB_CMDLINE_LINUX_DEFAULT`, then run `sudo grub-mkconfig -o /boot/grub/grub.cfg`.
   - For systemd-boot: Add `lockdown=none` to the options line of your active boot loader entry.
2. **Option B: Disable UEFI Secure Boot**:
   Reboot into your laptop's UEFI BIOS settings and set **Secure Boot** to **Disabled**.
3. **Option C: Disable MOK Validation**:
   Run `sudo mokutil --disable-validation` and follow the prompt to set a temporary password. Reboot and complete the MOK management screen to disable kernel signature enforcement while leaving UEFI Secure Boot active.

---

## Verified & Supported Hardware

> [!IMPORTANT]
> **Hardware Support Scope as of v1.0.0**:
> Official hardware verification, continuous benchmarking, and active hardware testing for Alatus `v1.0.0` is strictly scoped to:
>
> **`ASUS Vivobook S 15 OLED (S5506MA)`**
>
> **Upcoming Multi-Device Roadmap**:
> Generic fallback hardware providers and broader ASUS ROG/TUF gaming laptop integration are planned for subsequent minor releases. This future work may use an `asusd` / `asusctl` D-Bus bridge; Alatus does not currently depend on either project. Other manufacturers' laptops (Lenovo, Dell, HP, Framework) are detected via DMI and safely rejected.

| Target Platform | Validation Status | Validated Models | Feature Support |
| :--- | :--- | :--- | :--- |
| **ASUS Vivobook S / Zenbook OLED** | **Officially Verified** | `S5506MA` (Vivobook S 15 OLED) | 4 ACPI Thermal Modes (incl. Full Speed 8100 RPM), Battery Thresholds, ITE5570 LampArray RGB, OLED Care, Auto Refresh, Hardware Fn+F Hotkey |
| **ASUS ROG & TUF Gaming** | *Planned roadmap integration* | Zephyrus, Strix, TUF Gaming | *Future integration may use an external `asusctl`/`asusd` D-Bus bridge; no current dependency* |
| **Non-ASUS Hardware** | *Unsupported* | Generic PC hardware | Safely rejected on initialization |

---

## Core Features

- **Pure ACPI WMI DebugFS Thermal Management**:
  Direct hardware register orchestration via `/sys/kernel/debug/asus-nb-wmi/`, completely bypassing `power-profiles-daemon` (PPD) interference.
  - **Quiet (0)**: Ultra-low fan acoustics (~1800–2400 RPM) with power capping.
  - **Balanced (1)**: Dynamic acoustic and thermal balancing (~2800–4200 RPM).
  - **Performance (2)**: High-load thermal envelope for compiling and rendering (~4800–5600 RPM).
  - **Full Speed (3)**: Unlocks the hardware maximum fan ceiling (~8100 RPM) for sustained compute workloads.
- **Hardware `Fn+F` Evdev Hotkey Listener**:
  Native multi-device evdev listener capturing ASUS WMI hotkey events (including ASUS keycode `482`) with instant desktop OSD notifications.
- **Single-Instance Desktop Architecture (`alatus-gui`)**:
  Session D-Bus single-instance application guard (`io.strixwolf.alatus.Gui`) preventing redundant processes and duplicate tray icons. Re-launching Alatus instantly un-minimizes and raises the existing window.
- **FreeDesktop StatusNotifierItem & DBusMenu Tray**:
  Universal system tray integration supporting minimize-to-tray, quick thermal mode selection, pixel refresh triggering, and clean application exit across KDE Plasma, GNOME, XFCE, and Wayland status bars (Waybar).
- **Dedicated System Info & Hardware Diagnostics**:
  Integrated hardware testing suite verifying ASUS WMI DebugFS, ITE5570 USB HID, battery health telemetry (cycles, degradation %, charge wattage), and system board serials.
- **Flicker-Free OLED Care & Conditioning**:
  Hardware-safe flicker-free brightness dimming and automated subpixel conditioning cycles to mitigate OLED burn-in without PWM eye strain.
- **ITE5570 LampArray Single-Zone RGB Lighting**:
  Direct USB HID feature report control with real-time XDG Desktop Portal system accent color synchronization.

---

## Architecture Overview

```text
┌────────────────────────────────────────────────────────────────────────┐
│                   alatus (CLI) & alatus-gui (Slint GUI)                │
│       Status / Live TUI Monitor / Waybar JSON / Profile Switching      │
└───────────────────┬────────────────────────────────┬───────────────────┘
                    │ Session D-Bus                  │ System D-Bus
                    │ (io.strixwolf.alatus.Session)  │ (io.strixwolf.alatus.Daemon)
┌───────────────────▼────────────────────────────┐   │
│         alatus-session (User Agent)            │   │
│  Event-Driven Architecture with Periodic       │   │
│  State Reconciliation (tokio::select!)          │   │
├────────────────────────────────────────────────┤   │
│ • Udev Netlink Hooks + Fail-Safe State Ticker  │   │
│ • GNOME Mutter & KDE Display Refresh Switching │   │
│ • Flicker-Free OLED Dimming & Pixel Refresh    │   │
│ • XDG Desktop Portal Accent Color Sync         │   │
│ • Desktop OSD Notifications for Thermal Modes  │   │
└────────────────────────────────────────────────┘   │
                                                     │
┌────────────────────────────────────────────────────▼───────────────────┐
│                    alatusd (Root Daemon)                               │
│       Sandboxed Systemd Service with Polkit Policy Authorization       │
├────────────────────────────────────┬───────────────────────────────────┤
│    ASUS ACPI DebugFS Profiles      │    ITE5570 LampArray HID RGB      │
│    (Balanced, Quiet, Perf, Full)   │    (/dev/hidraw* Feature Reports) │
├────────────────────────────────────┼───────────────────────────────────┤
│    Battery Health Thresholds       │    Evdev ASUS Hotkey Listener     │
│    (/sys/class/power_supply/*/...) │    (Fn+F / keycode 482 -> OSD)    │
└────────────────────────────────────┴───────────────────────────────────┘
```

---

## Installation & Build Instructions

### Prerequisites
Install the required system compilation dependencies:

```bash
# Fedora / RHEL
sudo dnf install -y gcc gcc-c++ pkgconf-pkg-config fontconfig-devel freetype-devel \
    libxkbcommon-devel mesa-libGL-devel dbus-devel systemd-devel udev

# Ubuntu / Debian
sudo apt-get update && sudo apt-get install -y pkg-config \
    libfontconfig1-dev libfreetype6-dev libxkbcommon-dev libgl1-mesa-dev \
    libdbus-1-dev libudev-dev
```

### Build from Source
Compile all release binaries with the Slint GUI dashboard enabled:

```bash
cargo build --release --bins --features gui
```

Compiled binaries will be available under `target/release/`:
- `target/release/alatus` (CLI management tool)
- `target/release/alatusd` (Privileged root daemon)
- `target/release/alatus-session` (User desktop agent)
- `target/release/alatus-gui` (Material 3 desktop application)

### Generate Installation Packages

#### RPM Package (Fedora / RHEL / openSUSE)
```bash
cargo install cargo-generate-rpm --locked
cargo generate-rpm
```
Installs the generated `.rpm` from `target/generate-rpm/` via `sudo rpm -ivh target/generate-rpm/alatus-*.rpm`.

#### Debian Package (Ubuntu / Debian / Pop!_OS)
```bash
cargo install cargo-deb --locked
cargo deb --no-build
```
Installs the generated `.deb` from `target/debian/` via `sudo dpkg -i target/debian/alatus_*.deb`.

---

## Quick Start & Service Setup

1. **Enable and Start Root Daemon**:
   ```bash
   sudo systemctl daemon-reload
   sudo systemctl enable --now alatusd.service
   ```

2. **Enable and Start User Session Agent**:
   ```bash
   systemctl --user daemon-reload
   systemctl --user enable --now alatus-session.service
   ```

3. **Launch the GUI Dashboard**:
   ```bash
   alatus-gui
   ```

4. **CLI Quick Reference**:
   ```bash
   # View system and hardware telemetry
   alatus status

   # Cycle thermal modes (Quiet -> Balanced -> Performance -> Full)
   alatus mode cycle

   # Set battery charge threshold to 80%
   alatus charge-limit 80

   # Set keyboard backlight color
   alatus rgb color 61 174 233
   ```

---

## Acknowledgments

- **[vrgb](https://github.com/vrgb-dev/vrgb)**: For foundational hardware research, reverse engineering, and documentation of the ITE5570 LampArray USB HID protocol.
- **[asus-5606-fan-state](https://github.com/ThatOneCalculator/asus-5606-fan-state)**: For early research and reference implementation of direct ACPI WMI DebugFS fan register sequencing.

---

## License

Alatus is free software released under the [GNU General Public License v3.0 or later](LICENSE).
Copyright (C) 2026 Alatus Contributors.
