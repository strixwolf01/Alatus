# ASUS Vivobook S 15 Keyboard Lighting (ITE5570 LampArray) Specification

## 1. Overview
The ASUS Vivobook S 15 OLED (S5506MA) features an integrated single-zone RGB backlit keyboard driven by an **ITE5570** Embedded Controller. Unlike legacy ASUS ROG keyboards which used vendor-specific USB endpoint control transfers (`0x5A` direct packet blasts), modern ASUS Vivobook laptops implement the standard **HID LampArray** specification (USB HID Usage Page `0x0059`) over I2C-HID.

## 2. Device Identification

| Property | Value |
| :--- | :--- |
| **Bus Type** | I2C (`0x0018`) |
| **Vendor ID (VID)** | `0x0B05` (ASUSTeK Computer Inc.) |
| **Product ID (PID)** | `0x19B6` (ITE5570 LampArray Controller) |
| **HID Name** | `ITE5570:00 0B05:19B6` |
| **Physical Path** | `i2c-ITE5570:00` |
| **Sysfs Path** | `/sys/class/hidraw/hidraw*` (typically `hidraw1`) |
| **HID Usage Page** | `0x0059` (Lighting and Illumination / LampArray) |

## 3. Protocol Architecture (HID LampArray Standard)

The HID LampArray specification defines standard report structures for querying lamp attributes and sending color/intensity updates.

### Key Report IDs & Structures

#### 1. LampArray Attributes Report (Feature Report, Read-Only)
- **Report ID**: `0x01` (Attributes Request / Response)
- Queries number of lamps (keyboard backlight zone count = `1`), bounding box dimensions, and LampArray kind.

#### 2. Lamp Multi-Update Report (Feature / Output Report)
- Used to set RGB values for lamps.
- Format:
  - Byte 0: Report ID (`0x04` or device-specific multi-lamp report)
  - Byte 1: Lamp Count (`0x01` for single zone)
  - Byte 2: Lamp ID (`0x00`)
  - Byte 3: Red (`0x00` - `0xFF`)
  - Byte 4: Green (`0x00` - `0xFF`)
  - Byte 5: Blue (`0x00` - `0xFF`)
  - Byte 6: Intensity / Alpha (`0x00` - `0xFF`)

#### 3. ASUS Hardware Mode / Animation Fallback Report
For hardware-managed effects (Static, Breathing, Rainbow Cycle) without continuous software frame streaming:
- **Report ID**: `0x5A`
- Packet size: 17 bytes
- Byte 0: `0x5A`
- Byte 1: Command / Sub-ID (`0xBA` for keyboard lighting)
- Byte 2: Mode (`0x00`: Static, `0x01`: Breathing, `0x02`: Strobe, `0x03`: Rainbow cycle)
- Byte 3: Red (`0x00` - `0xFF`)
- Byte 4: Green (`0x00` - `0xFF`)
- Byte 5: Blue (`0x00` - `0xFF`)
- Byte 6: Speed (`0x00`: Slow, `0x01`: Normal, `0x02`: Fast)
- Byte 7: Brightness (`0` - `3`)

### Brightness Levels
The keyboard backlight supports 4 discrete brightness levels:
- `0`: Off
- `1`: Low (33%)
- `2`: Medium (66%)
- `3`: High (100%)

## 4. Hardware Reference & Licensing Notes
- **Reference**: `vrgb` (ASUS LampArray reverse engineering, `vrgb/devices/ite5570.py`). Source: [vrgb-dev/vrgb](https://github.com/vrgb-dev/vrgb).
  - Facts extracted: ITE5570 VID `0x0B05`, PID `0x19B6`, report layout, LampArray usage page `0x0059`.
  - License: MIT.
- **Reference**: USB HID Point of Sale / Lighting and Illumination Specification (LampArray v1.0).
- **Reference**: `asusctl` aura / lighting modules (`daemon/src/ctrl_aura.rs`).
