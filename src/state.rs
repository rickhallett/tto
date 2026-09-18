//! The daemon's memory. Root-owned, mode 0600, under /var/db/tto.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const DIR: &str = "/var/db/tto";
pub const STATE: &str = "/var/db/tto/state.json";
pub const SOCKET: &str = "/var/run/tto.sock";
pub const HELPER: &str = "/Library/PrivilegedHelperTools/dev.oceanheart.tto";
pub const PLIST: &str = "/Library/LaunchDaemons/dev.oceanheart.tto.plist";
pub const PLIST_BACKUP: &str = "/var/db/tto/dev.oceanheart.tto.plist";
pub const LABEL: &str = "dev.oceanheart.tto";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct State {
    /// Unix seconds at which the block ends. 0 = no block.
    pub until: u64,
    pub categories: Vec<String>,
    /// Unix seconds the block started; for the status display.
    pub since: u64,
}

impl State {
    pub fn active(&self) -> bool {
        self.until > now()
    }

    pub fn remaining(&self) -> u64 {
        self.until.saturating_sub(now())
    }

    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = PathBuf::from(format!("{}.tmp", path.display()));
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, path)
    }

    /// Merge a new request into the current block. A block can only ever get
    /// longer or wider. There is deliberately no operation that shortens one.
    pub fn extend(&mut self, until: u64, categories: &[String]) {
        if !self.active() {
            *self = State {
                until,
                categories: categories.to_vec(),
                since: now(),
            };
            return;
        }
        self.until = self.until.max(until);
        for c in categories {
            if !self.categories.contains(c) {
                self.categories.push(c.clone());
            }
        }
    }
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extend_never_shortens() {
        let mut s = State::default();
        s.extend(now() + 3600, &["chat".into()]);
        let first = s.until;
        s.extend(now() + 60, &["local".into()]);
        assert_eq!(s.until, first);
        assert_eq!(s.categories, vec!["chat".to_string(), "local".to_string()]);
        s.extend(now() + 7200, &[]);
        assert!(s.until > first);
    }
}
