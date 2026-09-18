//! Installing the root helper: one password prompt, once.

use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use crate::paths::Paths;
use crate::proto::{self, Request};
use crate::state::LABEL;

fn require_root() -> Result<(), String> {
    // SAFETY: geteuid has no preconditions.
    if unsafe { libc::geteuid() } != 0 {
        return Err("needs root: run `sudo tto install`, or use the menu bar app which asks for your password".into());
    }
    Ok(())
}

fn plist_xml(helper: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LABEL}</string>
  <key>ProgramArguments</key><array>
    <string>{helper}</string>
    <string>daemon</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>ProcessType</key><string>Background</string>
  <key>StandardErrorPath</key><string>/var/log/tto.log</string>
  <key>StandardOutPath</key><string>/var/log/tto.log</string>
</dict>
</plist>
"#
    )
}

pub fn install() -> Result<(), String> {
    require_root()?;
    let paths = Paths::from_env();
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let helper = paths.helper();

    std::fs::create_dir_all(helper.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(paths.state_dir()).map_err(|e| e.to_string())?;
    std::fs::set_permissions(paths.state_dir(), std::fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;

    // A running daemon holds the old binary open; bootout first.
    let _ = Command::new("/bin/launchctl")
        .args(["bootout", &format!("system/{LABEL}")])
        .output();
    std::fs::copy(&me, &helper).map_err(|e| format!("copy helper: {e}"))?;
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;
    chown_root(&helper)?;

    let xml = plist_xml(&helper.display().to_string());
    for p in [paths.plist(), paths.plist_backup()] {
        std::fs::write(&p, &xml).map_err(|e| format!("{}: {e}", p.display()))?;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644))
            .map_err(|e| e.to_string())?;
        chown_root(&p)?;
    }

    let out = Command::new("/bin/launchctl")
        .args(["bootstrap", "system"])
        .arg(paths.plist())
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "launchctl bootstrap: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }

    for _ in 0..50 {
        if proto::call(&paths.socket(), &Request::Ping).is_ok() {
            println!("Installed. The helper is running and will start at every boot.");
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err("helper installed but not answering; see /var/log/tto.log".into())
}

/// Refuses while a block is active. That is not a bug.
pub fn uninstall() -> Result<(), String> {
    require_root()?;
    let paths = Paths::from_env();
    if let Ok(resp) = proto::call(&paths.socket(), &Request::Status)
        && resp.state.active()
    {
        {
            return Err(format!(
                "a block is active until {}. Uninstall is not a back door; try again then. (`tto recovery` if you truly must.)",
                crate::when::describe(resp.state.until)
            ));
        }
    }
    let _ = Command::new("/bin/launchctl")
        .args(["bootout", &format!("system/{LABEL}")])
        .output();
    let _ = crate::hosts::apply(&paths.hosts(), &[]);
    for p in [paths.plist(), paths.helper(), paths.socket()] {
        let _ = std::fs::remove_file(p);
    }
    let _ = std::fs::remove_dir_all(paths.state_dir());
    println!("Uninstalled. Sleep well anyway.");
    Ok(())
}

fn chown_root(p: &std::path::Path) -> Result<(), String> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(p.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
    // SAFETY: valid NUL-terminated path; root:wheel.
    if unsafe { libc::chown(c.as_ptr(), 0, 0) } != 0 {
        return Err(format!(
            "chown {}: {}",
            p.display(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}
