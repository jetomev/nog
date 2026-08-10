# Test Matrix & Results — nog v1.1.0 (C1: Flatpak backend)

**Date:** 2026-08-10 · **Machine:** Javier's desktop (Arch, flatpak 1.18.0)
**Subject staged for the dogfood:** `com.github.tchx84.Flatseal` installed from
flathub, then rolled back one commit (`sudo flatpak update --commit=<older>`) so
a real pending update existed. Kept installed afterwards (useful app).

## Unit tests — 61 passed / 0 failed

New in this cycle (`src/flatpak.rs`, `src/sources.rs`):

| # | Test | Proves |
|---|------|--------|
| 1 | `parses_tab_separated_updates` | app-id / version / origin parsing, versionless flatpaks tolerated |
| 2 | `empty_output_means_no_updates` | empty + blank-line output = no pending |
| 3 | `remote_info_date_line_is_found` | commit-date extraction feeding the hold windows |
| 4 | `remote_info_without_date_is_none` | missing date → Unknown bucket, not a crash |
| 5 | `apply_list_names_only_cleared_flatpaks` | only Ready + approved-Unknown flatpak refs are named; pacman packages never leak into a flatpak transaction |
| 6 | `held_flatpaks_can_never_be_applied` | the hold rule is structural, not incidental |
| 7 | `to_pending_marks_missing_versions_honestly` | absent versions render `?`, never a fake value |
| 8–10 | sources.toml round-trips | `flatpak` key defaults active, round-trips, and fails closed with the others |

## Live dogfood — all passed

| # | Check | Result |
|---|-------|--------|
| L1 | `nog deactivate flatpak` | flag persisted; friendly consequence text |
| L2 | `nog update` while deactivated | flatpak line absent from source counts; pacman/AUR unaffected ✔ |
| L3 | `nog activate flatpak` | restored |
| L4 | Detection with a real pending app | `1 flatpak update(s) reported by flatpak` |
| L5 | Table row | `com.github.tchx84.Flatseal 2.4.0 → 2.4.1 · Tier 3 · 76 days past window · flatpak` — version pair, tier, window math, **and the source tag** all correct |
| L6 | Real apply (full run, 137 pacman + 1 flatpak) | `Handing off 1 app(s) to flatpak ...` → app updated; KDE's pending-update badge cleared on its own = independent confirmation |
| L7 | Second run after the #8 fix | flatpak's own transaction table and progress rendered under the banner |
| L8 | Run logging | both runs written to `~/.local/share/nog/logs/20260810 nog-update.csv` |

## Findings

- **[#8](https://github.com/jetomev/nog/issues/8) — flatpak handoff too quiet** (Javier, L6). `--noninteractive` suppressed flatpak's progress; the pacman/AUR handoffs show their work and flatpak should too. Fixed by dropping the flag (keeping `-y`), verified in L7, closed the same session. **Rule adopted for every future backend, starting with snap (C2).**
- *Test-setup note (not a defect):* flatpak refuses `--commit` deployments through polkit — the rollback needs real `sudo`. Worth knowing for any future staged-downgrade test.

## Not covered live (and why)

A *held* flatpak was never observed on real data: the only available pending
update was 83 days old, past even the Tier-1 window, and no fresh-enough
small app was at hand. The hold decision itself is shared code
(`holds::evaluate_candidate`, already covered), and the flatpak-specific half —
what may enter the transaction — is now pinned by tests 5 and 6, which is
stronger than a one-off observation.
