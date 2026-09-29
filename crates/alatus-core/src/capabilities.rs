use bitflags::bitflags;
use serde::{Deserialize, Serialize};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct BatteryCapabilities: u32 {
        const CHARGE_LIMIT_CONFIGURABLE = 1 << 0;
        const DISCHARGE_RATE_REPORTING  = 1 << 1;
        const HEALTH_REPORTING          = 1 << 2;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ThermalCapabilities: u32 {
        const PLATFORM_PROFILE_STANDARD = 1 << 0;
        const FULL_SPEED_FAN            = 1 << 1;
        const CUSTOM_FAN_CURVES         = 1 << 2;
        const FAN_RPM_READBACK          = 1 << 3;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct LightingCapabilities: u32 {
        const BRIGHTNESS_CONTROL = 1 << 0;
        const STATIC_COLOR       = 1 << 1;
        const BUILTIN_EFFECTS    = 1 << 2;
        const PER_KEY_ADDRESSING = 1 << 3;
    }
}
