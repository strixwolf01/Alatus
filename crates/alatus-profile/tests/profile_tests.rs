use alatus_profile::dmi::{DmiInfo, DmiReader, ProfileResolver, S5506MA_DEFAULT_PROFILE};
use alatus_profile::model::Profile;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_parse_valid_default_profile() {
    let profile = Profile::parse_toml(S5506MA_DEFAULT_PROFILE).expect("Failed to parse embedded profile");
    assert_eq!(profile.schema, 1);
    assert_eq!(profile.battery.default_charge_limit, 80);
    assert_eq!(profile.battery.supported_limits, vec![60, 80, 100]);
    assert_eq!(profile.thermal.fan_count, 2);
    assert!(profile.thermal.supports_full_speed);
}

#[test]
fn test_reject_unsupported_schema() {
    let invalid_schema = S5506MA_DEFAULT_PROFILE.replace("schema = 1", "schema = 2");
    let err = Profile::parse_toml(&invalid_schema).unwrap_err();
    assert!(err.to_string().contains("Unsupported profile schema version"));
}

#[test]
fn test_reject_invalid_default_limit() {
    let invalid_limit = S5506MA_DEFAULT_PROFILE.replace("default_charge_limit = 80", "default_charge_limit = 75");
    let err = Profile::parse_toml(&invalid_limit).unwrap_err();
    assert!(err.to_string().contains("not in supported limits"));
}

#[test]
fn test_dmi_matcher() {
    let profile = Profile::parse_toml(S5506MA_DEFAULT_PROFILE).unwrap();

    assert!(profile.dmi_match.matches(
        Some("ASUSTeK COMPUTER INC."),
        Some("ASUS Vivobook S 15 S5506MA_S5506MA"),
        Some("S5506MA"),
    ));

    // Case insensitivity
    assert!(profile.dmi_match.matches(
        Some("asustek computer inc."),
        Some("vivobook s 15 oled"),
        None,
    ));

    // Mismatched vendor
    assert!(!profile.dmi_match.matches(
        Some("Lenovo"),
        Some("ThinkPad"),
        Some("20XX"),
    ));
}

#[test]
fn test_mocked_dmi_reader() {
    let dir = tempdir().unwrap();
    let dmi_dir = dir.path().join("dmi/id");
    fs::create_dir_all(&dmi_dir).unwrap();

    fs::write(dmi_dir.join("sys_vendor"), "ASUSTeK COMPUTER INC.\n").unwrap();
    fs::write(dmi_dir.join("product_name"), "ASUS Vivobook S 15 S5506MA_S5506MA\n").unwrap();
    fs::write(dmi_dir.join("board_name"), "S5506MA\n").unwrap();
    fs::write(dmi_dir.join("bios_version"), "S5506MA.318\n").unwrap();

    let reader = DmiReader::with_sysfs_path(&dmi_dir);
    let info = reader.read_dmi().unwrap();

    assert_eq!(info.sys_vendor.as_deref(), Some("ASUSTeK COMPUTER INC."));
    assert_eq!(info.product_name.as_deref(), Some("ASUS Vivobook S 15 S5506MA_S5506MA"));
    assert_eq!(info.board_name.as_deref(), Some("S5506MA"));
    assert_eq!(info.bios_version.as_deref(), Some("S5506MA.318"));
}

#[test]
fn test_profile_resolver_override_order() {
    let dir = tempdir().unwrap();
    let etc_dir = dir.path().join("etc/alatus/profiles");
    let usr_dir = dir.path().join("usr/share/alatus/profiles");
    fs::create_dir_all(&etc_dir).unwrap();
    fs::create_dir_all(&usr_dir).unwrap();

    let custom_etc = S5506MA_DEFAULT_PROFILE
        .replace("Embedded default hardware mapping", "ETC Override mapping")
        .replace("default_charge_limit = 80", "default_charge_limit = 60");
    fs::write(etc_dir.join("custom.toml"), custom_etc).unwrap();

    let usr_profile = S5506MA_DEFAULT_PROFILE
        .replace("Embedded default hardware mapping", "USR Share mapping");
    fs::write(usr_dir.join("default.toml"), usr_profile).unwrap();

    let resolver = ProfileResolver::new(vec![etc_dir, usr_dir]);
    let dmi = DmiInfo {
        sys_vendor: Some("ASUSTeK COMPUTER INC.".into()),
        product_name: Some("ASUS Vivobook S 15 S5506MA_S5506MA".into()),
        board_name: Some("S5506MA".into()),
        bios_version: None,
    };

    let resolved = resolver.resolve(&dmi).expect("Should resolve profile");
    assert_eq!(resolved.battery.default_charge_limit, 60);
    assert_eq!(
        resolved.metadata.description.as_deref(),
        Some("ETC Override mapping for Vivobook S15 with ITE5570 LampArray keyboard")
    );
}

#[test]
fn test_profile_resolver_fallback_to_embedded() {
    let dir = tempdir().unwrap();
    let empty_dir = dir.path().join("empty");
    fs::create_dir_all(&empty_dir).unwrap();

    let resolver = ProfileResolver::new(vec![empty_dir]);
    let dmi = DmiInfo {
        sys_vendor: Some("ASUSTeK COMPUTER INC.".into()),
        product_name: Some("ASUS Vivobook S 15 S5506MA_S5506MA".into()),
        board_name: Some("S5506MA".into()),
        bios_version: None,
    };

    let resolved = resolver.resolve(&dmi).expect("Should fallback to embedded profile");
    assert_eq!(resolved.battery.default_charge_limit, 80);
}
