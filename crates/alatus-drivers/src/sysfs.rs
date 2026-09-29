//! Sysfs path helpers and root abstractions for mocking.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SysfsRoot {
    root: PathBuf,
}

impl Default for SysfsRoot {
    fn default() -> Self {
        Self {
            root: PathBuf::from("/"),
        }
    }
}

impl SysfsRoot {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn resolve(&self, absolute_sysfs_path: impl AsRef<Path>) -> PathBuf {
        let p = absolute_sysfs_path.as_ref();
        if self.root == Path::new("/") {
            p.to_path_buf()
        } else {
            let stripped = p.strip_prefix("/").unwrap_or(p);
            self.root.join(stripped)
        }
    }
}
