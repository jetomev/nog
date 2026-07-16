# 📦 nog

> A tier-aware package manager for Arch Linux — pacman with a safety net, written in Rust.

![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)
![Platform: Linux](https://img.shields.io/badge/Platform-Linux-lightgrey.svg)
![Base: Arch Linux](https://img.shields.io/badge/Base-Arch%20Linux-1793d1.svg)
![Language: Rust](https://img.shields.io/badge/Language-Rust-dea584.svg)
![Status: Stable](https://img.shields.io/badge/Status-Stable-brightgreen.svg)
![Version: 1.0.6](https://img.shields.io/badge/Version-1.0.6-purple.svg)
[![AUR](https://img.shields.io/aur/version/nog?color=1793d1&cacheSeconds=1800)](https://aur.archlinux.org/packages/nog)

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
- 🧩 **AUR helper integration** — auto-detects `yay` or `paru`; AUR pending upgrades are classified, date-evaluated (via the helper's cached metadata), and bucketed alongside official repo packages; transactions are handed off to the helper for combined `-Syu`
- ❓ **Interactive Unknown handling** — packages with no resolvable build date (locally-built, disabled-repo, or AUR query failure) are prompted case-by-case
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

### Tier coupling — kernel ↔ headers ↔ DKMS

A kernel package (`linux`, `linux-zen`, `linux-lts`, `linux-hardened`) and its matching `*-headers` package are produced from the same PKGBUILD — they share a single build date and must always be installed at the same version, because every DKMS module (e.g. `nvidia-open-dkms`) is rebuilt against whichever `<kernel>-headers` is on disk and then placed under `/usr/lib/modules/<KVER>/`. If headers move ahead of the kernel, the next DKMS rebuild has nowhere to land and the GPU driver fails to load after reboot.

To prevent this, nog **automatically couples `<X>-headers` to its base kernel's tier**. If `linux-zen` is Tier 1, then `linux-zen-headers` is treated as Tier 1 too — they hold together, they release together. This is hardcoded behavior: not configurable, always on. The Arch naming convention is universal and the failure mode is severe.

**For non-standard kernel names** (`linux-cachyos-cacule-headers` etc.), the `<X>-headers` pattern doesn't apply directly. Use the optional `[groups]` table in `/etc/nog/tier-pins.toml` to bundle them explicitly:

```toml
[groups]
cachyos-bundle = [
    "linux-cachyos",
    "linux-cachyos-headers",
    "linux-cachyos-cacule-headers",
]
```

Every member of a group inherits the highest tier present among any other member. The same mechanism can pull additional packages into a kernel's tier (e.g. `linux + nvidia-utils + nvidia-open-dkms` if you want maximally cautious GPU handling).

**DKMS modules themselves are not coupled.** They don't need to be — once kernel and headers are coherent, DKMS rebuilds succeed automatically.

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
3. Loads build dates from the **same fresh DB snapshot `checkupdates` just synced** (its private dbpath, `$CHECKUPDATES_DB` or `${TMPDIR:-/tmp}/checkup-db-<uid>/`), then (for AUR packages not found in any sync DB) from the helper's cached metadata via `<helper> -Sai`. If the snapshot is missing, falls back to `/var/lib/pacman/sync` with a warning (v1.0.5 — see [changelog](#changelog))
4. Classifies each pending package and evaluates its hold window against the combined build-date map. A DB entry that isn't the pending candidate's exact version is never trusted for dating — it routes to **Unknown** instead (v1.0.5 candidate-version guard)
5. Groups the result into three buckets:
   - **Ready to install** — hold expired, safe to upgrade
   - **Held** — either still inside the hold window, or Tier 1 under `manual_signoff = true`
   - **Unknown** — no usable build date (locally-built, disabled-repo, helper lookup failed, or a DB entry that doesn't match the candidate's version)
6. For each **Unknown** package, prompts `update anyway? [y/N]`
7. Hands off the transaction:
   - With helper: `<helper> -Syu --ignore=<held + skipped-unknowns>` — one combined upgrade for official + AUR. The helper runs as your user and sudo-s pacman internally for the pacman step.
   - Without helper: `sudo pacman -Syu --ignore=<...>` — official repos only.
8. If everything is held, exits cleanly without invoking anything.

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

Held (3):
  linux             6.19.10-1 -> 6.19.11-1 [Tier 1 · 22 days remaining]
  firefox           138.0-1   -> 138.0.2-1 [Tier 2 · 11 days remaining]
  fresh-editor-bin  0.2.24-1  -> 0.2.25-1  [Tier 3 · 6 days remaining]

Unknown (1):
  my-local-pkg    0.9-1 -> 1.0-1 [Tier 3 · no build date in sync DB]

nog: 1 package(s) have no usable build date in any sync DB.
      Usually an AUR-only, locally-built, or disabled-repo package — or a
      DB entry that doesn't match the pending candidate's version.

  my-local-pkg (Tier 3 0.9-1 -> 1.0-1) — update anyway? [y/N] n

nog: handing off to yay...
:: Starting full system upgrade...
```

(Package names are tier-colored — Tier 1 red, Tier 2 yellow, Tier 3 green — using the Catppuccin Mocha palette.)

---

## Configuration

nog reads two configuration files from `/etc/nog/`.

### `nog.conf`

General nog settings — version, logging, paths, and **the authoritative hold durations** for each tier.

```toml
[general]
version = "1.0.6"
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
|-- README.md                  # This file
|-- LICENSE                    # GPL v3
|-- Cargo.toml                 # Package manifest — dependencies, metadata
|-- Cargo.lock                 # Locked dependency tree (committed for reproducible binary builds)
|-- src/
|   |-- main.rs                # Entry point, CLI definition via clap
|   |-- commands/
|   |   |-- mod.rs             # All subcommand implementations (incl. nog update --realign)
|   |-- tiers.rs               # Tier classification engine (incl. *-headers auto-coupling + [groups])
|   |-- pacman.rs              # pacman subprocess wrapper; installed-versions reader for the desync detector
|   |-- aur.rs                 # AUR helper detection (yay / paru) + delegation
|   |-- sync_db.rs             # pacman sync-DB reader (build-date lookup)
|   |-- holds.rs               # Hold-status evaluator (pure function)
|   |-- config.rs              # Config loader (OnceLock-cached)
|-- config/
|   |-- nog.conf               # Default nog configuration
|   |-- tier-pins.toml         # Default tier assignments (incl. commented [groups] example)
|-- nog.1                      # Man page
|-- PKGBUILD                   # AUR package build file (in lockstep with the latest tag)
|-- testing/                   # Per-release Test Matrix + Test Results + release checklist
|   |-- 20260513 - Test Matrix for nog v1-0-3.md
|   |-- 20260419 - Test Results for nog v1-0-0.md
|   |-- RELEASE-CHECKLIST.md   # Pre-flight gates for every release (version sync, audits, AUR flow)
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

## Troubleshooting

### `ERROR: Missing <KVER> kernel modules tree for module <name>/<version>`

You're seeing this from `nvidia-open-dkms`, `nvidia-dkms`, `virtualbox-host-dkms`, or another DKMS hook after running `nog update` (or `pacman -Syu` directly). The message means: DKMS is trying to build a kernel module against a `<KVER>` whose kernel binary is not installed at `/usr/lib/modules/<KVER>/`.

This is the **kernel / headers / DKMS desync** described in [Tier coupling](#tier-coupling--kernel--headers--dkms). Until v1.0.3, nog's Tier 1 hold applied to `linux*` packages but not their `*-headers` companions, so headers could race ahead of held kernels and break DKMS rebuilds.

**Recovery in v1.0.3:**

```sh
nog update --realign
```

The `--realign` flag pulls held kernels into the upgrade transaction when their pending version matches the installed headers, so kernel + headers end up at the same version in a single coherent step. After the transaction completes, DKMS rebuilds run with consistent inputs and the affected modules build successfully.

**Manual recovery (if `nog update --realign` doesn't apply** — e.g. headers are *ahead* of any pending kernel upgrade, or you're on v1.0.2 and haven't upgraded nog yet**):**

```sh
# 1. Pull the held kernels forward to match the installed headers.
sudo pacman -S linux-zen linux-lts            # adjust to your kernels

# 2. Reinstall the DKMS package to retrigger the build hook.
sudo pacman -S nvidia-open-dkms                # or whatever DKMS package broke

# 3. Verify the modules built.
dkms status
find /usr/lib/modules/$(uname -r)/updates/dkms -name '*.ko.zst'
```

**Verifying coupling is in effect (v1.0.3+):**

```sh
nog search linux-zen-headers
# expect: red [Tier 1 — 30d hold] annotation
```

If it shows green Tier 3, you're still on v1.0.2 or earlier — upgrade nog before you next run `nog update`.

### `nog update` shows fewer Ready / more Held packages after upgrading to v1.0.5

Expected. Pre-1.0.5, hold windows were dated from the (stale) system sync DB, so updates being seen for the first time were often waved straight into Ready with inflated "days past window" figures. v1.0.5 dates every hold from the fresh DB snapshot `checkupdates` just synced, so new updates now serve their full window. "Days remaining" on already-Held packages may also shift a few days — the clock is now measured from the candidate's true build date. See the [v1.0.5 changelog entry](#changelog).

### `nog: warning — checkupdates DB not found; using the system sync DB.`

`nog update` couldn't locate the private dbpath `checkupdates` syncs into (`$CHECKUPDATES_DB`, default `${TMPDIR:-/tmp}/checkup-db-<uid>/`). It fell back to `/var/lib/pacman/sync`, which may date holds from stale build dates (the pre-1.0.5 behavior). Likely causes: `CHECKUPDATES_DB` set for checkupdates but not visible to nog, a `TMPDIR` mismatch between the two, or a pacman-contrib update changing the default path. Check `ls "${TMPDIR:-/tmp}/checkup-db-$(id -u)/sync"` right after a run — if the layout moved, file a bug.

### `nog update` reports more Held packages after upgrading from v1.0.2

Expected. v1.0.3 re-tiers `linux-headers`, `linux-zen-headers`, `linux-lts-headers`, and `linux-hardened-headers` from Tier 3 (7-day hold) to Tier 1 (30-day hold) implicitly via the `<X>-headers` coupling rule. The first `nog update` after upgrading will surface those headers in the **Held** bucket where v1.0.2 might have shown them as Ready. This is the protection working — they will release in lockstep with their kernel.

---

## Roadmap

### Future
- [ ] **First-run wizard** — on first `nog update`, ask the user whether Tier 1 should auto-update after 30 days (default, novice-friendly) or require manual `unlock --promote` per kernel/glibc/systemd upgrade (expert mode). Writes the chosen value to `tier-pins.toml [tier1] manual_signoff`.
- [ ] Chaotic-AUR binary package (submit once v1.0 is stable)
- [ ] `nog history` — log of all tier changes and package actions
- [ ] `nog status` — dashboard showing what's held, what's ready, what's overdue
- [ ] `nog rollback` — revert a recent update using pacman cache
- [ ] Hook support for notifying a GUI companion like `nogforge`

### v1.0.6 — Released
- [x] **lib32/base hold coupling ([#1](https://github.com/jetomev/nog/issues/1))** — a `lib32-<X>` and its base `<X>` are version-locked, but their hold windows are dated independently, so one could land in **Ready** while the other stayed **Held** — leaving pacman unable to satisfy the exact-version dependency and aborting the *entire* transaction (hit live on the nvidia stack). `holds::lib32_coupling_demotions()` demotes the Ready member of any split pair into Held so the pair releases together; bidirectional, and the Held row now names the package it's waiting on. 29 → 33 tests.
- [x] **Dogfooded on the AUR binary (2026-07-16)** — verified on the same host that hit the original abort, with the exact split still pending: `lib32-nvidia-utils` moved from Ready into Held as `[Tier 3 · coupled to nvidia-utils · 3 days]`, landed in the pacman ignore list, and the transaction resolved and installed 16 packages with no abort — while non-split lib32 pairs (`fontconfig`/`libffi`/`libssh2`/`p11-kit`) correctly stayed together in Ready. [Test Results](testing/20260716 - Test Results for nog v1-0-6.md).

### v1.0.5 — Released
- [x] **Phase 8 — candidate-fresh hold evaluation** — `nog update` now dates hold windows from the **same DB snapshot that produced the candidate list**: the private dbpath `checkupdates` syncs on every run (`$CHECKUPDATES_DB`, default `${TMPDIR:-/tmp}/checkup-db-<uid>/`). Previously it read `/var/lib/pacman/sync`, which only refreshes when root syncs — i.e. during the handoff *after* the report — so every first-sighting update was dated from its *predecessor's* builddate and could skip its hold entirely (975 days "past window" in the worst observed case). Falls back to the system DB with a warning if the snapshot is missing.
- [x] **Candidate-version guard** — `sync_db.rs` now reads `%VERSION%`; the new `holds::evaluate_candidate()` refuses to date a hold from a DB entry that isn't the pending candidate's exact version — mismatches route to **Unknown** (per-package prompt) instead of borrowing a clock from a different build. Defense-in-depth behind the fresh-snapshot fix.
- [x] **Test surface** — 22 → 29 tests (4 new in `holds::tests` covering the guard, 3 new in `sync_db::tests` covering `%VERSION%` parsing); [Test Matrix](testing/20260707 - Test Matrix for nog v1-0-5.md) section 18 adds regression-guard checks for the fresh-snapshot path, the fallback warning, and the guard.
- [x] **Dogfooded on the AUR binary (2026-07-08)** — `yay -S nog` install of 1.0.5 reproduced the fix live: a morning v1.0.4 run marked 12 day-old packages "Ready" (up to *"317 days past window"*); the afternoon v1.0.5 run held all first-sighting updates with sane countdowns, matching an independent recomputation from the fresh DBs. [Test Results](testing/20260708 - Test Results for nog v1-0-5.md).

### v1.0.4 — Released
- [x] **Phase 7 — split-PKGBUILD pkgbase coupling** — generalizes v1.0.3's `*-headers` rule to all packages sharing a `pkgbase`. `sync_db.rs` now reads the `%BASE%` field from pacman's sync DBs; `TierManager` consults `PkgbaseIndex` to bucket siblings to the highest tier present in their group. Auto-handles pipewire, mesa, plasma, qt, kde-applications, and every other Arch split PKGBUILD where Arch enforces lockstep via `=` version deps. Closes the 2026-05-25 pipewire-family lockstep failure.
- [x] **Layer B — `lib32-<X>` auto-coupling** — multilib packages have their own pkgbase but are version-pinned to the main package by Arch convention. Stripping `lib32-` and inheriting the base's Tier 1 / Tier 2 tier covers cases like `mesa` ↔ `lib32-mesa` where pkgbase alone wouldn't bridge them. Composes with Layer A — `lib32-libpipewire` correctly resolves Tier 2 via its lib32-pipewire sibling.
- [x] **Layer D — `nog unlock --promote` for any tier** — v1.0.3 restricted unlock to Tier 1. v1.0.4 relaxes it: Tier 2 (15-day hold) and Tier 3 (7-day hold) packages can be promoted too. Necessary fallback if a tier-mismatched lockstep deadlock recurs in a configuration the auto-coupling doesn't catch.
- [x] **Test surface** — 14 → 22 tests (8 new in `tiers::tests`); [Test Matrix](testing/20260525 - Test Matrix for nog v1-0-4.md) section 17 adds 16 regression-guard checks across 17a (pkgbase coupling), 17b (lib32), 17c (live family-upgrade reproduction), 17d (Tier 2 unlock), 17e (no false positives).
- [x] **Dogfood (post-AUR)** — [v1.0.4 Test Results](testing/20260525 - Test Results for nog v1-0-4.md) captured on the AUR-delivered binary (no findings); pkgbase coupling, lib32- rule, and composed Layer A+B all verified live; 22/22 unit tests run in the AUR build's `check()` phase on every install.

### v1.0.3 — Released
- [x] **Phase 6 — tier coupling for headers + DKMS** — `<X>-headers` auto-inherits Tier 1 when `<X>` is Tier 1 (hardcoded, same PKGBUILD → same build date); new optional `[groups]` table in `tier-pins.toml` for non-standard kernel names or custom bundles; plan-time desync detector compares installed kernel vs. headers versions; `nog update --realign` recovers a system already in the desynced state by pulling held kernels forward to match the installed headers; 14/14 tests (8 new in `tiers::tests`); [Test Matrix](testing/20260513 - Test Matrix for nog v1-0-3.md) section 16 with 16 regression-guard checks across 16a/b/c/d
- [x] **`testing/` folder convention adopted** — per-release Test Matrix + Test Results + a nog-specific `RELEASE-CHECKLIST.md` matching the KognogOS ecosystem layout
- [x] **Dogfood (post-AUR)** — [v1.0.3 Test Results](testing/20260513 - Test Results for nog v1-0-3.md) captured on the AUR-delivered binary (no findings); coupling assertions verified live, `cargo test --release --locked` runs 14/14 green on every machine via the PKGBUILD `check()` step

### v1.0 release kit — ✅ Shipped
- [x] **PKGBUILD in tree** at repo root, kept in lockstep with the latest tag
- [x] **AUR submission** — [`ssh://aur@aur.archlinux.org/nog.git`](https://aur.archlinux.org/packages/nog) tracks releases; maintained via `~/Programs/aur-nog-remote/`
- [x] **Dogfood** — full [`Test Matrix`](testing/20260513 - Test Matrix for nog v1-0-3.md) run captured in [`v1.0 Test Results`](testing/20260419 - Test Results for nog v1-0-0.md); the dogfood surfaced the v1.0.1 zstd fix and the v1.0.2 polish batch, both validated on the AUR-delivered binary
- [x] **Release discipline** — every release now runs through local `makepkg -si` test → AUR push → uninstall + fresh AUR install verification

### v1.0 — All phases shipped
- [x] ~~Phase 1 — sync DB reader with gzip + zstd support~~ ✅
- [x] ~~Phase 2 — hold evaluation logic~~ ✅
- [x] ~~Phase 3 — wire into `nog update`~~ ✅
- [x] ~~Phase 4 — AUR helper detection~~ ✅
- [x] ~~Phase 5a — AUR build-date resolution~~ ✅
- [x] ~~Phase 5b — documentation polish (man + help)~~ ✅

### v1.0.0 — Released
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
- [x] **Phase 5a — AUR build-date resolution** — AUR pending upgrades now get real build dates via the helper's cached metadata (`<helper> -Sai`), parsed to Unix timestamps and fed into the hold evaluator; AUR packages bucket as Ready/Held based on actual dates instead of always Unknown; zero new dependencies, zero new network surface from nog itself
- [x] **Phase 5b — documentation polish (docs)** — full man page rewrite (COMMANDS, TIER SYSTEM, DESCRIPTION, FILES now accurate through v0.12.0 behavior and mention AUR integration); clap help-text refresh (top-level `long_about` + per-subcommand short + long descriptions)

---

## Changelog

### v1.0.6 — July 15, 2026
**Hotfix — split `lib32`/base pairs could abort the whole transaction**

A `lib32-<X>` multilib package hard-depends on its base `<X>` at an exact version (`lib32-nvidia-utils` → `nvidia-utils=<ver>`). Their hold windows are dated independently from each package's first-sighting date, so they can cross their thresholds on different days and land in **different buckets** — one Ready, one Held. Releasing only half the pair leaves pacman unable to satisfy the exact-version dependency, and it aborts the **entire** `nog update` — taking every other Ready package down with it. Reported in [#1](https://github.com/jetomev/nog/issues/1), hit live on the nvidia stack (`lib32-nvidia-utils` Ready while `nvidia-utils` was Held).

Same *family* as the tier-coupling (v1.0.3) and pkgbase-coupling (v1.0.4) fixes, but a distinct trigger: coupling existed for **tier bucketing**, not for **hold release** — two version-locked packages in the same tier could still be released on different days.

**Fix:**

- 🔗 **Hold-release coupling.** New `holds::lib32_coupling_demotions()` (pure, unit-tested) takes the Ready and Held name sets and returns the Ready packages to demote so each split `lib32`/base pair moves as a unit. Runs as a post-bucketing pass in `nog update`, before the ignore list is built, so a demoted package is genuinely withheld. **Bidirectional** — fires whether the `lib32-` half or the base half is the one still held.
- 🏷 **Named hold reasons.** The Held listing now says *why* a coupled package waits — `[Tier 3 · coupled to nvidia-utils · 4 days]` — inheriting the partner's countdown so both rows clear together.

**Scope:** name-pattern coupling (`lib32-` ↔ base), the reported failure; a fuller version keyed on the real `depends`/`provides` graph is noted in [#1](https://github.com/jetomev/nog/issues/1) for later. Ready↔Held only — a partner in the **Unknown** bucket keeps its per-package prompt.

Unit tests 29 → 33; warnings unchanged.

### v1.0.5 — July 7, 2026
**Hotfix — hold windows dated from stale sync DBs**

Fixes the third — and most fundamental — bug in the hold system's short history: hold windows were being measured from the **wrong package's build date**. Surfaced 2026-07-06 when a routine `nog update` reported `lib32-brotli` as *"975 days past window"* — for a package built **the day before**. Post-mortem showed all 14 "Ready" packages that day were 1–4 days old and belonged in Held; among them `bluez` ("53 days past window", built 2 days earlier) and `qtkeychain-qt6` ("62 days past window", built *that same day*).

Root cause: a **split-brain between two databases.** `nog update` gets its candidate list from `checkupdates`, which syncs fresh DBs into a private dbpath as an unprivileged user. But it read build dates from `/var/lib/pacman/sync` — which only refreshes when root syncs, i.e. during the yay/pacman handoff *after* the hold report. So for any update published since the last run (by definition, every update seen for the first time), the system DB still held the *predecessor* version, and the hold was clocked from the predecessor's builddate. Slow-moving packages (predecessor older than the window) sailed through with zero hold; fast-moving packages landed in Held with a wrong clock that silently self-corrected on later runs — which is why the bug stayed invisible from v1.0.0 until a 2.7-year-stale predecessor made the number absurd. **The Tier 1 implication was the serious one:** a new kernel arriving after a >30-day gap since the previous kernel's build would have skipped its 30-day window entirely.

**Fixes:**

- 📸 **Candidate-fresh snapshot.** `sync_db::load_fresh_packages()` walks the DBs `checkupdates` just synced (`$CHECKUPDATES_DB`, default `${TMPDIR:-/tmp}/checkup-db-<uid>/sync/`) — the exact snapshot that produced the candidate list. `nog update` prefers it and falls back to `/var/lib/pacman/sync` with a visible warning only if the snapshot is missing.
- 🛡 **Candidate-version guard.** `sync_db` now parses `%VERSION%`, and the new `holds::evaluate_candidate()` refuses to date a hold from a DB entry whose version isn't the pending candidate's. Mismatches route to **Unknown** and the per-package y/N prompt — honest about what nog actually knows, instead of trusting a clock borrowed from a different build. AUR entries (helper-provided dates, no version) skip the guard, unchanged.

**What this changes for existing installs:**
- The first `nog update` after upgrading may show a **noticeably shorter Ready list** — brand-new updates that pre-1.0.5 would have skipped their hold now correctly enter **Held** with sane countdowns. This is the protection working for the first time on first-sighting updates.
- "Days remaining" figures on already-Held packages may shift by a few days — they're now measured from the candidate's true builddate.
- The Unknown-bucket copy now also mentions version-mismatched DB entries.

Verified live before release: the fixed binary and an independent recomputation from the fresh DBs agreed on all 22 pending updates (21 repo + 1 AUR), every one correctly Held; unit tests 22 → 29.

### v1.0.4 — May 25, 2026
**Hotfix — split-PKGBUILD pkgbase coupling**

Fixes a regression-class bug in the same architectural class as v1.0.3, surfaced 2026-05-25 when `nog update` produced an unresolvable transaction. `pipewire` and `pipewire-pulse` were Tier 2 with 2 days remaining; the rest of the pipewire family (`libpipewire`, `pipewire-audio`, `pipewire-alsa`, `pipewire-jack`, `gst-plugin-pipewire`, `alsa-card-profiles`, `lib32-pipewire`, `lib32-libpipewire`) defaulted to Tier 3 Ready. pacman aborted:

```
:: installing libpipewire (1:1.6.5-2) breaks dependency 'libpipewire=1:1.6.5-1' required by pipewire
:: installing libpipewire (1:1.6.5-2) breaks dependency 'libpipewire=1:1.6.5-1' required by pipewire-pulse
```

Root cause: Arch's split-PKGBUILD convention ships multiple subpackages from one source (`pkgbase = pipewire` here) and enforces `=` version dependencies between them. Tier-mismatched holds across siblings violate that lockstep. v1.0.3 fixed the special case (`<X>-headers`); v1.0.4 generalizes.

**Fixes:**

- 🔗 **Layer A — pkgbase sibling coupling.** `sync_db.rs::parse_desc` now extracts the `%BASE%` field from every package in the sync DBs and exposes a `load_packages()` API returning rich metadata. A new `PkgbaseIndex` (constructed in `TierManager::with_pkgbase_index`) maps each package to its pkgbase and each pkgbase to its sibling list. `TierManager::classify()` consults the index: when classifying a package P with pkgbase B, the result is the highest tier present among siblings of B. Auto-handles pipewire, plasma, qt, kde-applications, gnome family — anywhere Arch ships coordinated subpackages with versioned deps.
- 🔀 **Layer B — `lib32-<X>` auto-coupling.** Multilib packages have their own PKGBUILD (different pkgbase) but Arch enforces version-pinned lockstep with the main package. The rule strips `lib32-` and inherits the base's tier if Tier 1 or Tier 2. Composes with Layer A: `lib32-libpipewire`'s pkgbase is `lib32-pipewire`, which classifies Tier 2 via the lib32- rule stripping to `pipewire` — so `lib32-libpipewire` correctly inherits Tier 2 transitively.
- 🔓 **Layer D — `nog unlock --promote` for any tier.** v1.0.3 restricted unlock to Tier 1 with the message "no unlock needed (only Tier 1 is ever held by policy)." That assumption was wrong — Tier 2 packages are held within their 15-day window too, and during a tier-mismatched lockstep failure the user needs to release Tier 2 holds to break the deadlock. The rule is now: any package can be force-upgraded via `--promote`, regardless of tier. The informational (no `--promote`) mode now shows tier-specific copy explaining the relevant hold window.

**What this changes for existing installs:**
- After upgrading to v1.0.4, **many more packages will silently re-classify** to Tier 1 or Tier 2 via pkgbase coupling. Examples:
  - pipewire family (`libpipewire`, `pipewire-audio`, etc.) → Tier 2 (inheriting from `pipewire`)
  - lib32-mesa, lib32-vulkan-icd-loader, etc. → Tier 1 (inheriting from `mesa` via lib32 prefix)
  - plasma-meta siblings → Tier 2 (inheriting from `plasma-desktop`)
  - qt5/qt6 sub-libraries → Tier 2 if any qt package is Tier 2
- On the next `nog update`, you'll see **more packages in the Held bucket** than v1.0.3. This is the fix in action — those siblings should never have been able to flow ahead of their base. Same pattern of silent re-tiering as v1.0.3's `*-headers` rule, just broader.
- `nog search` annotations now reflect pkgbase coupling too — `nog search libpipewire` shows yellow `[Tier 2 — 15d hold]` where v1.0.3 showed green Tier 3.

**Performance note:** `load_tiers()` now walks the sync DB (~18k packages on a typical Arch install) once per nog invocation to build the pkgbase index. The walk is OnceLock-cached so repeated classify calls within the same process don't re-walk. Adds a one-time cost (hundreds of ms) to commands that previously didn't touch the DB (`nog install`, `nog search`, `nog pin`, `nog unlock`). Accepted for the correctness gain.

**Tests:** 14 → 22 (8 new in `tiers::tests`):
- `lib32_inherits_tier1_when_base_is_tier1` (e.g., `lib32-mesa` → Tier 1)
- `lib32_inherits_tier2_when_base_is_tier2`
- `lib32_of_tier3_stays_tier3`
- `lib32_of_headers_inherits_via_inner_pattern`
- `pkgbase_sibling_inherits_tier2_from_base` (the pipewire family)
- `pkgbase_sibling_with_no_tier_pinned_member_stays_tier3`
- `empty_pkgbase_index_falls_through_to_tier3` (back-compat with v1.0.3 tier-pin-only behavior)
- `lib32_of_pkgbase_sibling_resolves_via_own_multilib_pkgbase` (composed Layer A + B)

**TEST-MATRIX:** new section 17 with 16 regression-guard checks across 17a (Layer A pkgbase), 17b (Layer B lib32), 17c (live regression — pipewire family upgrades together), 17d (Layer D Tier 2 unlock), 17e (no false positives on coherent systems).

No new dependencies. Same dynamic-libzstd linking contract as v1.0.1/v1.0.2/v1.0.3.

### v1.0.3 — May 13, 2026
**Hotfix — kernel / headers / DKMS coupling**

Fixes a regression-class bug where `nog update` could leave a system unbootable. On 2026-05-13 a user's machine ran `nog update`: the Tier 1 30-day hold on `linux-zen` and `linux-lts` kept the kernel binaries pinned, but `linux-zen-headers`, `linux-lts-headers`, and `nvidia-open-dkms` (all Tier 3 defaults) flowed through. The next DKMS rebuild emitted:

```
ERROR: Missing 6.18.29-1-lts kernel modules tree for module nvidia/595.71.05.
ERROR: Missing 7.0.5-zen1-1-zen kernel modules tree for module nvidia/595.71.05.
```

After reboot the running kernel was the old one, no `nvidia.ko` existed for it either, the GPU was unbound, and the user fell back to a single washed-out monitor on simpledrm framebuffer.

The root cause was architectural: kernel + headers + DKMS modules form a triplet that must move together, but `<X>-headers` packages were defaulting to Tier 3 even when their kernel was Tier 1.

**Fixes:**
- 🔗 **Auto-coupling — `<X>-headers` inherits its kernel's Tier 1.** `TierManager::classify()` now treats any package matching the `<name>-headers` pattern as Tier 1 when `<name>` is Tier 1. Same PKGBUILD produces both, so their build dates match and their holds expire together — coupling guarantees they bucket together at plan time too. Hardcoded, not configurable; the Arch convention is universal and the bug is severe.
- 📦 **New optional `[groups]` table in `tier-pins.toml`.** Escape hatch for non-standard kernel names (`linux-cachyos-cacule-headers`) and for bundling extras (e.g. `linux + nvidia-utils`). Members inherit the highest tier present among any other group member. See the commented example in the default `tier-pins.toml`.
- ⚠ **Plan-time desync detector.** At `nog update`, the installed versions of each Tier 1 kernel and its matching headers are compared via `pacman -Q`. Any mismatch prints a red ⚠ block before the Ready/Held/Unknown buckets, naming each desynced pair and pointing at the recovery flag below.
- 🔧 **New `nog update --realign` flag — forward-path recovery.** When desync is detected and the held kernel's pending upgrade version matches the installed headers version, `--realign` pulls that kernel out of the Held bucket and into Ready with the annotation `[Tier 1 · realigned to match installed headers]`. The subsequent transaction upgrades the kernel to match the headers in one coherent step. For the pathological case where no held kernel matches, the flag prints a clear notice and falls back to the standard plan.

**What this changes for existing installs:**
- After upgrading to v1.0.3, **`linux-headers`, `linux-zen-headers`, `linux-lts-headers`, and `linux-hardened-headers` move silently from Tier 3 to Tier 1**. On the next `nog update`, they will appear under **Held** with 30-day windows where v1.0.2 would have shown them as Ready with 7-day windows. This is the fix in action — those headers should never have been able to flow ahead of their kernel.
- The new `[groups]` table is optional; existing `tier-pins.toml` files without it continue to work unchanged.
- DKMS modules (e.g. `nvidia-open-dkms`) are **not** coupled explicitly — they're downstream victims that succeed automatically once kernel ↔ headers are coherent.

**Tests:** 6 → 14. Eight new unit tests in `tiers::tests` cover direct lookup (regression guard), `*-headers` auto-coupling for Tier 1, non-Tier 1 fall-through, group inheritance (both Tier 1 and Tier 2 / 3 cases), empty groups, and the `tier1_packages()` accessor used by the desync detector.

**TEST-MATRIX:** new section 16 with 16 regression-guard checks across 16a (auto-coupling, dev-build-safe), 16b ([groups]), 16c (desync warning), 16d (--realign recovery). Section 16a runs cleanly against any dev build with no system state changes.

No new dependencies. Same dynamic-libzstd linking contract as v1.0.1/v1.0.2.

### v1.0.2 — April 19, 2026
**Dogfood-surfaced polish batch**

Five small fixes and two matrix refinements, all caught during the end-to-end dogfood of the AUR-installed v1.0.1 binary. See [`v1.0 Test Results`](testing/20260419 - Test Results for nog v1-0-0.md) for the full run — every finding is documented there with observed behavior, severity, and fix rationale.

**Fixes:**
- 🛑 **F5 — graceful exit on missing tier-pins.** `load_tiers()` no longer panics with a Rust-native backtrace hint when `/etc/nog/tier-pins.toml` is unreadable. Clean `eprintln!` + `std::process::exit(1)` with the attempted path in the error message for diagnostic clarity.
- 🗂 **F4 — single-warning config load.** `NogConfig::load_default()` now cached via `OnceLock` — no more duplicate "no nog.conf found" warnings on misconfigured systems, and repeat callers read from the cache instead of re-hitting the filesystem.
- 🔒 **F2 — release binaries no longer embed the maintainer's build path.** The `CARGO_MANIFEST_DIR` dev-fallback branch is gated behind `#[cfg(debug_assertions)]`. Release binaries pass `strings` checks cleanly; dev clones still work as before via `cargo run`. Resolves the `makepkg` `$srcdir` warning.
- 🎨 **F1 — `nog search` tier annotations are now config-aware and consistent.** Tier 1 shows `30d hold` by default (was the misleading `manual sign-off`), flipping to `manual sign-off` only when `tier1 manual_signoff = true`. Tier 3 shows `7d hold` (was `fast-track`). All day counts read from `nog.conf`'s `[holds]` section.
- 📝 **F3 — error messages no longer duplicate "exit status".** Every `eprintln!("... exited with status {}", status)` now uses `status.code().unwrap_or(-1)` so output reads `exited with status 1` instead of `exited with status exit status: 1`.

**Matrix refinements:**
- 📋 **M1** — [`Test Matrix`](testing/20260513 - Test Matrix for nog v1-0-3.md) check 15.3 updated: `.pacsave`/`.pacnew` siblings are expected after any uninstall/reinstall cycle (the PKGBUILD's `backup=` directive intentionally preserves user-modified configs)
- 📋 **M2** — [`Test Matrix`](testing/20260513 - Test Matrix for nog v1-0-3.md) check 3.5 no longer keys the pass criterion on a specific exit code for nonexistent packages — helpers have inconsistent behavior here (yay returns 0 with "nothing to do"; paru may return non-zero)

**No behavior changes** beyond the error-path polish and the search label text. 6/6 hold tests still green. Same zstd-via-pkg-config dynamic-linking contract as v1.0.1.

### v1.0.1 — April 19, 2026
**Hotfix — AUR build failure on fresh environments**
- 🔨 `Cargo.toml`: switch `zstd = "0.13"` to `zstd = { version = "0.13", features = ["pkg-config"] }`. The previous config relied on `zstd-sys`'s bundled static build, which failed to link under Arch's makepkg environment (LLD + `-Wl,--as-needed` + `-nodefaultlibs`) because `zstd-sys` didn't emit the static-library link directive in that toolchain config
- 📚 Now uses system `libzstd` via dynamic linking — zero extra runtime dep (pacman already depends on libzstd, so it's always present on Arch)
- 📄 Man page header + README badge + Cargo.toml + `nog.conf` all bumped to 1.0.1
- ℹ No behavior changes; 6/6 hold tests still green. Caught by the v1.0 dogfood pass — exactly what a dogfood is for.

### v1.0.0 — April 19, 2026
**Initial stable release.**

nog is now a complete tier-aware wrapper for pacman and the common AUR helpers, built and polished across six deliberate phases documented in the entries below. This release declares the core contract stable:

**What nog does**
- Classifies every package into Tier 1 (kernel / bootloader / glibc / systemd / mesa — 30-day hold), Tier 2 (DE and key applications — 15-day hold), or Tier 3 (everything else — 7-day hold)
- Computes a full tier-aware upgrade plan before any transaction runs, grouping pending updates into **Ready**, **Held**, and **Unknown** buckets with Catppuccin Mocha tier colors
- Resolves build dates from every enabled pacman sync database (gzip + zstd), then falls back to the configured AUR helper's cached metadata (`<helper> -Sai`) for AUR-only packages — so AUR upgrades get real hold evaluation, not always-Unknown
- Hands off the final transaction to pacman or the helper with `--ignore=<held + skipped>` — pacman-native enforcement, no shadowing
- Escalates to root only via `sudo pacman` for transactions and `sudo tee` for writing `/etc/nog/tier-pins.toml`. Run `nog` as your user — never with `sudo`. The one-rule privilege model is documented exhaustively in the [Privilege model](#privilege-model--what-nog-touches-and-when) section.

**What nog doesn't do**
- Does not shadow, patch, or replace pacman — every transaction goes through pacman's signature verification
- Does not modify any system file outside `/etc/nog/tier-pins.toml`
- Does not make direct network calls — the helper owns all AUR network I/O
- Does not install, upgrade, or remove anything without pacman's own confirmation prompts
- Does not gate explicit user commands — `nog install linux-lts` always proceeds; tier protection lives in the passive `update` path

**Ecosystem**
nog is the native package manager for [KognogOS](https://github.com/jetomev/KognogOS), with a TUI companion ([nogforge](https://github.com/jetomev/nogforge)) and bootloader/terminal utilities ([grubforge](https://github.com/jetomev/grubforge), [alacrittyforge](https://github.com/jetomev/alacrittyforge)).

**Known limitations carried into v1.0**
- AUR build-date resolution depends on the helper's cached metadata being fresh. If the cache is stale, hold windows are evaluated against the cached date rather than live upstream data. Running `<helper> -Sy` (or `yay -Syy`) refreshes it.
- Tier pinning of AUR packages works, but AUR packages without a `Last Modified` field still fall into the Unknown bucket and trigger the y/N prompt.

**Thanks**
Development happened in deliberate phases (see below). Every phase closed with a tagged pre-release and a working dev build; the v1.0.0 tag is the moment the release kit (AUR submission + dogfood) begins.

### v0.12.0 — April 18, 2026
**Phase 5b (docs) — man page and help-text accuracy pass**
- 📜 Full man page rewrite: **DESCRIPTION** updated (30/15/7 day windows, AUR helper mention, expert-mode pointer); **COMMANDS** updated for every subcommand's real v0.12.0 behavior (no more stale "Tier 1 blocked" on install, accurate `nog update` bucketing description, `nog unlock` new semantics); **TIER SYSTEM** rewritten with auto-release default + expert mode; **FILES** now lists sync DBs and pacman.conf as read paths and notes `sudo tee` for tier-pins writes
- 🏷 `man nog` header bumped to `v0.12.0`
- 💬 Clap help text refresh — top-level `long_about` now summarizes the tier system and no-sudo rule in a few sentences; every subcommand (`install`, `remove`, `update`, `search`, `pin`, `unlock`) has a short description for the command list plus a longer one shown in `<cmd> --help`
- 🗂 Roadmap split Phase 5's polish work: screenshots + v1.0.0 CHANGELOG consolidation moved into the **v1.0 dogfood + release kit** step (more honest framing — they belong at release time, not pre-release)
- ℹ No behavior changes; no test regressions (6/6 still green); warnings unchanged at 7

### v0.11.0 — April 18, 2026
**Phase 5a — AUR build-date resolution (the last Unknown falls)**
- 📅 AUR pending upgrades now get real Unix-timestamp build dates by parsing the `Last Modified` field from the helper's cached metadata (`<helper> -Sai`) — no direct AUR RPC calls from nog
- 🧮 The hold evaluator sees a unified build-date map (sync-DB ∪ AUR) and buckets AUR packages as **Ready** or **Held** based on their actual dates, with countdown/past-window reasons identical to official repo packages
- 🧩 New `aur::build_dates_for(helper, packages)` — batched `-Sai` subprocess call, robust colon-split parser that tolerates variable column widths across yay/paru, Unix-timestamp conversion via `date -d`
- 🛟 **Soft-fail discipline preserved** — if the helper is unreachable, the `Last Modified` line is missing, or `date` can't parse the string, those packages fall back to the Unknown bucket and hit the existing y/N prompt. No hard errors, no crashes, no change to current user-facing error paths
- 🔒 **Zero new dependencies, zero new network surface from nog itself** — threat model identical to v0.10.0: nog spawns subprocesses, the helper owns all AUR network I/O
- 🗣 Unknown-bucket message updated — "no resolvable build date" is more accurate than "no build date in any sync DB" now that lookup has multiple paths
- ⚠ Only truly orphan packages (locally-built, disabled-repo, AUR query failure) reach the prompt now — most previous "Unknown" cases resolve automatically

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

nog is stable as of v1.0.0 (April 2026), with the v1.0.3 hotfix locking in kernel/headers/DKMS coupling. The release cadence follows a phased discipline documented in [`testing/RELEASE-CHECKLIST.md`](testing/RELEASE-CHECKLIST.md); every release ships through GitHub + AUR with a fresh-install verification on the maintainer's machine.

Ideas, bug reports, regression scenarios, and pull requests are welcome — open an issue or PR on GitHub. If you hit an issue that the [Troubleshooting](#troubleshooting) section doesn't cover, paste the output of `nog --version`, `pacman -Qi nog`, and (if relevant) the failing `nog update` excerpt.

If this project resonates with you, consider starring the repository. It helps others find it and motivates continued development.