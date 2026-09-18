mod blocklist;
mod daemon;
mod hosts;
mod install;
mod menu;
mod paths;
mod procs;
mod proto;
mod state;
mod when;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

use paths::Paths;
use proto::Request;

/// Turn Them Off: a hard, timed block on every LLM you can reach from this Mac.
///
/// Pick what to turn off, pick until when, and that's that. There is no
/// cancel. Not by you, not by restarting. Get some rest.
#[derive(Parser, Debug)]
#[command(version, about, verbatim_doc_comment)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Turn them off until WHEN: "2h", "90m", "until 07:00", "tonight", "today", "weekend".
    Off {
        when: String,
        /// Categories to turn off (default: everything). See `tto list`.
        #[arg(short, long, value_delimiter = ',', default_value = "everything")]
        only: Vec<String>,
        /// Skip the "are you sure" line. It's still not cancellable.
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Show whether a block is on and until when.
    Status,
    /// Show the blocklist categories and what's in them.
    List {
        /// Print every domain and process, not just the summary.
        #[arg(long)]
        full: bool,
    },
    /// Install the root helper (needs sudo; once).
    Install,
    /// Remove the helper. Refuses while a block is active.
    Uninstall,
    /// Check that everything is wired up.
    Doctor,
    /// Print the recovery procedure. It is slow on purpose.
    Recovery,
    /// Run as the menu bar app.
    Menu,
    /// (internal) The root daemon; launchd runs this.
    #[command(hide = true)]
    Daemon,
    /// There is no stop.
    #[command(hide = true, aliases = ["cancel", "on", "unblock", "resume"])]
    Stop,
}

fn main() -> ExitCode {
    let cli = if launched_from_bundle() {
        Cli::parse_from(["tto", "menu"])
    } else {
        Cli::parse()
    };
    let paths = Paths::from_env();
    let result = match cli.command {
        Command::Off { when, only, yes } => off(&paths, &when, only, yes),
        Command::Status => status(&paths),
        Command::List { full } => {
            list(full);
            Ok(())
        }
        Command::Install => install::install(),
        Command::Uninstall => install::uninstall(),
        Command::Doctor => doctor(&paths),
        Command::Recovery => {
            print!("{}", include_str!("../docs/RECOVERY.md"));
            Ok(())
        }
        Command::Menu => {
            menu::run(paths);
            Ok(())
        }
        Command::Daemon => daemon::run(),
        Command::Stop => Err("There is no stop. That's the point. `tto status` tells you when; `tto recovery` if you truly must.".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tto: {e}");
            ExitCode::FAILURE
        }
    }
}

fn off(paths: &Paths, when: &str, only: Vec<String>, yes: bool) -> Result<(), String> {
    let until = when::parse(when)?;
    // Category names are checked by the daemon against its live list, which
    // may be newer than the one compiled in here.
    let what = if only.iter().any(|o| o == "everything") {
        "everything".to_string()
    } else {
        only.join(", ")
    };
    println!("Turning off {what} until {}.", when::describe(until));
    if !yes {
        println!(
            "This cannot be undone early. Not by you, not by restarting. Press Enter to continue, Ctrl-C to think about it."
        );
        let mut line = String::new();
        let bytes = std::io::stdin()
            .read_line(&mut line)
            .map_err(|e| e.to_string())?;
        if bytes == 0 {
            return Err("confirmation required; pass --yes to skip it".into());
        }
    }
    let resp = proto::call(
        &paths.socket(),
        &Request::Off {
            until,
            categories: only,
        },
    )
    .map_err(|e| e.to_string())?;
    println!(
        "Off. Back at {}. Get some rest.",
        when::describe(resp.state.until)
    );
    Ok(())
}

fn status(paths: &Paths) -> Result<(), String> {
    let resp = proto::call(&paths.socket(), &Request::Status).map_err(|e| e.to_string())?;
    let s = resp.state;
    if s.active() {
        println!(
            "Off ({}) until {} — {} to go.",
            s.categories.join(", "),
            when::describe(s.until),
            when::remaining(s.remaining())
        );
    } else {
        println!("Everything is on. Helper v{} is ready.", resp.version);
    }
    Ok(())
}

fn list(full: bool) {
    let list = blocklist::Blocklist::embedded();
    println!(
        "blocklist v{} (compiled in; the helper may hold a newer one)",
        list.version
    );
    println!("categories: {}\n", list.names().join(", "));
    for (name, cat) in &list.categories {
        println!("{name:<8} {}", cat.title);
        println!("         {}", cat.blurb);
        println!(
            "         {} domains, {} processes, {} apps",
            cat.domains.len(),
            cat.processes.len(),
            cat.apps.len()
        );
        if full {
            for d in &cat.domains {
                println!("           {d}");
            }
            for p in cat.processes.iter().chain(&cat.apps) {
                println!("           {p}");
            }
        }
        println!();
    }
    println!("everything  All of the above (the default).");
}

fn doctor(paths: &Paths) -> Result<(), String> {
    let mut bad = 0;
    let mut check = |name: &str, ok: bool, hint: &str| {
        println!(
            "{} {name}{}",
            if ok { "ok " } else { "!! " },
            if ok {
                String::new()
            } else {
                format!("  ({hint})")
            }
        );
        if !ok {
            bad += 1;
        }
    };
    check("helper binary", paths.helper().exists(), "sudo tto install");
    check("launchd plist", paths.plist().exists(), "sudo tto install");
    let ping = proto::call(&paths.socket(), &Request::Ping);
    check(
        "daemon answering",
        ping.is_ok(),
        "sudo tto install; then see /var/log/tto.log",
    );
    if let Ok(resp) = &ping {
        check(
            "helper version matches",
            resp.version == env!("CARGO_PKG_VERSION"),
            "sudo tto install to update the helper",
        );
    }
    check(
        "hosts file readable",
        std::fs::read_to_string(paths.hosts()).is_ok(),
        "unexpected; is /etc/hosts a file?",
    );
    check(
        "blocklist valid",
        blocklist::Blocklist::parse(blocklist::EMBEDDED).is_ok(),
        "this is a bug",
    );
    let cat = blocklist::Blocklist::embedded().select(&["everything".into()]);
    let running: Vec<_> = procs::list()
        .into_iter()
        .filter(|p| procs::matches(p, &cat))
        .collect();
    println!(
        "   {} listed process(es) currently running{}",
        running.len(),
        if running.is_empty() { "" } else { ":" }
    );
    for p in running {
        println!("     {} (pid {})", p.path, p.pid);
    }
    if bad == 0 {
        Ok(())
    } else {
        Err(format!("{bad} problem(s)"))
    }
}

fn launched_from_bundle() -> bool {
    std::env::args_os().len() == 1
        && std::env::current_exe()
            .map(|p| p.components().any(|c| c.as_os_str() == "MacOS"))
            .unwrap_or(false)
}
