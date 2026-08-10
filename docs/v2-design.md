# nog v2 — the multi-source arc (design)

*Javier (jetomev) & Claude · locked 2026-08-10 · simple words on purpose*

nog grows from "pacman wrapper with tiers" into **the one package manager for
KognogOS**: official repos, chaotic-aur, AUR, Flatpak, and Snap behind one
command, one tier model, and one visual app (nogForge). Shipped as v1.x
increments, released on GitHub as they land; **v2.0.0** is the crown once the
whole chain works.

## The rulings (Javier, 2026-08-10)

1. **Backends are on-demand.** Flatpak and snapd are never required. On first
   run (or first need) nog *offers* to install a missing backend — same way it
   treats the AUR helper and chaotic-aur. After that, everything goes through
   nog commands.
2. **The install chain.** An install request tries sources in this order:
   **pacman (incl. chaotic-aur) → AUR → Flatpak → Snap** — falling through
   only when a source doesn't have the package. The user **always sees the
   source** in the table before saying yes.
3. **Tiers apply to everything.** Flatpaks and snaps get tier aging like any
   package (default Tier 3). Expectation: Tier 1/2 packages live in
   pacman/chaotic/AUR; Flatpak covers most of the rest; Snap is the ~1% tail.
4. **Snap is pragmatic, not ideological.** We're Arch-based, not purists. If
   the chain reaches snapd and the user says install — their machine, their
   choice. We just provide the road.
5. **Method:** one step at a time, 101 style; live ToDo list on screen; walls
   become GitHub issues (opened when hit, closed when solved); document as we
   go; release increments as they're ready. No deadline pressure.

## Architecture rules

- **`--json` on every new query command** from day one. nogForge consumes
  structured output only — no scraping of pretty tables. (Human tables and
  JSON come from the same data; the tables never change behavior.)
- **`sources.toml` is the one switchboard.** `aur` and `chaotic-aur` (v1.0.9)
  are joined by `flatpak` and `snap`. `nog activate|deactivate <source>`
  covers all four; missing file = defaults, unparseable = fail closed.
- **Fail closed everywhere** — an unreachable backend means "hold", never
  "assume fine" (the v1.0.9 lesson).

## The cycles

| Cycle | Ships | Content |
|---|---|---|
| **C1** | v1.1.0 | **Flatpak backend**: detect/offer, `activate/deactivate flatpak`, flatpak updates in `nog update` tables (Source column) + gated apply, tier aging on flatpak refs |
| **C2** | v1.2.0 | **Snap backend**: same pattern, detect-if-present (snapd is AUR-only — offered, never demanded) |
| **C3** | v1.3.0 | **The install chain**: `nog install` falls through pacman→AUR→Flatpak→Snap, pre-install table shows source, first-need backend offers |
| **C4** | v1.4.0 | **Command surface + JSON**: `info`, `search` (cross-source), `remove`, `reinstall`, `tier`, `lock`, `status`, `history` (reads the v1.0.8 CSVs) — every one with `--json` |
| **C5** | v1.5.0 | **Maintenance & cleanup** (Javier, 2026-08-10): one command that reports reclaimable space per source and cleans on confirmation — pacman orphans + package cache, AUR helper build cache, **flatpak unused runtimes**, **snap old revisions**. Report-then-gate, like `nog update`; `--json` for the TUI |
| **C6** | nogForge v0.2.0 | **The visual handler** on forgekit: Dashboard (tier status, last run), Installed (details/reinstall/remove/tier-move/lock — staged, one Apply), Search (cross-source, target tier), **Maintenance** (reclaimable space per category, pick what to clean, one Apply), Config (four source toggles). In-place editing design language throughout |
| **C7** | nog v2.0.0 | Crown release: docs sweep, man page, Tier Reference update, announcement |

Open items carried alongside (slot into cycles when natural): #5 tier-ABI-skew
linchpin heuristic, promote-family gap, first-run wizard, **#9 reboot
recommendation** (Javier, 2026-08-10 — after installing kernel / systemd /
glibc / mesa / nvidia / dkms packages, end the run with
`IMPORTANT: It is highly recommended to reboot the system!` and say why;
Tier 1 already *is* most of that list. Found the hard way: an nvidia update
silently broke every 3D app until reboot).

## Maintenance — what "cleanup" means per source (C5)

| Source | Garbage it accumulates | How nog reclaims it |
|---|---|---|
| pacman | orphaned dependencies; every downloaded package version, forever | `pacman -Qtdq` → remove; cache trimmed keeping the last N versions (never all — the last versions are the downgrade path) |
| AUR helper | cloned build trees and built packages | helper cache directory |
| flatpak | **unused runtimes** (a 400 MB GNOME platform can outlive the one app that needed it) | `flatpak uninstall --unused` |
| snap | old disabled revisions (snapd keeps several by default) | remove revisions beyond the retain count |

Rules: **report first, act on confirmation** (the `nog update` shape); never
remove the newest cached version of anything; each category independently
selectable; nothing outside package management (system logs, browser caches
and friends are not nog's business).

## What nogForge is (and stays)

A **visual handler for nog** — nothing more. Everything it shows is a nog
command's `--json`; everything it does is a nog command. If nogForge ever
needs something nog can't say or do, that's a nog cycle first.
