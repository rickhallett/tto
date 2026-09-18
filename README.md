<p align="center"><img src="assets/logo.png" width="240" alt="A stick figure, eyes closed, holding an unplugged cable that hangs to the floor."></p>
<h1 align="center">Turn Them Off</h1>
<p align="center"><em>A hard, timed block on every LLM you can reach from your Mac. Get some rest.</em></p>
<p align="center">
  <a href="https://github.com/rickhallett/tto/actions/workflows/ci.yml"><img src="https://github.com/rickhallett/tto/actions/workflows/ci.yml/badge.svg" alt="ci"></a>
  <a href="https://github.com/rickhallett/tto/releases/latest"><img src="https://img.shields.io/github/v/release/rickhallett/tto?color=1b1f27" alt="release"></a>
  <img src="https://img.shields.io/badge/platform-macOS%2012%2B-000000?logo=apple&logoColor=white" alt="macOS">
  <img src="https://img.shields.io/badge/rust-stable-DEA584?logo=rust&logoColor=white" alt="rust">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT"></a>
  <img src="https://img.shields.io/badge/cancel%20button-none-critical" alt="no cancel button">
  <img src="https://img.shields.io/badge/phones%20home-never-success" alt="never phones home">
</p>

---

You pick what to turn off and until when. Then it is off: the chat apps and
websites, the AI in your editor and terminal, the models running on your
Mac. At the time you chose it all comes back on its own. In between, there
is no button. Not by you, not by restarting.

This is for people who have found themselves in a loop with these tools
past the point of use, and who would like the decision made once, earlier,
by a calmer version of themselves.

## For normal people

Download **Turn Them Off.app** from the [latest release](https://github.com/rickhallett/tto/releases/latest),
drag it to Applications, open it. A small stick figure appears in your menu
bar. Click it, choose **Install helper**, type your password once. Then:

- **Turn them off** for an hour, for the evening, until 7am, for the weekend.
- **What to turn off**: chat, code editor AI, local models. All three by default.
- Read the line about it not being cancellable. Mean it. Click.

That is the whole app. Everything technical below is handled for you.

## For terminal people

```
brew install rickhallett/tap/tto      # soon; or grab the tarball from releases
sudo tto install                      # once
tto off tonight                       # until 07:00
tto off 2h --only coding              # just the editor and CLI agents
tto off weekend                       # until Monday 07:00
tto status
tto list --full                       # exactly what is blocked
tto doctor
tto menu                              # the menu bar app, from a shell
```

There is no `tto stop`. Try it if you like; it will tell you so.

## What it does, technically

A root daemon (launchd, `KeepAlive`) holds the timer. Every second while a
block is active it rewrites a fenced section of `/etc/hosts` for the listed
domains, and stops the listed processes by exact executable name or app
bundle. At expiry it restores `/etc/hosts` exactly and forgets the block.
The list of domains and processes ships in the binary and is refreshed daily
from this repo, free, forever.

What it does not do: touch anything outside its hosts fence, signal any
process not on the list, phone home, or offer any way to end early. The full
design, the honest limits, and the recovery procedure:

- [docs/DESIGN.md](docs/DESIGN.md): architecture and principles
- [docs/THREAT-MODEL.md](docs/THREAT-MODEL.md): what "hard" means on a Mac, and what it does not
- [docs/BLOCKLIST-POLICY.md](docs/BLOCKLIST-POLICY.md): why we block `api2.cursor.sh` and never `cursor.sh`
- [docs/RECOVERY.md](docs/RECOVERY.md): the ten-minute way out
- [docs/PRICING.md](docs/PRICING.md): 99p once, and why not the App Store
- [docs/RELEASING.md](docs/RELEASING.md)

## Building

```
cargo build --release
sh tests/e2e.sh                       # daemon, unprivileged, against a scratch hosts file
scripts/bundle-app.sh target/release/tto 0.1.0 dist
```

Stable Rust, macOS 12 or later. The daemon can be run unprivileged for
development by setting `TTO_PREFIX=/some/dir`, which relocates every path it
touches.

## Status

0.1: hosts fence, process stop, launchd daemon, menu bar app, CLI. Ad-hoc
signed. See [DESIGN.md](docs/DESIGN.md) for Tier 2 (pf firewall) and Tier 3
(NetworkExtension), and for schedules.

## License

MIT. The website is at [site/](site/).
