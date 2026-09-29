# Alatus

A modern, fast, and capability-driven Linux hardware control suite for ASUS laptops written in Rust.

## Features
- **Battery Care**: Charge limit control (60%, 80%, 100%) and health reporting.
- **Thermal & Fans**: Dynamic `platform_profile` (Quiet, Balanced, Performance) and full-speed fan boost via `asus-nb-wmi` DebugFS.
- **RGB Lighting**: Full support for ITE5570 HID LampArray single-zone backlit keyboards.
- **Hotkeys**: Seamless integration with ASUS WMI hotkeys (`Fn+F`, fan toggle) emitting D-Bus signals for desktop OSD.
- **Architecture**: Decoupled, modular crates with root daemon (`alatusd`), user session agent (`alatus-session`), CLI (`alatus-cli`), and Slint GUI (`alatus-gui`).

## License
GPL-3.0-or-later
