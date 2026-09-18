//! Finding and stopping the processes on the list. Root only sees all of
//! them; as a user this still works for your own processes (handy for
//! `tto doctor`).

use std::ffi::CStr;
use std::path::Path;

use crate::blocklist::Category;

#[derive(Debug, Clone)]
pub struct Proc {
    pub pid: i32,
    /// Resolved executable path, e.g. /Applications/ChatGPT.app/Contents/MacOS/ChatGPT
    pub path: String,
    /// argv[0..2], for CLIs that run under an interpreter (`node …/claude`).
    pub argv: Vec<String>,
    /// Start time at enumeration; re-checked before a kill so a reused pid
    /// is never signalled.
    pub started: Option<(u64, u64)>,
}

pub fn list() -> Vec<Proc> {
    let mut pids = vec![0i32; 4096];
    loop {
        // SAFETY: buffer is sized in bytes; the call writes at most that many
        // and returns how many bytes it would need in total.
        let n =
            unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), (pids.len() * 4) as i32) };
        if n <= 0 {
            return Vec::new();
        }
        // The kernel reports the count it filled; if the buffer was full,
        // there may be more. Grow and ask again until it is not full.
        if (n as usize) < pids.len() {
            pids.truncate(n as usize);
            break;
        }
        pids.resize(pids.len() * 2, 0);
    }
    pids.into_iter()
        .filter(|&pid| pid > 1)
        .filter_map(|pid| {
            let path = exe_path(pid)?;
            Some(Proc {
                pid,
                path,
                argv: argv(pid).unwrap_or_default(),
                started: start_time(pid),
            })
        })
        .collect()
}

/// Process start time (seconds, microseconds), used to make sure a pid still
/// belongs to the process we enumerated before signalling it.
fn start_time(pid: i32) -> Option<(u64, u64)> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::uninit();
    let size = size_of::<libc::proc_bsdinfo>() as i32;
    // SAFETY: the buffer is exactly `size` bytes of proc_bsdinfo.
    let n = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if n != size {
        return None;
    }
    let info = unsafe { info.assume_init() };
    Some((info.pbi_start_tvsec, info.pbi_start_tvusec))
}

fn exe_path(pid: i32) -> Option<String> {
    let mut buf = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    // SAFETY: buffer length is passed alongside; proc_pidpath NUL-terminates.
    let n = unsafe { libc::proc_pidpath(pid, buf.as_mut_ptr().cast(), buf.len() as u32) };
    if n <= 0 {
        return None;
    }
    Some(String::from_utf8_lossy(&buf[..n as usize]).into_owned())
}

/// First two argv entries via KERN_PROCARGS2. Fails (None) for processes we
/// may not inspect, which is fine: the exe path check still applies.
fn argv(pid: i32) -> Option<Vec<String>> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut size: usize = 0;
    // SAFETY: standard two-call sysctl pattern; buffer sized by the first call.
    unsafe {
        if libc::sysctl(
            mib.as_mut_ptr(),
            3,
            std::ptr::null_mut(),
            &mut size,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return None;
        }
        let mut buf = vec![0u8; size];
        if libc::sysctl(
            mib.as_mut_ptr(),
            3,
            buf.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return None;
        }
        buf.truncate(size);
        parse_procargs2(&buf)
    }
}

/// Layout: i32 argc, NUL-terminated exec path, NUL padding, then argv strings.
fn parse_procargs2(buf: &[u8]) -> Option<Vec<String>> {
    if buf.len() < 4 {
        return None;
    }
    let argc = i32::from_ne_bytes(buf[..4].try_into().ok()?).max(0) as usize;
    let mut rest = &buf[4..];
    let exec = CStr::from_bytes_until_nul(rest).ok()?;
    rest = &rest[exec.to_bytes().len() + 1..];
    while rest.first() == Some(&0) {
        rest = &rest[1..];
    }
    let mut out = Vec::new();
    for _ in 0..argc.min(2) {
        let s = CStr::from_bytes_until_nul(rest).ok()?;
        out.push(s.to_string_lossy().into_owned());
        rest = &rest[s.to_bytes().len() + 1..];
    }
    Some(out)
}

fn basename(p: &str) -> &str {
    Path::new(p)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(p)
}

/// Does this process belong to the selected categories?
/// Runtimes that execute a script named in argv[1]. Only for these do we
/// look past the executable: `node .../claude` is Claude Code, but
/// `vim claude` is someone editing a file called claude, and must live.
const INTERPRETERS: &[&str] = &[
    "node", "bun", "deno", "python", "python3", "ruby", "perl", "sh", "bash", "zsh",
];

/// The name this process answers to: its executable's basename, or, for an
/// interpreter, the basename of the script it is running.
pub fn effective_name(p: &Proc) -> &str {
    let exe = basename(&p.path);
    if INTERPRETERS.contains(&exe)
        && let Some(script) = p.argv.get(1)
    {
        return basename(script);
    }
    exe
}

/// Does this process belong to the selected categories?
pub fn matches(p: &Proc, cat: &Category) -> bool {
    if cat
        .apps
        .iter()
        .any(|app| p.path.split('/').any(|c| c == app))
    {
        return true;
    }
    let name = effective_name(p);
    cat.processes.iter().any(|want| want == name)
}

/// SIGKILL everything that matches. Returns what was killed.
pub fn kill_matching(cat: &Category) -> Vec<Proc> {
    let me = std::process::id() as i32;
    let mut killed = Vec::new();
    for p in list() {
        if p.pid == me || !matches(&p, cat) {
            continue;
        }
        let Some(started) = p.started else { continue };
        // A pid can be recycled between any two steps here, and macOS has
        // no pid-bound handle. So: freeze whatever holds the pid, verify it
        // is still our process (a stopped process cannot exit, so the pid
        // cannot change under us), then kill; otherwise thaw and move on.
        // SAFETY: plain kill(2) with signals that cannot corrupt state.
        unsafe {
            if libc::kill(p.pid, libc::SIGSTOP) != 0 {
                continue;
            }
            if start_time(p.pid) == Some(started) {
                if libc::kill(p.pid, libc::SIGKILL) == 0 {
                    killed.push(p);
                }
            } else {
                libc::kill(p.pid, libc::SIGCONT);
            }
        }
    }
    killed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat() -> Category {
        Category {
            processes: vec!["claude".into(), "ollama".into()],
            apps: vec!["ChatGPT.app".into()],
            ..Default::default()
        }
    }

    #[test]
    fn matches_app_bundle_by_path_component() {
        let p = Proc {
            pid: 1,
            path: "/Applications/ChatGPT.app/Contents/MacOS/ChatGPT".into(),
            argv: vec![],
            started: None,
        };
        assert!(matches(&p, &cat()));
        let helper = Proc {
            pid: 2,
            path: "/Applications/ChatGPT.app/Contents/Frameworks/x.app/Contents/MacOS/x".into(),
            argv: vec![],
            started: None,
        };
        assert!(
            matches(&helper, &cat()),
            "helpers inside the bundle count too"
        );
    }

    #[test]
    fn matches_cli_by_exe_or_argv_basename() {
        let native = Proc {
            pid: 3,
            path: "/opt/homebrew/bin/ollama".into(),
            argv: vec!["ollama".into(), "serve".into()],
            started: None,
        };
        assert!(matches(&native, &cat()));
        let node = Proc {
            pid: 4,
            path: "/usr/local/bin/node".into(),
            argv: vec!["node".into(), "/Users/x/.npm/bin/claude".into()],
            started: None,
        };
        assert!(matches(&node, &cat()));
        let innocent = Proc {
            pid: 5,
            path: "/usr/local/bin/node".into(),
            argv: vec!["node".into(), "server.js".into()],
            started: None,
        };
        assert!(!matches(&innocent, &cat()));
        // The promise: nothing outside the list is ever signalled.
        let editing = Proc {
            pid: 7,
            path: "/usr/bin/vim".into(),
            argv: vec!["vim".into(), "claude".into()],
            started: None,
        };
        assert!(
            !matches(&editing, &cat()),
            "an editor opened on a file called claude must live"
        );
        let arg = Proc {
            pid: 8,
            path: "/usr/bin/grep".into(),
            argv: vec!["grep".into(), "ollama".into()],
            started: None,
        };
        assert!(!matches(&arg, &cat()));
        let lookalike = Proc {
            pid: 6,
            path: "/usr/bin/claudette".into(),
            argv: vec![],
            started: None,
        };
        assert!(!matches(&lookalike, &cat()), "exact basenames only");
    }

    #[test]
    fn procargs2_parses_argv() {
        let mut buf = 2i32.to_ne_bytes().to_vec();
        buf.extend(b"/usr/local/bin/node\0\0\0node\0/x/claude\0");
        assert_eq!(parse_procargs2(&buf).unwrap(), vec!["node", "/x/claude"]);
    }

    #[test]
    fn list_includes_self() {
        let me = std::process::id() as i32;
        assert!(list().iter().any(|p| p.pid == me));
    }
}
