# Alatus

[![License: GPL-3.0-or-later](https://img.shields.io/badge/License-GPL--3.0--or--later-blue.svg)](LICENSE)
[![Release: 2.0.0](https://img.shields.io/badge/release-2.0.0-blue.svg)](https://github.com/strixwolf01/Alatus/releases)
[![Rust: 2024 Edition](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org)

**Alatus** is the definitive, memory-safe ASUS Hardware Control Center engineered in Rust for modern ASUS laptops running Linux. It unifies low-level platform thermal management, keyboard backlighting, OLED Flicker-Free dimming, battery charge thresholds, and an intuitive Material 3 desktop application into a cohesive, zero-overhead architecture.

---

## Critical Prerequisite: Linux Kernel Lockdown Notice

> [!CAUTION]
> **Kernel Lockdown Mode Blocks Hardware DebugFS Registers**:
> Alatus achieves direct ACPI register orchestration by communicating with platform interfaces including `/sys/kernel/debug/asus-nb-wmi/` (`dev_id`, `dsts`, `devs`).
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

## Supported Hardware & Architecture Scope

> [!IMPORTANT]
> **Validated Reference Platform vs. Extensible Architecture**:
> - **Validated Reference Platform**: The **ASUS Vivobook S 15 OLED (S5506MA)** serves as the primary hardware-verified development platform, with 100% feature verification across all subsystems (ITE5570 LampArray RGB, ACPI WMI DebugFS thermal profiles with Full Speed 8100 RPM, OLED Care flicker-free dimming, battery charge limits, and hardware Fn+F hotkey handling).
> - **Architecture Scope**: Alatus employs a modular, profile-driven architecture designed to orchestrate modern ASUS laptops across the **Vivobook**, **Zenbook**, **ROG**, **TUF Gaming**, and **ProArt Studiobook** series.
> - **Extensibility Notice**: Alatus does **not** assume identical firmware interfaces on every laptop out of the box. Non-reference models leverage declarative device profiles (`assets/devices/*.toml`) and dynamic tri-state capability probing (`Supported`, `Unsupported`, `Unavailable`). Subsystem availability varies depending on the specific EC registers, WMI methods, and kernel driver modules exposed by each laptop model. Non-ASUS platforms (Lenovo, Dell, HP, etc.) are detected via DMI and safely rejected.

| Platform Tier | Validation Status | Laptop Models / Series | Subsystem Availability |
| :--- | :--- | :--- | :--- |
| **Reference Platform** | **Fully Verified** | ASUS Vivobook S 15 OLED (`S5506MA`) | Full 4 Thermal Modes (incl. Full Speed), Battery Thresholds, ITE5570 RGB, OLED Care, Hardware Fn+F Hotkey |
| **Declarative Profiles** | **Profile-Driven** | ASUS Zenbook (`UM5302`), ROG (`G14`), TUF, Vivobook | Available subsystems adapt dynamically to device profile (`assets/devices/*.toml`) and kernel drivers |
| **Generic ASUS Fallback** | **Runtime Detection** | Other ASUS Laptop Series | Standard ACPI platform profiles and sysfs battery charge limits where supported by kernel |
| **Non-ASUS Hardware** | *Unsupported* | Generic PC hardware | Safely rejected on initialization |

---

## Core Features

- **ACPI Platform Thermal Management**:
  Direct hardware register orchestration, completely bypassing `power-profiles-daemon` (PPD) interference.
  - **Quiet (0)**: Ultra-low fan acoustics (~1800–2400 RPM) with power capping.
  - **Balanced (1)**: Dynamic acoustic and thermal balancing (~2800–4200 RPM).
  - **Performance (2)**: High-load thermal envelope for compiling, gaming, and rendering (~4800–5600 RPM).
  - **Full Speed (3)**: Unlocks the hardware maximum fan ceiling (~8100 RPM) for sustained compute workloads.
- **Hardware `Fn+F` Evdev Hotkey Listener**:
  Native multi-device evdev listener capturing platform thermal switch hotkeys with instant desktop OSD notifications.
- **Battery Health & Charge Thresholds**:
  Configurable charge limits (50–100%) to prolong battery lifespan for desk-bound and travel workflows.
- **OLED Care & Flicker-Free Dimming**:
  Cumulative screen-on tracking, conditioning pixel refresh sweeps, and hardware-level flicker-free dimming for OLED displays.
- **Keyboard Backlight & RGB Synchronization**:
  Keyboard backlight controls, brightness stepping, inactivity timeout policies, and desktop accent color synchronization.
- **Single-Instance Desktop Architecture (`alatus-gui`)**:
  Session D-Bus single-instance application guard (`io.strixwolf.alatus.Gui`) preventing redundant processes and duplicate tray icons. Re-launching Alatus instantly un-minimizes and raises the existing window.
- **FreeDesktop StatusNotifierItem & DBusMenu Tray**:
  Universal system tray integration supporting minimize-to-tray, quick thermal mode selection, pixel refresh triggering, and clean application exit across KDE Plasma, GNOME, XFCE, and Wayland status bars (Waybar).
- **Dedicated System Info & Hardware Diagnostics**:
  Integrated hardware testing suite verifying platform thermal profiles, backlight controllers, battery health telemetry (cycles, degradation %, charge wattage), and system board serials.

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
│  State Reconciliation (tokio::select!)         │   │
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
