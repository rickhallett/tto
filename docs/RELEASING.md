# Releasing

1. Bump `version` in `Cargo.toml`. Commit.
2. `git tag -a vX.Y.Z -m "tto X.Y.Z" && git push origin vX.Y.Z`.
3. `release.yml` builds a universal binary (arm64 + x86_64), packages
   `tto-X.Y.Z-macos-universal.tar.gz` (CLI) and
   `Turn-Them-Off-X.Y.Z-macos-universal.zip` (the app), writes `SHA256SUMS`,
   and publishes a GitHub release with generated notes.
4. Until Developer ID signing lands, builds are ad-hoc signed. Users
   right-click, Open once. This is a known gap; see PRICING.md.
5. Blocklist changes do not need a release: bump `version` in
   `assets/blocklist.toml`, merge to main, and every installed daemon picks
   it up within a day.
