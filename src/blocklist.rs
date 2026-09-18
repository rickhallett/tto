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

/// Domains that are infrastructure for far more than chatbots. A list that
/// names one of these, or any suffix of one, is refused outright, whatever
/// its version says. Rule 1 of BLOCKLIST-POLICY.md, enforced.
pub const NEVER_BLOCK: &[&str] = &[
    "google.com",
    "googleapis.com",
    "gstatic.com",
    "googleusercontent.com",
    "youtube.com",
    "microsoft.com",
    "live.com",
    "office.com",
    "azure.com",
    "windows.net",
    "office365.com",
    "apple.com",
    "icloud.com",
    "amazonaws.com",
    "amazon.com",
    "cloudfront.net",
    "cloudflare.com",
    "github.com",
    "githubusercontent.com",
    "githubassets.com",
    "gitlab.com",
    "cursor.sh",
    "cursor.com",
    "vercel.app",
    "vercel.com",
    "netlify.app",
    "fastly.net",
    "akamai.net",
    "openai.com",
    "anthropic.com",
    "x.com",
    "twitter.com",
    "facebook.com",
    "meta.com",
    "slack.com",
    "notion.so",
    "zoom.us",
    "dropbox.com",
    "figma.com",
];

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
                // Hostname characters only: anything else (whitespace, TOML
                // escapes, a path) could smuggle extra lines into /etc/hosts.
                let hostname = !d.is_empty()
                    && d.contains('.')
                    && !d.starts_with('.')
                    && !d.ends_with('.')
                    && d.chars().all(|c| {
                        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-'
                    });
                if !hostname {
                    return Err(format!("{name}: bad domain {d:?}"));
                }
                if let Some(bad) = NEVER_BLOCK
                    .iter()
                    .find(|n| d == *n || n.ends_with(&format!(".{d}")))
                {
                    return Err(format!(
                        "{name}: {d:?} would block {bad}, which is infrastructure; blocklist refused"
                    ));
                }
            }
            for p in cat.processes.iter().chain(&cat.apps) {
                if p.is_empty() || p.chars().any(|c| c == '/' || c == '*' || c.is_control()) {
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
    fn refuses_smuggled_lines() {
        for bad in [
            "a.com\nb.com",
            "a.com\tb.com",
            "a.com b.com",
            "A.com",
            "a.com/x",
            ".a.com",
            "a.com.",
        ] {
            let text = format!("version = 99\n[categories.x]\ndomains = [{bad:?}]\n");
            assert!(Blocklist::parse(&text).is_err(), "{bad:?} must be refused");
        }
    }

    #[test]
    fn refuses_infrastructure_domains() {
        for bad in [
            "googleapis.com",
            "cursor.sh",
            "com",
            "github.com",
            "amazonaws.com",
        ] {
            let text = format!("version = 99\n[categories.x]\ndomains = [\"{bad}\"]\n");
            assert!(Blocklist::parse(&text).is_err(), "{bad} must be refused");
        }
        // Exact chatbot endpoints under those providers remain fine.
        let ok = "version = 99\n[categories.x]\ndomains = [\"generativelanguage.googleapis.com\", \"api2.cursor.sh\"]\n";
        assert!(Blocklist::parse(ok).is_ok());
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
