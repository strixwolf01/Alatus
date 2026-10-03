# Alatus ⚡

[![CI](https://github.com/strixwolf01/Alatus/actions/workflows/ci.yml/badge.svg)](https://github.com/strixwolf01/Alatus/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-2.0.0-blue.svg)](Cargo.toml)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0--or--later-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021%20edition-orange.svg)](https://www.rust-lang.org)

**Alatus** is a modern, modular, capability-driven Linux hardware control suite for ASUS laptops, engineered from the ground up in pure **Rust**.

It provides seamless, low-overhead hardware management for battery charging thresholds, thermal fan curves, keyboard RGB backlighting, OLED panel protection, and custom touchpad edge gestures.

---

## ✨ Features

### 🔋 Battery Care & Health
- **Charge Threshold Limiting**: Protect battery lifespan by enforcing custom charge limits (60%, 80%, or 100%) through ASUS ACPI sysfs nodes.
- **Hardware Telemetry**: Real-time readouts of battery level, charge health percentage, active power draw (Watts), and charging status.

### ❄️ Cooling & Thermal Fan Control
- **Dynamic Performance Profiles**: Effortless switching between ASUS platform profiles:
  - **Whisper (Quiet)**: Silent operation with relaxed power limits.
  - **Standard (Balanced)**: Everyday balance of acoustics and performance.
  - **Performance**: High sustained clocks and aggressive cooling.
  - **⚡ Full Speed**: 100% fan duty override utilizing ASUS WMI DebugFS sequencing.
- **Hardware Shortcut Integration**: Full hardware hotkey listening (`Fn + F`) with dynamic cycle switching and desktop notifications.
- **Fan Telemetry**: Live RPM monitors for both CPU and GPU fans.

### 🌈 Aura RGB Backlight & Lighting
- **Backlight Management**: Brightness control, custom color selection, and lighting mode switches (Static, Breathing, Rainbow, Strobe).
- **Desktop Accent Color Sync**: Automatically synchronize keyboard backlight colors with KDE Plasma / desktop accent palettes.
- **Keyboard Idle Timeout**: Power-saving automatic backlight turn-off timers (1m, 3m, 5m, 10m) on AC power and battery.

### 🖥️ Display & OLED Care
- **OLED Flicker-Free Dimming**: Hardware-friendly software brightness reduction to eliminate low-frequency PWM flicker.
- **Dynamic Refresh Rates**: Instant toggling between 60 Hz (Power Saving) and 120 Hz (High Smoothness).
- **Alatus OLED Care**:
  - **Pixel Refresh**: Screensaver stress cycle to prevent uneven pixel degradation.
  - **Target Mode (Dim Inactive)**: Automatically dims unfocused windows using compositor effects to preserve OLED life and conserve power.

### 👆 Touchpad Edge Gestures
- **Custom Edge Gestures**: Intuitive edge-swipe gestures on the touchpad for rapid control of volume, display brightness, and window navigation.

### 🔒 Security & Privacy
- **Privilege Separation**: Unprivileged user interface and session agents communicate with the root daemon via system D-Bus.
- **Polkit / pkexec Authentication**: Sensitive hardware telemetry (such as device serial numbers) requires explicit PolicyKit authentication to reveal.
- **Single-Instance Enforcement**: Smooth window raising and tray activation without duplicate process spawning.

---

## 🏛️ Architecture

Alatus follows a clean, decoupled micro-crate architecture:

| Component | Description |
| :--- | :--- |
| **`alatusd`** | Privileged system daemon running as root. Manages hardware sysfs/debugfs writes, exposes `org.alatus.Daemon` on system D-Bus, and validates Polkit policies. |
| **`alatus-session`** | Unprivileged user session agent. Implements the StatusNotifierItem (KDE/GNOME system tray), desktop OSD notifications, and touchpad input monitoring. |
| **`alatus-gui`** | Native desktop graphical application built with [Slint](https://slint.dev/) and native Wayland integration (`alatus-gui`). |
| **`alatus`** (`alatus-cli`) | High-performance command-line utility for scripting and headless automation. |
| **`alatus-core`** | Shared data structures, persistent configuration, and user settings models. |
| **`alatus-drivers`** | Hardware abstraction layer for ASUS ACPI, sysfs, debugfs, and evdev interfaces. |
| **`alatus-ipc`** | D-Bus interfaces and typed client proxies generated via `zbus`. |
| **`alatus-profile`** | Hardware DMI matcher and TOML device profile resolver. |

---

## 🚀 Getting Started

### Prerequisites

Ensure the following system development packages are installed (example for Fedora/RHEL):
```bash
sudo dnf install git gcc make rust cargo dbus-devel systemd-devel libudev-devel fontconfig-devel polkit
```

On Debian/Ubuntu:
```bash
sudo apt-get install git build-essential cargo libdbus-1-dev libudev-dev libfontconfig1-dev polkit-1-auth-agent
```

### Building from Source

Clone the repository and build in release mode:
```bash
git clone https://github.com/strixwolf01/Alatus.git
cd Alatus
cargo build --release
```

### Installation

Install system binaries, D-Bus service policies, udev rules, and icons:
```bash
sudo make install
```

Start the system daemon and enable user session integration:
```bash
sudo systemctl daemon-reload
sudo systemctl enable --now alatusd.service
systemctl --user enable --now alatus-session.service
```

---

## ⌨️ Command-Line Interface (CLI)

Alatus comes with a powerful CLI tool (`alatus`):

```bash
# Battery Management
alatus battery status
alatus battery limit 80

# Thermal & Fan Profiles
alatus thermal status
alatus thermal profile Balanced
alatus thermal profile FullSpeed

# Aura Lighting
alatus lighting status
alatus lighting brightness 3
alatus lighting color 00F0FF
alatus lighting mode Static
```

---

## 💻 Graphical Interface (GUI)

Launch the desktop interface from your application launcher or terminal:
```bash
alatus-gui
```

---

## 📜 License

This project is licensed under the **GNU General Public License v3.0 or later** (GPL-3.0-or-later). See [LICENSE](LICENSE) for details.
