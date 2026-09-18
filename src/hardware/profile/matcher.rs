// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

use crate::hardware::profile::model::DeviceProfile;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Error conditions encountered during DMI hardware identification.
#[derive(Debug, Error)]
pub enum MatcherError {
    #[error("I/O error reading DMI sysfs: {0}")]
    Io(#[from] std::io::Error),

    #[error("DMI product_name missing or empty at {0}")]
    MissingProduct(PathBuf),
}

/// Helper for matching host physical hardware DMI strings against declared device profiles.
pub struct DmiMatcher;

impl DmiMatcher {
    /// Reads `product_name` and optionally `board_name` from the specified sysfs DMI path.
    /// Default system path is typically `/sys/class/dmi/id`.
    pub fn read_dmi(sysfs_root: &Path) -> Result<(String, Option<String>), MatcherError> {
        let product_path = sysfs_root.join("product_name");
        let product_raw = fs::read_to_string(&product_path)?;
        let product = product_raw.trim().to_string();
        if product.is_empty() {
            return Err(MatcherError::MissingProduct(product_path));
        }

        let board_path = sysfs_root.join("board_name");
        let board = fs::read_to_string(board_path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        Ok((product, board))
    }

    /// Matches the host's `product` and optional `board` name against a set of device profiles.
    ///
    /// Priority is given to profiles that match both product and board name, followed by
    /// product-only matches.
    pub fn match_profile<'a>(
        profiles: &'a [DeviceProfile],
        product: &str,
        board: Option<&str>,
    ) -> Option<&'a DeviceProfile> {
        let prod_clean = product.trim();
        let board_clean = board.map(|b| b.trim());

        let matches_pattern = |pat: &str, target: &str| -> bool {
            let trimmed = pat.trim_end_matches('*');
            pat.eq_ignore_ascii_case(target)
                || target.contains(pat)
                || (!trimmed.is_empty()
                    && (target
                        .to_ascii_lowercase()
                        .contains(&trimmed.to_ascii_lowercase())
                        || trimmed.eq_ignore_ascii_case(target)))
        };

        // First pass: exact or substring match for BOTH product and board
        for profile in profiles {
            let product_matched = profile
                .device
                .match_product
                .iter()
                .any(|p| matches_pattern(p, prod_clean));

            let board_matched = match (&profile.device.match_board, board_clean) {
                (Some(boards), Some(b_clean)) => boards.iter().any(|b| matches_pattern(b, b_clean)),
                _ => false,
            };

            if product_matched && board_matched {
                return Some(profile);
            }
        }

        // Second pass: match profiles that do not require a specific board name
        for profile in profiles {
            let product_matched = profile
                .device
                .match_product
                .iter()
                .any(|p| matches_pattern(p, prod_clean));

            if product_matched && profile.device.match_board.is_none() {
                return Some(profile);
            }
        }

        // Third pass: product match fallback even if board constraint was specified
        for profile in profiles {
            let product_matched = profile
                .device
                .match_product
                .iter()
                .any(|p| matches_pattern(p, prod_clean));

            if product_matched {
                return Some(profile);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::profile::model::{DeviceMeta, DeviceProfileCapabilities};
    use tempfile::tempdir;

    fn sample_profiles() -> Vec<DeviceProfile> {
        vec![
            DeviceProfile {
                device: DeviceMeta {
                    name: "ASUS Vivobook S 15 OLED".to_string(),
                    vendor: "ASUSTeK COMPUTER INC.".to_string(),
                    match_product: vec![
                        "S5506MA".to_string(),
                        "Vivobook_ASUSLaptop_S5506MA".to_string(),
                    ],
                    match_board: Some(vec!["S5506MA".to_string()]),
                },
                capabilities: DeviceProfileCapabilities::default(),
            },
            DeviceProfile {
                device: DeviceMeta {
                    name: "Generic ASUS Laptop".to_string(),
                    vendor: "ASUSTeK COMPUTER INC.".to_string(),
                    match_product: vec!["GenericBook".to_string()],
                    match_board: None,
                },
                capabilities: DeviceProfileCapabilities::default(),
            },
        ]
    }

    #[test]
    fn test_match_profile_exact_and_fallback() {
        let profiles = sample_profiles();

        // Exact match with board
        let matched = DmiMatcher::match_profile(&profiles, "S5506MA", Some("S5506MA"));
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().device.name, "ASUS Vivobook S 15 OLED");

        // Substring / alias product match with board
        let matched_alias = DmiMatcher::match_profile(
            &profiles,
            "Vivobook_ASUSLaptop_S5506MA_S5506MA",
            Some("S5506MA"),
        );
        assert!(matched_alias.is_some());
        assert_eq!(
            matched_alias.unwrap().device.name,
            "ASUS Vivobook S 15 OLED"
        );

        // Profile without board requirement
        let matched_generic = DmiMatcher::match_profile(&profiles, "GenericBook", None);
        assert!(matched_generic.is_some());
        assert_eq!(matched_generic.unwrap().device.name, "Generic ASUS Laptop");

        // Non-matching device
        let matched_none = DmiMatcher::match_profile(&profiles, "ThinkPad X1", Some("20XX"));
        assert!(matched_none.is_none());
    }

    #[test]
    fn test_read_dmi_from_mock_dir() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("product_name"), "S5506MA\n").unwrap();
        fs::write(tmp.path().join("board_name"), "S5506MA\n").unwrap();

        let (product, board) = DmiMatcher::read_dmi(tmp.path()).unwrap();
        assert_eq!(product, "S5506MA");
        assert_eq!(board.as_deref(), Some("S5506MA"));
    }

    #[test]
    fn test_read_dmi_missing_product() {
        let tmp = tempdir().unwrap();
        // product_name is absent
        assert!(matches!(
            DmiMatcher::read_dmi(tmp.path()),
            Err(MatcherError::Io(_))
        ));
    }

    #[test]
    fn test_read_dmi_empty_product() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("product_name"), "   \n").unwrap();
        assert!(matches!(
            DmiMatcher::read_dmi(tmp.path()),
            Err(MatcherError::MissingProduct(_))
        ));
    }
}
