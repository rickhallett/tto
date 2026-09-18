//! /etc/hosts management. We own exactly one fenced section and touch
//! nothing outside it.

use std::path::Path;
use std::process::Command;

pub const PATH: &str = "/etc/hosts";
const BEGIN: &str =
    "# >>> tto: turn them off. Do not edit; rewritten every second while a block is active. >>>";
const END: &str = "# <<< tto <<<";

/// The hosts file with our section replaced by one for `domains`
/// (or removed, if `domains` is empty). Pure; easy to test.
pub fn render(current: &str, domains: &[String]) -> String {
    let mut out = String::with_capacity(current.len() + domains.len() * 48);
    let mut skipping = false;
    for line in current.lines() {
        if line == BEGIN {
            skipping = true;
            continue;
        }
        if line == END {
            skipping = false;
            continue;
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !domains.is_empty() {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(BEGIN);
        out.push('\n');
        for d in domains {
            out.push_str(&format!("0.0.0.0 {d}\n:: {d}\n"));
            if !d.starts_with("www.") && d.matches('.').count() == 1 {
                out.push_str(&format!("0.0.0.0 www.{d}\n:: www.{d}\n"));
            }
        }
        out.push_str(END);
        out.push('\n');
    }
    out
}

/// Ensure the file on disk matches `domains`. Returns true if it was rewritten.
pub fn apply(path: &Path, domains: &[String]) -> std::io::Result<bool> {
    let current = std::fs::read_to_string(path)?;
    let wanted = render(&current, domains);
    if wanted == current {
        return Ok(false);
    }
    let tmp = path.with_extension("tto-tmp");
    std::fs::write(&tmp, &wanted)?;
    std::fs::rename(&tmp, path)?;
    flush_dns();
    Ok(true)
}

pub fn flush_dns() {
    let _ = Command::new("/usr/bin/dscacheutil")
        .arg("-flushcache")
        .status();
    let _ = Command::new("/usr/bin/killall")
        .args(["-HUP", "mDNSResponder"])
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_leaves_rest_alone() {
        let base = "127.0.0.1 localhost\n255.255.255.255 broadcasthost\n::1 localhost\n";
        let blocked = render(base, &["claude.ai".into(), "api.openai.com".into()]);
        assert!(blocked.starts_with(base));
        assert!(blocked.contains("0.0.0.0 claude.ai\n"));
        assert!(blocked.contains(":: www.claude.ai\n"));
        assert!(!blocked.contains("www.api.openai.com"));
        // Re-rendering with the same list is a no-op.
        assert_eq!(
            render(&blocked, &["claude.ai".into(), "api.openai.com".into()]),
            blocked
        );
        // Removing the section restores the original exactly.
        assert_eq!(render(&blocked, &[]), base);
    }

    #[test]
    fn replaces_stale_section() {
        let base = "127.0.0.1 localhost\n";
        let old = render(base, &["chatgpt.com".into()]);
        let new = render(&old, &["claude.ai".into()]);
        assert!(!new.contains("chatgpt.com"));
        assert!(new.contains("claude.ai"));
        assert_eq!(new.matches(BEGIN).count(), 1);
    }
}
