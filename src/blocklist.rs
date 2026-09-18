//! The curated list of what "them" means. Ships inside the binary; a daily
//! fetch from `BLOCKLIST_URL` can replace it (same format, same rules).

use std::collections::BTreeMap;

use serde::Deserialize;

pub const EMBEDDED: &str = include_str!("../assets/blocklist.toml");
pub const BLOCKLIST_URL: &str =
    "https://raw.githubusercontent.com/rickhallett/tto/main/assets/blocklist.toml";

#[derive(Debug, Deserialize)]
pub struct Blocklist {
    pub version: u32,
    pub categories: BTreeMap<String, Category>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Category {
    pub title: String,
    pub blurb: String,
    pub domains: Vec<String>,
    /// Exact executable basenames (CLIs, servers).
    pub processes: Vec<String>,
    /// App bundle names; matched as a path component (`…/ChatGPT.app/…`).
    pub apps: Vec<String>,
}

impl Blocklist {
    pub fn embedded() -> Self {
        toml::from_str(EMBEDDED).expect("embedded blocklist is valid")
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let list: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        list.validate()?;
        Ok(list)
    }

    /// The precision rules from the top of blocklist.toml, enforced.
    fn validate(&self) -> Result<(), String> {
        for (name, cat) in &self.categories {
            for d in &cat.domains {
                if d.contains('/') || d.contains('*') || d.contains(' ') || !d.contains('.') {
                    return Err(format!("{name}: bad domain {d:?}"));
                }
            }
            for p in cat.processes.iter().chain(&cat.apps) {
                if p.contains('/') || p.contains('*') || p.is_empty() {
                    return Err(format!("{name}: bad process {p:?}"));
                }
            }
        }
        Ok(())
    }

    pub fn names(&self) -> Vec<&str> {
        self.categories.keys().map(String::as_str).collect()
    }

    /// Union of the selected categories. Unknown names are ignored.
    pub fn select(&self, names: &[String]) -> Category {
        let mut out = Category::default();
        for (name, cat) in &self.categories {
            if names.iter().any(|n| n == name || n == "everything") {
                out.domains.extend(cat.domains.iter().cloned());
                out.processes.extend(cat.processes.iter().cloned());
                out.apps.extend(cat.apps.iter().cloned());
            }
        }
        for v in [&mut out.domains, &mut out.processes, &mut out.apps] {
            v.sort();
            v.dedup();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_is_valid_and_precise() {
        let list = Blocklist::parse(EMBEDDED).unwrap();
        assert!(list.categories.contains_key("chat"));
        assert!(list.categories.contains_key("coding"));
        assert!(list.categories.contains_key("local"));
        // The rule that matters most: never a whole provider.
        for cat in list.categories.values() {
            for d in &cat.domains {
                assert_ne!(d, "googleapis.com");
                assert_ne!(d, "google.com");
                assert_ne!(d, "microsoft.com");
                assert_ne!(d, "github.com");
            }
        }
    }

    #[test]
    fn everything_selects_all() {
        let list = Blocklist::embedded();
        let all = list.select(&["everything".into()]);
        let chat = list.select(&["chat".into()]);
        assert!(all.domains.len() > chat.domains.len());
        assert!(all.processes.contains(&"ollama".to_string()));
    }
}
