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
    // Lines inside a BEGIN..END fence are dropped, but only once END is
    // seen: an unmatched BEGIN (a truncated file, a stray paste) must not
    // eat the rest of the hosts file.
    let mut held: Option<Vec<&str>> = None;
    for line in current.lines() {
        match (&mut held, line) {
            (None, l) if l == BEGIN => held = Some(vec![l]),
            (None, l) => {
                out.push_str(l);
                out.push('\n');
            }
            (Some(_), l) if l == END => held = None,
            (Some(buf), l) => buf.push(l),
        }
    }
    if let Some(buf) = held {
        // Write the held lines back, but defuse the stray marker itself:
        // if it stayed verbatim, the END we append below would pair with
        // it and the next rewrite would swallow everything in between.
        for l in buf {
            if l == BEGIN {
                out.push_str("# (tto: stray begin marker, kept for reference) ");
            }
            out.push_str(l);
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
    fn unmatched_begin_loses_nothing() {
        let base = format!("127.0.0.1 localhost\n{BEGIN}\n0.0.0.0 x.com\n10.0.0.1 nas.local\n");
        let out = render(&base, &[]);
        assert!(out.contains("10.0.0.1 nas.local"), "{out}");
        assert!(
            out.contains("0.0.0.0 x.com"),
            "we cannot tell what was ours; keep it"
        );
        assert!(
            !out.lines().any(|l| l == BEGIN),
            "stray marker must be defused"
        );
        // And blocking on top of it, then releasing, still loses nothing.
        let blocked = render(&base, &["claude.ai".into()]);
        assert_eq!(
            blocked.lines().filter(|l| *l == BEGIN).count(),
            1,
            "{blocked}"
        );
        let released = render(&blocked, &[]);
        assert!(released.contains("10.0.0.1 nas.local"), "{released}");
        assert!(!released.contains("claude.ai"));
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
