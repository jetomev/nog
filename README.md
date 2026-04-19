# 📦 nog

> A tier-aware package manager for Arch Linux — pacman with a safety net, written in Rust.

![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)
![Platform: Linux](https://img.shields.io/badge/Platform-Linux-lightgrey.svg)
![Base: Arch Linux](https://img.shields.io/badge/Base-Arch%20Linux-1793d1.svg)
![Language: Rust](https://img.shields.io/badge/Language-Rust-dea584.svg)
![Status: Alpha](https://img.shields.io/badge/Status-Alpha-orange.svg)
![Version: 0.10.0](https://img.shields.io/badge/Version-0.10.0-purple.svg)
[![AUR](https://img.shields.io/aur/version/nog)](https://aur.archlinux.org/packages/nog)

---

## Why nog?

Arch Linux is fast, current, and beautifully simple. But rolling releases treat every package the same — when an update is available, it gets installed. Your kernel and core libraries update automatically alongside a trivial icon theme. One bad kernel update and your machine doesn't boot.

There is no safety net. One bad sync and you're in single-user mode at 2 AM.

**nog exists to change that.**

nog is a thin, readable Rust wrapper around pacman that adds a single idea: **not all packages deserve equal urgency**. Every package on your system belongs to one of three tiers, and each tier has its own update rules. The kernel, bootloader, and glibc sit behind a longer hold window. Your desktop environment gets a shorter one. Everything else flows through quickly.

We believe managing your system should be:
- **Safe** — critical packages are never updated without a community-tested buffer
- **Transparent** — nog is a pacman wrapper, not a replacement; no magic, no surprises
- **Familiar** — if you know pacman, you know nog; same commands, same flags, same mental model
- **Readable** — the whole source is a few hundred lines of Rust, deliberately simple

nog was born from a simple frustration: why does Arch give you everything except control over _which_ updates reach you and _when_? It doesn't have to be that way.

---

## Features

- 🎚 **Three-tier package classification** — every package is Tier 1, Tier 2, or Tier 3
- 🕒 **Date-based hold windows** — 30 / 15 / 7 day holds let community testing surface regressions before updates land on your machine
- 🔒 **Tier 1 protection** — kernel, bootloader, glibc, systemd, mesa held for 30 days by default; expert mode swaps to manual-only promotion
- 📦 **Status-grouped update output** — every `nog update` groups pending upgrades into **Ready / Held / Unknown** with Catppuccin Mocha tier colors
- 🧩 **AUR helper integration** — auto-detects `yay` or `paru`; AUR pending upgrades are classified and bucketed alongside official repo packages, transactions are handed off to the helper for combined `-Syu`
- ❓ **Interactive Unknown handling** — packages with no sync-DB build date (AUR-only, locally-built, disabled-repo) are prompted case-by-case
- 🧑 **No-sudo rule** — run `nog` as your user; it escalates to root only via `sudo pacman` and `sudo tee /etc/nog/tier-pins.toml`. See [Privilege model](#privilege-model--what-nog-touches-and-when) below.
- ⚡ **Tier 3 fast track** — everything else flows through pacman on a short hold
- 🎨 **Color-coded search** — every `nog search` result tagged with its tier
- 📌 **Persistent tier pinning** — `nog pin <pkg> --tier=<N>` writes to `/etc/nog/tier-pins.toml`
- 🔓 **Promote escape hatch** — `nog unlock <pkg> --promote` force-upgrades a held Tier 1 package now
- 🛡 **Pacman-native** — uses `pacman --ignore` for holds, no patching or shadowing
- 📖 **Man page included** — `man nog` for full reference

---

## The Three-Tier System

Every package nog manages falls into one of three tiers. Tier assignments live in `/etc/nog/tier-pins.toml` and can be adjusted at any time with `nog pin`. Hold durations live in `/etc/nog/nog.conf`.

### Tier 1 — 30-Day Hold (auto-release by default)
The most critical packages on your system. Updates are held for **30 days** after upstream publish date — a full month of community testing before an update reaches your machine. Once the hold expires, the update flows through `nog update` like any other package.

**Default Tier 1 packages:**
`linux`, `linux-zen`, `linux-lts`, `linux-hardened`, `systemd`, `systemd-libs`, `glibc`, `grub`, `efibootmgr`, `mkinitcpio`, `pacman`, `mesa`

> **Expert mode.** Set `manual_signoff = true` under `[tier1]` in `tier-pins.toml` to switch Tier 1 off the auto-release and require explicit `nog unlock <pkg> --promote` for every kernel/glibc/systemd update. Recommended only if you want to personally eyeball every critical upgrade.

### Tier 2 — 15-Day Hold
Key desktop applications and system services. Updates are held for **15 days** — enough time for major regressions to surface, not so long that you fall behind.

**Default Tier 2 packages:**
`plasma-meta`, `plasma-desktop`, `sddm`, `pipewire`, `pipewire-pulse`, `wireplumber`, `networkmanager`, `firefox`, `dolphin`, `konsole`, `kate`, `grubforge`, `alacritty`, `fish`, `alacrittyforge`

### Tier 3 — 7-Day Hold
Everything else. Updates are held for **7 days** — a short safety buffer without meaningful delay.

---

## Requirements

- Arch Linux (or Arch-based distribution)
- `pacman` and `pacman-contrib`
- `yay` or `paru` — optional; enables AUR support. nog functions without one; official repos only.
- Rust toolchain (only for building from source)

---

## Installation

### AUR (recommended)

nog is available on the Arch User Repository:
[https://aur.archlinux.org/packages/nog](https://aur.archlinux.org/packages/nog)

```bash
yay -S nog
```

### From source

```bash
git clone https://github.com/jetomev/nog.git
cd nog
cargo build --release
sudo install -Dm755 target/release/nog /usr/bin/nog
sudo install -Dm644 config/nog.conf /etc/nog/nog.conf
sudo install -Dm644 config/tier-pins.toml /etc/nog/tier-pins.toml
sudo install -Dm644 nog.1 /usr/share/man/man1/nog.1
```

### System files installed

| File | Location | Description |
|------|----------|-------------|
| `nog` binary | `/usr/bin/nog` | The nog executable |
| `nog.conf` | `/etc/nog/nog.conf` | Main configuration file |
| `tier-pins.toml` | `/etc/nog/tier-pins.toml` | Tier 1/2/3 package assignments |
| `nog.1` | `/usr/share/man/man1/nog.1` | Man page |

---

## Usage

> Run `nog` as your regular user. nog escalates via `sudo` only where genuinely required; you'll see the prompt at that moment. See [Privilege model](#privilege-model--what-nog-touches-and-when).

```bash
# Install a package (respects tier rules, routes to AUR helper if needed)
nog install <package>

# Update the system (tier holds applied; AUR included when a helper is configured)
nog update

# Search with tier annotations
nog search <query>

# Pin a package to a specific tier
nog pin <package> --tier=<1|2|3>

# Force-upgrade a held Tier 1 package
nog unlock <package> --promote

# Remove a package
nog remove <package>

# Version
nog --version

# Help
nog --help
```

### How `nog update` works

When you run `nog update`, nog:

1. Calls `checkupdates` (pacman-contrib) to get the list of pending **official repo** upgrades — no sync-DB side effects
2. If an AUR helper is configured, calls `<helper> -Qua` to append pending **AUR** upgrades to the same list
3. Classifies each pending package and evaluates its hold window using the sync-DB build date (AUR packages have no sync-DB entry and fall into the Unknown bucket for now)
4. Groups the result into three buckets:
   - **Ready to install** — hold expired, safe to upgrade
   - **Held** — either still inside the hold window, or Tier 1 under `manual_signoff = true`
   - **Unknown** — no build date in any enabled sync DB (AUR-only, locally-built, or disabled repo)
5. For each **Unknown** package, prompts `update anyway? [y/N]`
6. Hands off the transaction:
   - With helper: `<helper> -Syu --ignore=<held + skipped-unknowns>` — one combined upgrade for official + AUR. The helper runs as your user and sudo-s pacman internally for the pacman step.
   - Without helper: `sudo pacman -Syu --ignore=<...>` — official repos only.
7. If everything is held, exits cleanly without invoking anything.

All classification happens before the transaction, so you always see the plan before anything is touched.

### Example: `nog search`

```
extra/firefox 138.0-1 [Tier 2 — 15d hold]
    Fast, Private & Safe Web Browser
extra/linux-zen 6.19.10-1 [Tier 1 — 30d hold]
    The Linux ZEN kernel
extra/htop 3.4.1-1 [installed] [Tier 3 — 7d hold]
    Interactive process viewer
```

### Example: `nog update`

```
nog: checking for pending updates...

Ready to install (2):
  libmpc          1.4.0-1  -> 1.4.1-1   [Tier 3 · 24 days past window]
  lib32-libngtcp2 1.22.0-1 -> 1.22.1-1  [Tier 3 · 14 days past window]

Held (2):
  linux           6.19.10-1 -> 6.19.11-1 [Tier 1 · 22 days remaining]
  firefox         138.0-1   -> 138.0.2-1 [Tier 2 · 11 days remaining]

Unknown (1):
  my-local-pkg    0.9-1 -> 1.0-1 [Tier 3 · no build date in sync DB]

nog: 1 package(s) have no build date in any sync DB.
      This usually means an AUR-only, locally-built, or disabled-repo package.

  my-local-pkg (Tier 3 0.9-1 -> 1.0-1) — update anyway? [y/N] n

nog: handing off to pacman...
:: Starting full system upgrade...
```

(Package names are tier-colored — Tier 1 red, Tier 2 yellow, Tier 3 green — using the Catppuccin Mocha palette.)

---

## Screenshots

*Terminal screenshots coming in v1.0*

---

## Configuration

nog reads two configuration files from `/etc/nog/`.

### `nog.conf`

General nog settings — version, logging, paths, and **the authoritative hold durations** for each tier.

```toml
[general]
version = "0.10.0"
log_level = "info"

[paths]
tier_pins = "/etc/nog/tier-pins.toml"
pacman_conf = "/etc/pacman.conf"
log_file = "/var/log/nog.log"

[holds]
tier1_days = 30
tier2_days = 15
tier3_days = 7

[aur]
# AUR helper to use for AUR-only packages and AUR update detection.
#   "auto" — prefer yay, fall back to paru, skip AUR support if neither installed
#   "yay"  — require yay; error if not installed
#   "paru" — require paru; error if not installed
#   "none" — disable all AUR-aware paths (official repos only)
helper = "auto"
```

### `tier-pins.toml`

The tier assignment file — who goes in Tier 1, Tier 2, or Tier 3. Anything not listed here falls into Tier 3 by default. As of v0.8.0, the obsolete `hold_days` field has been removed — hold durations are owned by `nog.conf`'s `[holds]` section (single source of truth).

```toml
[tier1]
# false (default): Tier 1 auto-updates after the 30-day hold window.
# true (expert):   Tier 1 stays wholesale held until `nog unlock <pkg> --promote`.
manual_signoff = false
packages = [
    "linux",
    "linux-zen",
    "systemd",
    "glibc",
    "grub",
    "mesa",
    # ...
]

[tier2]
manual_signoff = false
packages = [
    "plasma-desktop",
    "firefox",
    # ...
]

[tier3]
manual_signoff = false
# everything not listed above falls here automatically
```

The `manual_signoff` field is only meaningful on `[tier1]`. Tier 2 and Tier 3 do not consult it.

---

## Project Structure

```
nog/
|-- Cargo.toml                 # Package manifest — dependencies, metadata
|-- src/
|   |-- main.rs                # Entry point, CLI definition via clap
|   |-- commands/
|   |   |-- mod.rs             # All subcommand implementations
|   |-- tiers.rs               # Tier classification engine
|   |-- pacman.rs              # pacman subprocess wrapper (invokes sudo pacman)
|   |-- aur.rs                 # AUR helper detection (yay / paru) + delegation
|   |-- sync_db.rs             # pacman sync-DB reader (build-date lookup)
|   |-- holds.rs               # Hold-status evaluator (pure function)
|   |-- config.rs              # Config loader
|-- config/
|   |-- nog.conf               # Default nog configuration
|   |-- tier-pins.toml         # Default tier assignments
|-- nog.1                      # Man page
|-- PKGBUILD                   # AUR package build file
|-- LICENSE                    # GPL v3
```

---

## Safety Philosophy

nog is built around one principle: **never surprise the user with a kernel update**.

Every system action goes through three layers of protection:

1. **Classification** — every package is assigned a tier before any operation
2. **Transparency** — holds, their remaining duration, and their reason are always reported before a change is made
3. **Pacman-native enforcement** — holds use pacman's own `--ignore` mechanism, so there is no way for nog to silently bypass them

Explicit commands (`install`, `remove`, `pin`) execute the user's intent without gating — tier protection lives in the passive path (`update`). Installing `linux-lts` is always allowed; what's governed is when the *next* kernel update lands on your machine.

nog does not replace pacman. It does not patch pacman. It does not shadow pacman commands. It is a small, readable wrapper — you can read the entire source in an afternoon.

---

## Privilege model — what nog touches and when

nog is designed so that you **never need to invoke it with `sudo`**. It runs as your regular user and only escalates to root at the specific moments where root is genuinely required. Every elevation is visible — you will see the `sudo` password prompt when it happens.

### The rule

Run `nog` as your user. Never `sudo nog`.

If you forget and prefix `sudo` while an AUR helper is configured, nog detects it (via `$SUDO_USER`/`$SUDO_UID`) and exits with a clear error. This is a hard stop because `yay` and `paru` both refuse to run as root. Without a helper configured, `sudo nog` still works — `sudo`-as-root is a no-op passthrough — but it isn't necessary.

### When nog escalates

nog invokes `sudo` in exactly two places. Both are transparent to the user (you see the prompt directly):

| Operation            | Command invoked                               | When |
|----------------------|-----------------------------------------------|------|
| Package transactions | `sudo pacman -S \| -R \| -Syu ...`            | `install`, `remove`, `update`, `unlock --promote` — **only when no AUR helper is configured**. When a helper is configured, nog calls the helper (as your user) and the helper runs its own `sudo pacman` internally. |
| Tier-pin writes      | `sudo tee /etc/nog/tier-pins.toml`            | Only during `nog pin`. The new file contents are rebuilt in memory and piped through `sudo tee`. nog itself never runs as root; only `tee` does. |

That is the complete list. nog never invokes `sudo` anywhere else.

### Files nog reads (no elevation)

All of these are world-readable on a standard Arch install, so nog reads them as your user:

- `/etc/nog/nog.conf` — nog main configuration
- `/etc/nog/tier-pins.toml` — tier assignments
- `/etc/pacman.conf` — for repo enablement and priority ordering
- `/var/lib/pacman/sync/*.db` — sync DBs, for package build-date lookup

### Files nog writes (elevated)

Exactly one file is ever written by nog itself:

- `/etc/nog/tier-pins.toml` — written via `sudo tee` during `nog pin`. No other persistent file is created or modified by nog.

### What nog does NOT touch

The entire rest of your system is out of scope:

- `/etc/pacman.conf` — never modified
- `/etc/pacman.d/**` (mirrorlists, etc.) — never modified
- `/var/lib/pacman/local/**` — pacman's own installed-package state; nog never touches it
- `/var/lib/pacman/sync/**` — read-only access for date lookups
- `/var/cache/pacman/**` — never touched
- Pacman's GPG keyring and signature verification — unmodified; every transaction runs through pacman's own checks
- `/etc/sudoers`, PAM configuration, any other auth state — never touched
- `/usr/bin`, `/usr/lib`, or any other system binary location — never touched directly; pacman and the helper own these paths

nog does not shadow, patch, or replace `pacman`. It is purely a wrapper that calls `pacman` (or an AUR helper) as a subprocess. Every install, remove, and upgrade goes through pacman's signature verification and conflict resolution — nog cannot bypass them.

### AUR helper integration

When `[aur] helper` in `nog.conf` resolves to `yay` or `paru`:

- nog calls `<helper> -Qua` (as your user) to list AUR pending upgrades
- nog calls `<helper> -S ...` (as your user) for installs, or `<helper> -Syu --ignore=...` for the combined upgrade
- The helper fetches PKGBUILDs and runs `makepkg` as your user
- The helper runs `sudo pacman` internally when it reaches its pacman steps — that `sudo` prompt comes from the helper, not from nog

nog never invokes `sudo yay` or `sudo paru`. That is a deliberate refusal — both helpers refuse to run as root precisely because `makepkg` needs to run as a non-root user.

### In one paragraph

nog runs as your user. It escalates exactly twice: `sudo pacman` for package transactions, and `sudo tee /etc/nog/tier-pins.toml` for the one file it ever writes. It never modifies any other file on your system, never bypasses pacman's signature verification, and never runs as root itself. If a helper is configured, transactions are handed off to `yay` or `paru` as your user, and those helpers escalate themselves.

---

## Roadmap

### v0.10.0 — Current
- [x] CLI skeleton with all subcommands
- [x] Three-tier classification engine
- [x] Real pacman subprocess integration
- [x] `nog search` with color-coded tier annotations
- [x] System-wide install at `/usr/bin/nog`
- [x] `nog pin` with persistent tier changes to `tier-pins.toml`
- [x] AUR package
- [x] Man page
- [x] **Phase 1 — sync DB reader** — reads every enabled pacman sync database (gzip + zstd), extracts build dates for all packages across all repos
- [x] **Phase 2 — hold evaluation logic** — pure function returning Expired / Holding / Unknown for any package; 6 unit tests; 30/15/7 day windows live in `nog.conf`
- [x] **Phase 3 — wired into `nog update`** — `checkupdates` integration, status-grouped output (Ready / Held / Unknown) with Catppuccin Mocha tier colors, interactive y/N prompt for Unknowns, `manual_signoff` honored as Tier 1 expert-mode toggle, Tier 1 install block removed
- [x] **Phase 4 — AUR helper detection** — auto-detects `yay` / `paru`; AUR pending upgrades fold into the status-grouped output; transactions hand off to the helper for combined `-Syu`; one consistent no-sudo rule; `nog pin` writes via `sudo tee`; root-guard catches `sudo nog` invocations when a helper is configured

### v1.0 — In Progress
- [x] ~~Phase 1 — sync DB reader with gzip + zstd support~~ ✅
- [x] ~~Phase 2 — hold evaluation logic~~ ✅
- [x] ~~Phase 3 — wire into `nog update`~~ ✅
- [x] ~~Phase 4 — AUR helper detection~~ ✅
- [ ] **Phase 5 — polish** — full man page refresh, updated help text, terminal screenshots, CHANGELOG finalization, AUR build-date lookup via AUR RPC (lets real hold windows apply to AUR packages instead of bucketing them as Unknown)

### Future
- [ ] **First-run wizard** — on first `nog update`, ask the user whether Tier 1 should auto-update after 30 days (default, novice-friendly) or require manual `unlock --promote` per kernel/glibc/systemd upgrade (expert mode). Writes the chosen value to `tier-pins.toml [tier1] manual_signoff`.
- [ ] Chaotic-AUR binary package (submit once v1.0 is stable)
- [ ] `nog history` — log of all tier changes and package actions
- [ ] `nog status` — dashboard showing what's held, what's ready, what's overdue
- [ ] `nog rollback` — revert a recent update using pacman cache
- [ ] Hook support for notifying a GUI companion like `nogforge`

---

## Changelog

### v0.10.0 — April 18, 2026
**Phase 4 — AUR helper integration + unified no-sudo privilege model**
- 🧩 New `aur` module — helper detection (`yay` → `paru` → `none`) driven by `[aur] helper` in `nog.conf`. Supports `"auto"`, `"yay"`, `"paru"`, `"none"`; hard-errors if the user requests a specific helper that isn't installed
- 📦 `nog update` folds AUR pending upgrades (`<helper> -Qua`) into the existing status-grouped output alongside official repo packages from `checkupdates`. AUR packages bucket as Unknown for now (no sync-DB build date); the y/N prompt already handles them correctly
- 🔄 `nog update` transaction handoff routes through the helper when configured (`<helper> -Syu --ignore=...`) for a single combined official+AUR upgrade. Without a helper, pacman handoff is unchanged
- 📥 `nog install <pkg>` routes through the helper when configured, so AUR-only packages "just work" without a pre-check. The helper resolves sync repos before AUR automatically
- 🔓 `nog unlock --promote` similarly routes through the helper when configured
- 🧑 **No-sudo rule** — single consistent UX: run `nog` as your user. `pacman.rs` now invokes `sudo pacman` internally; `tiers::pin_package` writes `/etc/nog/tier-pins.toml` via `sudo tee`. `nog pin` no longer needs shell-level sudo. Fully backwards-compatible: `sudo nog <cmd>` still works for non-helper paths (sudo-as-root passes through)
- 🛑 **Root-guard** — if nog is invoked via sudo (detected via `$SUDO_USER`/`$SUDO_UID`) *and* a helper is configured, it exits with a clear message pointing the user to drop the `sudo`. Necessary because `yay`/`paru` refuse to run as root
- 📖 **New "Privilege model" section in README** — documents exactly where nog escalates (`sudo pacman`, `sudo tee /etc/nog/tier-pins.toml`), which files it reads without elevation, the single file it ever writes, and the comprehensive list of system files it never touches (pacman.conf, pacman.d, /var/lib/pacman/local, keyring, sudoers, etc.)
- 📜 Man page gains a targeted **PRIVILEGES AND SUDO** section mirroring the README content; version header bumped to 0.10.0; EXAMPLES dropped their `sudo` prefixes. Full man page rewrite (command descriptions, tier metadata) deferred to Phase 5 polish
- ℹ No regressions in existing behavior: 6/6 hold tests still green, 7 warnings (unchanged since Phase 3)

### v0.9.0 — April 18, 2026
**Phase 3 — wired into `nog update` (the tier system goes live)**
- 🔌 `nog update` now calls `checkupdates` (pacman-contrib) to list pending upgrades *without* the `-Sy` side effect, then classifies every pending package against its tier's hold window
- 📊 **Status-grouped output**: three labelled buckets — `Ready to install`, `Held`, `Unknown` — each showing package name, version bump, tier, and either "N days past window", "N days remaining", or "no build date in sync DB"
- 🎨 Tier-colored output using the **Catppuccin Mocha** palette (Tier 1 red `#F38BA8`, Tier 2 yellow `#F9E2AF`, Tier 3 green `#A6E3A1`) — muted subtext color `#A6ADC8` for version/metadata
- ❓ Interactive `[y/N]` prompt per Unknown package (AUR-only, locally-built, or disabled-repo); EOF / non-TTY stdin auto-skips remaining Unknowns with a warning instead of hanging
- 🎚 **Tier 1 policy change, novice-friendly default:** `manual_signoff` now defaults to `false` — Tier 1 auto-updates once the 30-day hold expires. Expert users can set `manual_signoff = true` to restore always-held-until-promoted behavior
- 🔓 `nog unlock <pkg> --promote` kept as the expert-mode escape hatch: force-upgrade a held Tier 1 package right now, bypassing the hold and `manual_signoff`
- 🗑 **Tier 1 install block removed** — `nog install linux-lts` now proceeds normally; tier classification is shown as informational output only. Explicit user commands execute user intent; tier protection lives in the passive update path
- 🧹 `nog unlock` without `--promote` now honestly reports it has no session state to toggle, and points the user at `--promote` for the real action
- ⚠ Warnings reduced to 7 — previously-unused `is_manual_signoff` method is now live; the orphaned `tier1_packages()` helper was removed

### v0.8.0 — April 18, 2026
**Phase 2 — Hold evaluation logic (the date-math engine)**
- 🧮 New `holds` module with a pure `evaluate()` function — given a package, tier, build-date map, and hold config, returns one of `Expired { days_past_window }`, `Holding { days_remaining }`, or `Unknown`
- ✅ 6 unit tests covering all three states, the exact-window boundary, partial-day rounding (ceiling per spec), and future-dated-package edge cases
- 🔒 All inputs explicit including `now: SystemTime` — tests run deterministically, no hidden clock dependency
- 🗓 **New hold spec live in `nog.conf`:** Tier 1 = 30 days, Tier 2 = 15 days, Tier 3 = 7 days
- 🧹 Removed obsolete `hold_days` field from `tier-pins.toml` — hold durations now owned exclusively by `nog.conf [holds]` (single source of truth)
- 🔧 `tiers.rs` cleanup: dropped `hold_days` field and method, simplified `Display` for `Tier` enum, removed unused `std::path::Path` import
- 🧪 Hidden `_debug-hold <package>` subcommand added for internal verification — classifies, looks up build date, evaluates hold, prints result
- ⚠ Warnings reduced from 11 to 9 — previously-unused `HoldsConfig` fields are now active
- ℹ This phase adds no user-visible commands. The `_debug-hold` tool is hidden from `--help`. Phase 3 will wire this evaluator into `nog update`.

### v0.7.0 — April 18, 2026
**Phase 1 — Sync DB reader (foundation for date-based holds)**
- 🧱 New `sync_db` module reads every enabled pacman sync database and builds a map of package → build-date Unix timestamp
- 🗜 Auto-detects **gzip** (core, extra, multilib) and **zstd** (Chaotic-AUR and similar) compression via magic-byte sniffing
- 📋 Respects `pacman.conf` repo priority — first repo wins on name collisions, matching pacman's own resolution
- 🛡 Graceful fallback when `pacman.conf` is unreadable — scans the sync directory directly
- 📦 Indexes **18,000+** packages across all enabled repos on a standard Arch install
- 🧪 Verified against `pacman -Si` output for official and Chaotic-AUR packages — exact timestamp match
- ➕ Dependencies added: `flate2`, `tar`, `zstd`
- 🔢 Version bumped to 0.7.0 to mark v1.0 development in progress
- ℹ This phase adds no user-visible commands. It is infrastructure for Phase 2 and onward.

### v0.6.0 — April 7, 2026
**AUR package + man page**
- 📦 `nog` available on the AUR — install with `yay -S nog`
- 📖 Man page added — `nog.1` installed to `/usr/share/man/man1/`
- 🔢 Version now reads from `CARGO_PKG_VERSION` — no hardcoded strings
- 📋 PKGBUILD installs binary, config files, license, and man page

### v0.5.0 — April 5, 2026
**`nog pin` — persistent tier changes**
- 📌 `nog pin <package> --tier=<1|2|3>` writes changes to `/etc/nog/tier-pins.toml`
- ➕ Pinning to Tier 1 or 2 adds the package to the correct section
- ➖ Pinning to Tier 3 removes it from Tier 1/2 — Tier 3 is the default, no entry needed
- ♻ Changes survive reboots and are immediately reflected in `nog search` annotations

### v0.4.0 — April 5, 2026
**`nog update` — Tier 1 properly excluded**
- 🛡 `nog update` passes Tier 1 packages to pacman via `--ignore` flags
- 🔒 Tier 1 packages are genuinely untouchable during a system upgrade
- ✅ Confirmed: system upgraded 14 packages, zero Tier 1 packages touched

### v0.3.0 — April 4, 2026
**`nog search` + system install**
- 🎨 `nog search` shows color-coded tier annotations for every result
- 📂 Installed system-wide with config files at `/etc/nog/`
- 🚀 `nog` callable from anywhere on the system without a path

### v0.2.0 — March 25, 2026
**Tier system + real pacman calls**
- 🎚 Three-tier classification engine fully implemented in `tiers.rs`
- 📋 `tier-pins.toml` defines all Tier 1/2/3 package assignments
- 🔌 `pacman.rs` wires real subprocess calls — nog installs, removes, and updates for real
- ⛔ `nog install` blocks Tier 1 packages with a clear error message
- 🔓 `nog unlock --promote` allows manual Tier 1 upgrades
- ⚙ `config.rs` reads `/etc/nog/nog.conf` with graceful fallback

### v0.1.0 — March 25, 2026
**Initial release — nog CLI skeleton**
- 🦀 Rust CLI using clap with derive macros
- 📝 All subcommands defined: install, remove, update, search, pin, unlock
- 🏗 Three-tier architecture designed and stubbed

---

## Related Projects

### KognogOS
The parent distribution where nog is the native package manager. Arch-based, KDE Plasma on Wayland, Zen kernel, Catppuccin Mocha.
[https://github.com/jetomev/KognogOS](https://github.com/jetomev/KognogOS)

### nogforge
A TUI for managing nog, plus unified interface for AUR helpers, Flatpak, and Snap. Built on top of nog to extend it into a full graphical package management experience.
[https://github.com/jetomev/nogforge](https://github.com/jetomev/nogforge)

### GrubForge
A TUI for managing the GRUB bootloader. Ships with KognogOS, pinned to Tier 2.
[https://github.com/jetomev/grubforge](https://github.com/jetomev/grubforge)

### AlacrittyForge
A TUI for managing and customizing the Alacritty terminal emulator. Ships with KognogOS, pinned to Tier 2.
[https://github.com/jetomev/alacrittyforge](https://github.com/jetomev/alacrittyforge)

---

## Authors

**jetomev** — idea, vision, direction, testing

**Claude (Anthropic)** — co-developer, architecture, implementation

This project is a collaboration between a human with a clear vision for what Linux package management should feel like, and an AI that helped design and build the tools to make it real — one command at a time.

---

## License

nog is free software: you can redistribute it and/or modify it under the terms of the **GNU General Public License v3.0** as published by the Free Software Foundation.

See [LICENSE](LICENSE) for the full license text.

---

## Contributing

nog is in early alpha. Ideas, feedback, and contributions are welcome — open an issue or pull request on GitHub.

If this project resonates with you, consider starring the repository. It helps others find it and motivates continued development.