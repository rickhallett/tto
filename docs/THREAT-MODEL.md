# Threat model

## Who we are defending against

A person, at night, who chose this a few hours ago and now wants out. They
are an admin on their own Mac. They may be a software engineer. They are not
an attacker; they are the same person, later.

That is the whole model. There is no adversary who wants to bypass this
without also being the person who installed it.

## What "hard" means on macOS

A user with admin rights can, with a terminal and `sudo`, undo anything a
third-party app installs. macOS runs at `kern.securelevel 0`, so even the
BSD system-immutable flag can be cleared by root. Only paths under System
Integrity Protection are off limits, and no third-party app can write there.

So a hard commitment on a Mac is not "impossible to remove". It is:

> Removal requires a sequence of deliberate, technical, root-level steps
> that the person has to look up and type, while in the state the tool
> exists to protect them from.

Every layer buys friction, and we label each one honestly.

## Escape routes, ranked by effort

| route | effort | who |
|---|---|---|
| Menu bar app, CLI | none exists | nobody |
| Restart, sleep, log out | nothing; the daemon comes back | nobody |
| `kill` the daemon | launchd restarts it in seconds | nobody |
| Delete the plist | the daemon restores it | nobody |
| Edit `/etc/hosts` by hand | rewritten within a second | nobody |
| Edit the expiry in state.json | root-owned, `schg` | needs sudo and the flag |
| `sudo launchctl bootout` + clear flag + fix hosts | five commands, in the right order, looked up at 2am | a determined engineer |
| Boot to Recovery, Terminal, remove files | about ten minutes | anyone who reads RECOVERY.md |

The last row is the documented exit. It is slow enough to work as a
cooling-off period, and it is documented rather than secret because the
point is friction, not deceit.

## What we do not defend against

- A second admin account without the block. (Blocks are machine-wide, so
  this actually works.)
- Another device. Your phone is your phone.
- DNS-over-HTTPS and VPN resolvers. Tier 2 (pf) closes this.
- A brand-new chatbot not yet on the list. The daily blocklist fetch closes
  this within a day; `tto list --full` shows what is covered.

## What we promise instead

- Nothing phones home. One outbound request, the blocklist, daily.
- Nothing outside the fenced section of `/etc/hosts` is touched.
- No process outside the exact names in the blocklist is ever signalled.
- The full source is public, and the release binaries are built by CI from
  a tag you can check out.
