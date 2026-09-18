//! The root daemon. Holds the timer, applies the block every second, and
//! answers the socket. It can make a block longer or wider. It cannot make
//! one shorter; that code does not exist.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::blocklist::{Blocklist, Category};
use crate::paths::Paths;
use crate::proto::{Request, Response};
use crate::state::State;
use crate::{hosts, procs};

const TICK: Duration = Duration::from_secs(1);
const BLOCKLIST_REFRESH: Duration = Duration::from_secs(24 * 3600);

struct Daemon {
    paths: Paths,
    state: Mutex<State>,
    blocklist: Mutex<Blocklist>,
}

pub fn run() -> Result<(), String> {
    let paths = Paths::from_env();
    // SAFETY: geteuid has no preconditions.
    if !paths.is_test() && unsafe { libc::geteuid() } != 0 {
        return Err(
            "the daemon must run as root (launchd does this; you don't run it by hand)".into(),
        );
    }
    std::fs::create_dir_all(paths.state_dir()).map_err(|e| e.to_string())?;
    if !paths.is_test() {
        let _ = std::fs::set_permissions(paths.state_dir(), std::fs::Permissions::from_mode(0o700));
    }

    let state = State::load(&paths.state());
    log(&format!(
        "starting; block {}",
        if state.active() {
            format!("active until {}", state.until)
        } else {
            "inactive".into()
        }
    ));
    let blocklist = load_blocklist(&paths);
    let d = Arc::new(Daemon {
        paths,
        state: Mutex::new(state),
        blocklist: Mutex::new(blocklist),
    });

    {
        let d = Arc::clone(&d);
        thread::spawn(move || enforce_forever(d));
    }
    serve(&d)
}

fn load_blocklist(paths: &Paths) -> Blocklist {
    if let Ok(text) = std::fs::read_to_string(paths.blocklist_override()) {
        match Blocklist::parse(&text) {
            Ok(list) => {
                log(&format!("using updated blocklist v{}", list.version));
                return list;
            }
            Err(e) => log(&format!("ignoring bad blocklist override: {e}")),
        }
    }
    Blocklist::embedded()
}

/// Once a day, try to fetch a newer list. Failure is silent; the embedded
/// list always works. Uses curl so we carry no TLS stack of our own.
fn refresh_blocklist(d: &Daemon) {
    let tmp = d.paths.state_dir().join("blocklist.toml.tmp");
    let ok = std::process::Command::new("/usr/bin/curl")
        .args(["-fsSL", "--max-time", "20", "-o"])
        .arg(&tmp)
        .arg(crate::blocklist::BLOCKLIST_URL)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        let _ = std::fs::remove_file(&tmp);
        return;
    }
    match std::fs::read_to_string(&tmp)
        .map_err(|e| e.to_string())
        .and_then(|t| Blocklist::parse(&t))
    {
        Ok(list) => {
            let current = d.blocklist.lock().unwrap().version;
            if list.version > current {
                let _ = std::fs::rename(&tmp, d.paths.blocklist_override());
                log(&format!(
                    "blocklist updated v{current} -> v{}",
                    list.version
                ));
                *d.blocklist.lock().unwrap() = list;
            } else {
                let _ = std::fs::remove_file(&tmp);
            }
        }
        Err(e) => {
            log(&format!("fetched blocklist is invalid, ignored: {e}"));
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

fn enforce_forever(d: Arc<Daemon>) {
    let mut last_refresh = Instant::now() - BLOCKLIST_REFRESH;
    // Start as if a block had just ended: the first inactive tick then
    // restores /etc/hosts, so a fence left by a crash or a lost state file
    // never outlives the daemon restart.
    let mut was_active = true;
    loop {
        let state = d.state.lock().unwrap().clone();
        if state.active() {
            let cat = d.blocklist.lock().unwrap().select(&state.categories);
            let _ = enforce(&d, &cat);
            heal_plist(&d);
            if !was_active {
                log(&format!(
                    "block on until {} ({})",
                    state.until,
                    state.categories.join(",")
                ));
            }
            was_active = true;
        } else if was_active || state != State::default() {
            // Stays true until the hosts file is actually clean, so a failed
            // restore is retried every tick rather than forgotten.
            was_active = !release(&d);
        }
        if last_refresh.elapsed() >= BLOCKLIST_REFRESH {
            last_refresh = Instant::now();
            // Off the enforcement thread: a slow fetch must never pause the block.
            let d = Arc::clone(&d);
            thread::spawn(move || refresh_blocklist(&d));
        }
        thread::sleep(TICK);
    }
}

/// Apply the block once. The hosts error is returned (and logged) so a
/// caller answering a request can say so; the tick loop just logs it.
fn enforce(d: &Daemon, cat: &Category) -> Result<(), String> {
    let hosts = hosts::apply(&d.paths.hosts(), &cat.domains)
        .map_err(|e| format!("could not write hosts file: {e}"));
    if let Err(e) = &hosts {
        log(e);
    }
    for p in procs::kill_matching(cat) {
        log(&format!("stopped {} (pid {})", p.path, p.pid));
    }
    hosts.map(drop)
}

/// Take the block down. Returns false if the hosts file could not be
/// restored, in which case the state is kept so the next tick tries again.
/// Holds the state lock throughout so an `off` that lands in the meantime
/// cannot be wiped by a decision made on a stale snapshot.
fn release(d: &Daemon) -> bool {
    let mut s = d.state.lock().unwrap();
    if s.active() {
        return true;
    }
    if let Err(e) = hosts::apply(&d.paths.hosts(), &[]) {
        log(&format!("hosts restore failed, will retry: {e}"));
        return false;
    }
    log("block over; hosts restored");
    *s = State::default();
    let _ = persist(d, &s);
    true
}

/// If someone deletes our launchd plist while a block is on, put it back.
/// (It only takes effect at next boot, but that is exactly when it matters.)
fn heal_plist(d: &Daemon) {
    if d.paths.is_test() || d.paths.plist().exists() {
        return;
    }
    if std::fs::copy(d.paths.plist_backup(), d.paths.plist()).is_ok() {
        log("launchd plist was missing; restored");
    }
}

fn persist(d: &Daemon, s: &State) -> Result<(), String> {
    let path = d.paths.state();
    if !d.paths.is_test() {
        set_immutable(&path, false);
    }
    let saved = s
        .save(&path)
        .map_err(|e| format!("could not save state: {e}"));
    if let Err(e) = &saved {
        log(e);
    }
    if !d.paths.is_test() {
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        set_immutable(&path, s.active());
    }
    saved
}

/// System-immutable flag. Root can clear it (securelevel 0), but only if
/// they know it exists. Friction, honestly labelled.
fn set_immutable(path: &std::path::Path, on: bool) {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
        return;
    };
    let flags: libc::c_uint = if on { libc::SF_IMMUTABLE } else { 0 };
    // SAFETY: valid NUL-terminated path.
    unsafe {
        libc::chflags(c.as_ptr(), flags);
    }
}

fn serve(d: &Arc<Daemon>) -> Result<(), String> {
    let sock = d.paths.socket();
    let _ = std::fs::remove_file(&sock);
    if let Some(dir) = sock.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let listener = UnixListener::bind(&sock).map_err(|e| format!("{}: {e}", sock.display()))?;
    // Anyone may ask; nobody can undo. World-writable socket is fine.
    let _ = std::fs::set_permissions(&sock, std::fs::Permissions::from_mode(0o666));
    log(&format!("listening on {}", sock.display()));
    // One request at a time, each bounded to MAX_REQUEST bytes and a short
    // timeout: a misbehaving client can only make the next caller wait a
    // second, never exhaust threads or memory.
    // A small bounded pool: each request is one short line with a 1 s
    // timeout, so MAX_WORKERS stalled clients are needed to delay anyone,
    // and even then only for a second. Beyond the cap, connections are
    // dropped rather than queued, so memory and threads stay flat.
    let busy = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    for conn in listener.incoming() {
        match conn {
            Ok(stream) => {
                use std::sync::atomic::Ordering;
                if busy.fetch_add(1, Ordering::SeqCst) >= MAX_WORKERS {
                    busy.fetch_sub(1, Ordering::SeqCst);
                    drop(stream);
                    continue;
                }
                let d = Arc::clone(d);
                let busy = Arc::clone(&busy);
                thread::spawn(move || {
                    handle(&d, stream);
                    busy.fetch_sub(1, Ordering::SeqCst);
                });
            }
            Err(e) => log(&format!("accept: {e}")),
        }
    }
    Ok(())
}

const MAX_WORKERS: usize = 16;

const MAX_REQUEST: u64 = 4096;

/// Who may start a block: root, or whoever is sitting at the machine (the
/// owner of /dev/console). Other local accounts may only ask for status.
fn peer_may_block(stream: &UnixStream, paths: &Paths) -> bool {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::MetadataExt;
    if paths.is_test() {
        return true;
    }
    let (mut uid, mut gid) = (u32::MAX, u32::MAX);
    // SAFETY: valid socket fd and out-pointers.
    if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0 {
        return false;
    }
    if uid == 0 {
        return true;
    }
    std::fs::metadata("/dev/console")
        .map(|m| m.uid() == uid)
        .unwrap_or(false)
}

fn handle(d: &Daemon, mut stream: UnixStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
    let mut line = String::new();
    {
        let mut reader = BufReader::new(&stream).take(MAX_REQUEST);
        if reader.read_line(&mut line).is_err() {
            return;
        }
    }
    let resp = if !line.ends_with('\n') {
        Response::err("request too long or unterminated")
    } else {
        match serde_json::from_str::<Request>(&line) {
            Ok(Request::Ping) | Ok(Request::Status) => {
                Response::ok(d.state.lock().unwrap().clone())
            }
            Ok(Request::Off { .. }) if !peer_may_block(&stream, &d.paths) => {
                Response::err("only the person at this Mac (or root) can start a block")
            }
            Ok(Request::Off { until, categories }) => off(d, until, categories),
            Err(e) => Response::err(format!("bad request: {e}")),
        }
    };
    let mut out = serde_json::to_string(&resp).unwrap_or_default();
    out.push('\n');
    let _ = stream.write_all(out.as_bytes());
}

fn off(d: &Daemon, until: u64, categories: Vec<String>) -> Response {
    let known = d.blocklist.lock().unwrap();
    let unknown: Vec<&String> = categories
        .iter()
        .filter(|c| *c != "everything" && !known.categories.contains_key(*c))
        .collect();
    if until <= crate::state::now() {
        return Response::err("that time is already past");
    }
    if until > crate::state::now() + 30 * 86400 {
        return Response::err("30 days is the longest block; you can add more later");
    }
    if !unknown.is_empty() {
        return Response::err(format!("unknown categories: {unknown:?}; try `tto list`"));
    }
    if categories.is_empty() {
        return Response::err("nothing selected");
    }
    drop(known);
    let mut s = d.state.lock().unwrap();
    s.extend(until, &categories);
    let persisted = persist(d, &s);
    // Apply immediately rather than waiting for the next tick.
    let cat = d.blocklist.lock().unwrap().select(&s.categories);
    let snapshot = s.clone();
    drop(s);
    let enforced = enforce(d, &cat);
    // The block is on in memory either way; but say plainly what did not
    // happen rather than print "Off." over a hosts file we could not write.
    match (persisted, enforced) {
        (Ok(()), Ok(())) => Response::ok(snapshot),
        (Err(e), _) => Response::err(format!(
            "{e}. The block is on, but will not survive a restart."
        )),
        (_, Err(e)) => Response::err(format!(
            "{e}. Processes are stopped, but the network is not blocked."
        )),
    }
}

fn log(msg: &str) {
    eprintln!("tto: {msg}");
}
