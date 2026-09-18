# Design

Turn Them Off (`tto`) puts a hard, timed block on every large language model
a person can reach from their Mac: chat apps and websites, the AI inside
code editors and terminals, and models running locally. It exists for people
who have found themselves in a loop with these tools past the point of use,
and who want a way out that does not depend on willpower at 2am.

The product is a small, glossy menu bar app for people who have never opened
Terminal. Underneath it is a root daemon and a CLI, both in Rust, both open
source. This document is the reasoning; `README.md` is the manual.

## Principles

1. **Hard commitment.** Once a block starts, nothing in the app, the CLI, or
   the daemon can end it early. Not a hidden flag, not a debug build. The
   code to shorten a block does not exist. A block can only be made longer
   or wider.
2. **Do no collateral damage.** Block the exact LLM endpoint, never the
   provider. Kill only processes whose sole purpose is talking to a model;
   editors stay alive with their AI dead. A person going to bed must not
   lose Google Drive or an unsaved file.
3. **Nothing phones home.** No accounts, no telemetry, no activation server.
   The daemon makes exactly one outbound request: a daily fetch of the
   blocklist, which fails silently and harmlessly.
4. **Everything handled for them.** One password prompt, once. Then three
   choices in plain words: what, until when, go.
5. **Honest about limits.** This is friction, not cryptography. See
   [THREAT-MODEL.md](THREAT-MODEL.md).

## Architecture

```
 menu bar app (user session)     tto CLI (user session)
        \                              /
         \      JSON over unix socket /
          \    /var/run/tto.sock      /
           v                         v
        +-------------------------------+
        | tto daemon (root, launchd)    |   /var/db/tto/state.json   (0600, schg while active)
        |  every second while active:   |   /var/db/tto/blocklist.toml (optional update)
        |   - rewrite /etc/hosts fence  |   /Library/LaunchDaemons/dev.oceanheart.tto.plist
        |   - kill listed processes     |   /Library/PrivilegedHelperTools/dev.oceanheart.tto
        |   - restore plist if deleted  |
        |  at expiry: restore hosts     |
        +-------------------------------+
```

One binary. `tto daemon` is what launchd runs as root; `tto menu` is the
menu bar app; `Turn Them Off.app` is the same binary in a bundle that starts
in menu mode when double-clicked. The helper copy under
`/Library/PrivilegedHelperTools` is what the daemon runs from, so updating
the app in `/Applications` never breaks an active block.

### Socket protocol

One JSON line in, one out. Three requests: `ping`, `status`, and
`off {until, categories}`. `off` merges into the current block: later of the
two expiries, union of the categories. The socket is world-writable because
anyone may ask and nobody can undo.

### Enforcement layers (Tier 1, shipped)

| layer | covers | bypassed by |
|---|---|---|
| `/etc/hosts` fence, rewritten every second | every app and CLI that resolves a hostname | DNS-over-HTTPS in the browser; a VPN that pushes its own resolver; hardcoded IPs |
| process kill, every second, exact names and bundle paths | local models, standalone agents, chat apps | renaming the binary (you would have to want to) |
| launchd `KeepAlive` + plist self-healing | `kill`, deleting the plist | `launchctl bootout` as root |
| root-owned state, `schg` flag while active | editing the expiry | `chflags noschg` as root |
| survives reboot and sleep | restarting | Recovery mode (see [RECOVERY.md](RECOVERY.md)) |

### Planned layers

- **Tier 2: pf firewall anchor.** Resolve the blocklist to IPs at block start,
  refresh every few minutes, drop the packets. Closes the DoH and VPN holes
  without needing Apple's permission.
- **Tier 3: NetworkExtension content filter.** Hostname-based, per-process,
  DoH-proof. Needs a system extension, Developer ID, and user approval.
  Swift, not Rust. This is what a shippable v2 looks like.
- **Schedules.** "Every night 22:00 to 07:00" as a first-class thing. The
  biological case is sleep; a one-off timer is the demo.

## The blocklist

Lives in `assets/blocklist.toml`, compiled into the binary, and fetched
daily from `BLOCKLIST_URL` so an installed copy keeps working the week a new
chatbot launches. Rules in [BLOCKLIST-POLICY.md](BLOCKLIST-POLICY.md). The
list is the definitions file; the app is the antivirus.
