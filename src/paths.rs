//! Where things live. `TTO_PREFIX=/some/dir` relocates everything for tests
//! and for running the daemon unprivileged; the real install uses "".

use std::path::PathBuf;

pub struct Paths {
    pub prefix: PathBuf,
}

impl Paths {
    pub fn from_env() -> Self {
        Self {
            prefix: PathBuf::from(std::env::var_os("TTO_PREFIX").unwrap_or_default()),
        }
    }

    /// True when relocated: no root required, no immutable flags, no launchd.
    pub fn is_test(&self) -> bool {
        !self.prefix.as_os_str().is_empty()
    }

    fn p(&self, abs: &str) -> PathBuf {
        self.prefix.join(abs.trim_start_matches('/'))
    }

    pub fn state_dir(&self) -> PathBuf {
        self.p(crate::state::DIR)
    }
    pub fn state(&self) -> PathBuf {
        self.p(crate::state::STATE)
    }
    pub fn socket(&self) -> PathBuf {
        self.p(crate::state::SOCKET)
    }
    pub fn hosts(&self) -> PathBuf {
        self.p(crate::hosts::PATH)
    }
    pub fn helper(&self) -> PathBuf {
        self.p(crate::state::HELPER)
    }
    pub fn plist(&self) -> PathBuf {
        self.p(crate::state::PLIST)
    }
    pub fn plist_backup(&self) -> PathBuf {
        self.p(crate::state::PLIST_BACKUP)
    }
    pub fn blocklist_override(&self) -> PathBuf {
        self.state_dir().join("blocklist.toml")
    }
}
