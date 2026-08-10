# Test Matrix & Results — nog v1.2.0 (C2: Snap backend)

**Date:** 2026-08-10 · **Machine:** Javier's desktop
**Subject staged for the dogfood:** snapd 2.76 installed **through nog**
(`nog install snapd` → AUR via yay) + `systemctl enable --now snapd.socket`;
then `hello` (106 kB, Canonical) installed at **revision 29** while tracking
`latest/stable` (revision 42) — a genuine pending refresh.

## Unit tests — 69 passed / 0 failed (61 → 69)

| # | Test | Proves |
|---|------|--------|
| 1 | `parses_the_refresh_list_table` | `snap refresh --list` columns; header skipped; channel resolved per snap with a sane default |
| 2 | `all_up_to_date_means_no_updates` | "All snaps up to date." and empty output both mean nothing pending |
| 3 | `parses_snap_list_tracking_column` | tracked channel extracted from `snap list` — the key to the right date |
| 4 | `finds_the_tracked_channels_date` | per-channel publish date from `snap info` |
| 5 | `channel_without_a_date_is_unresolved_not_guessed` | a `↑` ("same as above") channel yields no date → Unknown, never a fabricated one |
| 6 | `apply_list_names_only_cleared_snaps` | only Ready + approved-Unknown snaps are named; pacman packages never leak in |
| 7 | `held_snaps_can_never_be_refreshed` | the hold rule is structural |
| 8 | `to_pending_marks_missing_versions_honestly` | missing versions render `?` |
| +2 | sources.toml round-trips | `snap` key defaults active, round-trips, fails closed with the rest |

## Live dogfood — all passed

| # | Check | Result |
|---|-------|--------|
| L1 | snapd **absent** (before install) | no snap line, no warning, no error — dormant as ruled ✔ |
| L2 | `nog deactivate snap` / `activate snap` while snapd absent | state persists; activate helpfully notes snapd isn't installed and how to get it |
| L3 | Detection after install | `1 snap update(s) reported by snapd` |
| L4 | Table row | `hello 2.10.1 → 2.10 · Tier 3 · 1663 days past window · snap` — channel date (2022-01-14) resolved, tier applied, **source tag present** |
| L5 | Real apply | `Handing off 1 snap(s) to snapd ...` → sudo escalation → `hello 2.10 from Canonical✓ refreshed`, snap's own output shown (issue #8 rule honored for the new backend) |
| L6 | Coexistence | same run also applied 3 pacman upgrades; flatpak reported 0; every source independent |
| L7 | Run logging | written to `~/.local/share/nog/logs/20260810 nog-update.csv` |

## Notes

- **Distro identity:** `snap version` reports `kognogos` — snapd reads the
  branded `/etc/os-release`. Harmless and rather satisfying.
- **snapd's install nagged about `$PATH`** (`/var/lib/snapd/snap/bin` needs a
  fresh session). Doesn't affect nog, which calls `snap` by name from the
  normal PATH.
- **Test-setup note:** `snap install --revision=N` reported *"Channel
  latest/stable for hello is closed; temporarily forwarding to beta"* — the
  snap still tracks `latest/stable`, so the pending update appeared as
  intended.
- **Left installed after testing:** snapd + `hello` + `core` (the C3 install
  chain will want a working snapd anyway). `snap remove hello` and disabling
  `snapd.socket` reverse it whenever wanted.
- **Not covered live:** a *held* snap (the only available update was 1663 days
  old). Same reasoning as C1 — the hold decision is shared, tested code, and
  the snap-specific half is pinned by tests 6 and 7.
