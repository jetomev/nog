# 📦 nog

> A tier-aware package manager for Arch Linux — pacman with a safety net, written in Rust.

![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)
![Platform: Linux](https://img.shields.io/badge/Platform-Linux-lightgrey.svg)
![Base: Arch Linux](https://img.shields.io/badge/Base-Arch%20Linux-1793d1.svg)
![Language: Rust](https://img.shields.io/badge/Language-Rust-dea584.svg)
![Status: Stable](https://img.shields.io/badge/Status-Stable-brightgreen.svg)
![Version: 1.2.0](https://img.shields.io/badge/Version-1.2.0-purple.svg)
[![AUR](https://img.shields.io/aur/version/nog?color=1793d1&cacheSeconds=1801)](https://aur.archlinux.org/packages/nog)

> 🛡 **Security:** every release is GPG-signed and every commit GitHub-Verified. Read **[Where We Stand](https://github.com/jetomev/KognogOS/blob/main/docs/where-we-stand.md)** — our response to the 2026 AUR supply-chain attacks, what is current during the AUR freeze, and how to verify us instead of trusting us.

> ⚠️ **AUR freeze notice (Aug 2026):** the AUR is not accepting package pushes during the [supply-chain-attack lockdown](https://github.com/jetomev/KognogOS/blob/main/docs/operation-ironhold.md), so the AUR package lags at the badge's version until pushes reopen. **v1.2.0 is available now from source** (see [Installation](#installation)); the AUR update is staged and ships the day the freeze lifts.

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
- 🗒 **CSV run logs** — every `nog update` run is appended to a per-day CSV log (`YYYYMMDD nog-update.csv`) mirroring the report tables plus the run's outcome; 3-month retention, pruned automatically
- 🧩 **AUR helper integration** — auto-detects `yay` or `paru`; AUR pending upgrades are classified, date-evaluated (via the helper's cached metadata), and bucketed alongside official repo packages; transactions are handed off to the helper for combined `-Syu`
- ❓ **Interactive Unknown handling** — packages with no resolvable build date (locally-built, disabled-repo, or AUR query failure) are prompted case-by-case
- 🛡 **Foreign fence (v1.0.9)** — the upgrade handoff can only touch AUR/local packages nog explicitly cleared **this run**; a failed or empty AUR query can never silently release a hold. Born from the August 2026 AUR supply-chain attacks, when exactly that bypass happened live.
- 📦 **Flatpak, under the same rules (v1.1.0)** — `nog update` reports and applies Flatpak app updates alongside pacman and AUR, aged through the same tier hold windows (clock = the pending remote commit's publish date). Every row shows its source in the Note column, so you always know where an update comes from. Optional backend: no flatpak binary, no problem — the source is simply dormant. Toggle with `nog activate|deactivate flatpak`.
- 🐧 **Snap, for the tail (v1.2.0)** — same treatment as Flatpak: snaps appear in the update tables with their own `· snap` tag, aged by the publish date of the pending revision in their tracked channel. snapd is AUR-only on Arch, so nog never demands it — absent snapd is a dormant source, never an error. Toggle with `nog activate|deactivate snap`.
- 🔌 **Source kill switches (v1.0.9)** — `nog deactivate aur` / `nog deactivate chaotic-aur` sever a supply chain in one command during an incident; `nog activate <source>` restores it (byte-exact for pacman.conf, timestamped backups first). State persists in `/etc/nog/sources.toml`.
- 🧑 **No-sudo rule** — run `nog` as your user; it escalates to root only via `sudo pacman`, `sudo tee` for its own config files, and `sudo cp` for pacman.conf backups. See [Privilege model](#privilege-model--what-nog-touches-and-when) below.
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

# Source kill switches (incident response): sever a supply chain in one command
nog deactivate aur           # every AUR path refuses until reactivated
nog deactivate chaotic-aur   # repo commented out of pacman.conf (backup first)
nog activate aur             # restore — configured helper resumes
nog activate chaotic-aur     # restore — byte-exact, then DB refresh

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
9. Logs the run to `[paths] run_logs` (default `~/.local/share/nog/logs/YYYYMMDD nog-update.csv`) — one CSV row per package, mirroring the report tables, tagged with the run's outcome (`installed` / `cancelled` / `all held` / `up to date` / `handoff failed`) — then prunes logs older than 90 days. Logging soft-fails with a warning; it never blocks an update.

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

Captured from a live run (v1.0.8), Held section trimmed for brevity:

```
nog - Update!
=============
Date: 07/29/2026
Time: 08:52 PM
User: jetomev

nog: Checking for pending updates ...

nog: 75 official repository update(s) reported by pacman.
nog: 1 AUR update(s) reported by yay.

READY TO INSTALL:
-----------------

Package (3)     Old Version   New Version   Tier  Note

libraqm         0.10.5-1      0.11.0-1      3     hold just expired
plasma-desktop  6.7.2-1       6.7.3-1       2     hold just expired
python-certifi  2026.06.17-1  2026.07.22-1  3     1 day past window

ON HOLD FROM INSTALL:
---------------------

Package (73)            Old Version                  New Version                  Tier  Note

archlinux-keyring       20260707.1-1                 20260727-1                   3     4 days remaining
glibc                   2.43+r37+gfdf10644d6ee-1     2.44+r5+g7cba77790f32-1      1     28 days remaining
libnm                   1.56.1-2                     1.58.0-1                     2     5 days remaining
linux-zen               7.0.5.zen1-1                 7.1.5.zen1-2                 1     28 days remaining
linux-zen-headers       7.0.5.zen1-1                 7.1.5.zen1-2                 1     28 days remaining
lib32-libnm             1.56.1-1                     1.58.0-1                     3     coupled to libnm · 5 days
  ⋮                     (67 more)

UNKNOWN:
--------

(none)

nog: Proceed with installation? [Y/n] y

nog: Handing off to yay ...
:: Starting full system upgrade...
   (the yay/pacman transaction runs here)

nog: Update finished!
nog: run logged to /home/jetomev/.local/share/nog/logs/20260729 nog-update.csv

Thank you for using nog!
```

(The `Tier` digit is tier-colored — Tier 1 red, Tier 2 yellow, Tier 3 green — using the Catppuccin Mocha palette. Note the `coupled to libnm` row: v1.0.6's lib32/base coupling holding a version-locked pair together, and the closing `run logged to` line: v1.0.8's CSV run log.)

---

## Configuration

nog reads two configuration files from `/etc/nog/`.

### `nog.conf`

General nog settings — version, logging, paths, and **the authoritative hold durations** for each tier.

```toml
[general]
version = "1.0.8"
log_level = "info"

[paths]
tier_pins = "/etc/nog/tier-pins.toml"
pacman_conf = "/etc/pacman.conf"
log_file = "/var/log/nog.log"
# v1.0.8: per-run CSV update logs ("YYYYMMDD nog-update.csv", 90-day
# retention). nog runs unprivileged, so this lives in user space; a leading
# ~/ expands against $HOME. Key is optional — this is also the default.
run_logs = "~/.local/share/nog/logs"

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

### `sources.toml` (v1.0.9)

`/etc/nog/sources.toml` holds the source kill-switch state — managed by `nog activate` / `nog deactivate`, never hand-edited (though nothing breaks if you do):

```toml
# Managed by `nog activate <source>` / `nog deactivate <source>`.
# A missing file or missing key means the source is ACTIVE.
[sources]
aur = true
"chaotic-aur" = true
flatpak = true
snap = true
```

A missing file means everything is active (installs older than v1.0.9 are unaffected). An **unreadable** file fails **closed**: every source is treated as deactivated, with a loud warning — a corrupted kill switch must never silently re-open a supply chain. Running any `nog activate`/`deactivate` rewrites the file cleanly.

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

Three files, each with a single well-defined writer:

- `/etc/nog/tier-pins.toml` — written via `sudo tee` during `nog pin`.
- `/etc/nog/sources.toml` — written via `sudo tee` during `nog activate` / `nog deactivate` (v1.0.9 kill-switch state).
- `/etc/pacman.conf` — written **only** by `nog activate/deactivate chaotic-aur` (v1.0.9): the `[chaotic-aur]` section is commented in/out with a `#nog# ` marker, always preceded by a timestamped `sudo cp` backup (`pacman.conf.nog-bak-<stamp>`), and the restore is byte-exact. No other command touches it.

No other persistent file is created or modified by nog.

### What nog does NOT touch

The entire rest of your system is out of scope:

- `/etc/pacman.conf` outside the `[chaotic-aur]` section toggle above — never modified; every other byte of the file survives untouched (unit-tested)
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

nog runs as your user. It escalates for exactly three purposes: `sudo pacman` for package transactions, `sudo tee` for its own two config files (`tier-pins.toml`, `sources.toml`), and — only during a `chaotic-aur` toggle — `sudo cp` for a pacman.conf backup followed by `sudo tee` for the marker-commented rewrite of that one section. It never modifies any other file on your system, never bypasses pacman's signature verification, and never runs as root itself. If a helper is configured, transactions are handed off to `yay` or `paru` as your user, and those helpers escalate themselves.

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

### `nog: warning — /etc/nog/sources.toml is unreadable`

The kill-switch state file failed to parse (usually a hand-edit). nog fails **closed** — every source is treated as deactivated until the file is rewritten — so `nog update` will skip the AUR and warn. Fix: run `nog activate aur` (and/or `nog activate chaotic-aur`); each command rewrites the file in its canonical form.

## Roadmap

### Future
- [ ] **Zero-day lane for `archlinux-keyring`** (greenlit 2026-07-30, KognogOS audit finding) — a held keyring is itself the breakage: signatures fail on every later update until it lands. Add a special-case hold class (0-day / always-release) for keyring packages, likely a `[tier0]`-style list defaulting to `archlinux-keyring` + `chaotic-keyring`.
- [ ] **Depends-graph coupling (rule d)** — generalize the name-pattern couplings: an exact-version dependency (`pkg=ver`) hold-couples the depender to its target, subsuming the headers/lib32/pkgbase rules. Evidence: the 2026-07-30 KognogOS dep-chain audit found 736 exact-version pairs repo-wide, 2 unrescued (`dahdi-linux-git`, `wanpipe` → `linux=…`). Note: complete fix is hold-release coupling (v1.0.6 style), since same tier ≠ same release day.
- [ ] **First-run wizard** — on first `nog update`, ask the user whether Tier 1 should auto-update after 30 days (default, novice-friendly) or require manual `unlock --promote` per kernel/glibc/systemd upgrade (expert mode). Writes the chosen value to `tier-pins.toml [tier1] manual_signoff`.
- [ ] Chaotic-AUR binary package (submit once v1.0 is stable)
- [ ] `nog history` — log of all tier changes and package actions
- [ ] `nog status` — dashboard showing what's held, what's ready, what's overdue
- [ ] `nog rollback` — revert a recent update using pacman cache
- [ ] Hook support for notifying a GUI companion like `nogforge`

### The v2 arc — multi-source nog ([design](docs/v2-design.md) · [tracking issue #7](https://github.com/jetomev/nog/issues/7))
- [x] **C1 · v1.1.0 — Flatpak backend**
- [x] **C2 · v1.2.0 — Snap backend** *(below)*
- [ ] C3 · v1.3.0 — Install chain: pacman (+chaotic) → AUR → Flatpak → Snap, source always shown before installing
- [ ] C4 · v1.4.0 — Command surface + `--json`: info, search, remove, reinstall, tier, lock, status, history
- [ ] C5 · v1.5.0 — Maintenance & cleanup (orphans, caches, unused runtimes, old snap revisions)
- [ ] C6 — nogForge on forgekit: the visual handler
- [ ] C7 · v2.0.0 — crown release

### v1.2.0 — Released (C2: Snap backend)
- [x] `snap` joins `sources.toml` with `nog activate|deactivate snap`
- [x] Snap updates in the same tables, `· snap` tag, tier windows clocked by the tracked channel's publish date (`snap info`)
- [x] Same naming-based hold enforcement as flatpak, with its own `apply_list()` tests
- [x] `snap refresh` escalates through sudo (snapd requires root) with snap's own progress shown
- [x] Dormant when snapd is absent — never an error (snapd is AUR-only on Arch)

### v1.1.0 — Released (C1: Flatpak backend)
- [x] `flatpak` joins `sources.toml` with `nog activate|deactivate flatpak`
- [x] Flatpak updates folded into `nog update`: own source count, rows in the Ready/Held/Unknown tables, `· flatpak` tag in the Note column
- [x] Tier hold windows applied to flatpak refs via the pending commit's publish date (`flatpak remote-info`)
- [x] Holds enforced by naming (flatpak has no `--ignore`) — pure `apply_list()` with tests proving held/skipped refs can never be applied
- [x] Fail-closed on an unreachable remote; dormant when the flatpak binary is absent
- [x] [#8](https://github.com/jetomev/nog/issues/8) the flatpak handoff shows its own transaction (dogfood finding — "show the work", now a rule for every backend)

### v1.0.9 — Released ("Ironhold" security cycle)

Built during the July–August 2026 AUR supply-chain attacks (malicious orphan adoptions, `-bin` typosquats with sudo-time malware; the AUR froze all pushes on Aug 2), as Phase A of the cross-project [Operation Ironhold](https://github.com/jetomev/KognogOS/blob/main/docs/operation-ironhold.md). Every item was field-verified live on the reference machine the day it was built.

- [x] **The foreign fence ([#2](https://github.com/jetomev/nog/issues/2))** — fixes a fail-open hole caught by nog's own CSV run logs: on 2026-08-01, with the AUR mid-lockdown, `yay -Qua` returned empty, two held AUR packages vanished from the report, and the handoff's own resolution upgraded them anyway. Now every installed foreign package is ignored at handoff unless nog explicitly cleared it **this run** (Ready, or a user-approved Unknown) — "couldn't check" and "all quiet" are treated identically, and holds survive an AUR blackout. The bypass is reproduced as a unit test.
- [x] **AUR kill switch ([#3](https://github.com/jetomev/nog/issues/3))** — `nog deactivate aur` / `nog activate aur`: persisted in the new nog-owned `/etc/nog/sources.toml`, gated upstream of helper detection (helper-agnostic), turns off every AUR-aware path at once; the handoff runs pacman-only. `nog.conf` is never rewritten — the configured helper resumes exactly on activation. Unreadable state fails closed.
- [x] **chaotic-aur kill switch ([#4](https://github.com/jetomev/nog/issues/4))** — `nog deactivate chaotic-aur` / `activate chaotic-aur`: nog comments the `[chaotic-aur]` section in/out of `/etc/pacman.conf` itself (`#nog# ` marker; timestamped backup first; user comments inside the section survive; restore is byte-exact — proven by unit test *and* a live `diff` against the pre-toggle backup), then refreshes the sync DBs. The repo definition is the gate: nothing on the system can resolve from a deactivated chaotic-aur, and installed chaotic packages sit frozen.
- [x] **Held table sorted by days remaining ([#6](https://github.com/jetomev/nog/issues/6))** — soonest-to-release first, ties alphabetical; the table now reads as a release calendar and visualizes the tier gradient (1-day Tier 3 movers on top, 23-day Tier 1 kernels at the bottom). The CSV run log mirrors the same order.
- [x] **Test surface** — 42 → 54 (three fence tests incl. the Aug-1 replay, four `sources` state tests, five pacman.conf toggle tests incl. the byte-exact roundtrip).
- [x] ⚠️ *AUR note: released during the AUR push freeze — the AUR package updates to 1.0.9 the moment Arch reopens pushes; until then, install from source (`cargo build --release`).*


*Older roadmap entries live in [docs/ROADMAP.md](docs/ROADMAP.md).*

## Changelog

### v1.2.0 — August 10, 2026

**C2: nog speaks Snap.** The same shape as the Flatpak backend — snaps are detected, aged through the tier windows (clock = the publish date of the pending revision in the snap's *tracked channel*, read from `snap info`), tagged `· snap` in the tables, and refreshed only when nog has cleared them. Holds are enforced by naming, as with flatpak, and pinned by their own tests.

Two structural differences from flatpak, both deliberate: `snap refresh` requires root, so nog escalates through `sudo` for that step alone (staying unprivileged everywhere else); and because snapd is AUR-only on Arch, its absence is *normal* — the source sits dormant and silent, never an error. Snap is expected to serve the tail: most software is covered long before the chain reaches it, but when it's the only home for something, nog can now manage it like everything else.

Dogfooded live on 2026-08-10 with `hello` installed at an old revision — detection, channel-date resolution (1663 days past window, honestly reported), the source tag, and a real sudo-escalated refresh.

Also in this release: the README's Roadmap and Changelog now keep only the upcoming work and the two most recent releases, with full history in [docs/ROADMAP.md](docs/ROADMAP.md) and [docs/CHANGELOG.md](docs/CHANGELOG.md) — the file had grown to a thousand lines, which serves archaeologists better than newcomers.

### v1.1.0 — August 10, 2026

**C1 of the [v2 multi-source arc](docs/v2-design.md): nog speaks Flatpak.**

Flatpak becomes a first-class source rather than a parallel universe you update separately. `nog update` now queries flatpak alongside pacman and the AUR, reports its own per-source count, and folds every pending app into the same Ready / On-hold / Unknown tables — aged by the same tier windows, with the clock set by the pending remote commit's publish date (the build-date analogue). Rows carry a `· flatpak` marker so the source of every update is visible before you say yes.

Holds work differently under the hood because flatpak has no `--ignore`: nog enforces them **by naming**, passing exactly the refs it cleared this run. That rule lives in one pure function (`flatpak::apply_list`) with tests proving a held or skipped ref can never reach the transaction.

The backend is optional in both directions — a missing `flatpak` binary leaves the source dormant (never an error), and `nog deactivate flatpak` freezes it by choice. A failed remote query is reported and skipped, never assumed quiet (the fail-closed doctrine from v1.0.9).

Dogfooded live on 2026-08-10 across the full pipeline: detection, date resolution, tier bucketing, the source tag, the kill switch, and a real apply. One finding came out of it and shipped in the same release — [#8](https://github.com/jetomev/nog/issues/8): the flatpak handoff now shows flatpak's own transaction instead of a single silent line. *Show the work* is now a requirement for every future backend.

### v1.0.9 — August 5, 2026
**The "Ironhold" security cycle — holds that fail closed, and supply chains you can sever in one command**

Built live during the July–August 2026 AUR supply-chain attacks, as Phase A of [Operation Ironhold](https://github.com/jetomev/KognogOS/blob/main/docs/operation-ironhold.md). The trigger was nog's own CSV run log catching a real bypass on this machine: on Aug 1, with the AUR mid-lockdown, the AUR query silently returned empty, two packages that had been *held with 5 days remaining the night before* vanished from the report — and the handoff upgraded them anyway. (Both proved clean. The hole didn't.)

- 🛡 **The foreign fence ([#2](https://github.com/jetomev/nog/issues/2))** — the handoff's `--ignore` list now always includes **every** installed foreign package except the ones nog explicitly cleared this run (Ready, or a user-approved Unknown). "Couldn't check the AUR" and "no AUR updates" are treated identically — the fence stands either way, so a hold can never again evaporate because a query failed. Healthy runs behave exactly as before (ignoring an up-to-date package is a no-op). The Aug-1 bypass is now a unit test.
- 🔌 **`nog deactivate aur` / `nog activate aur` ([#3](https://github.com/jetomev/nog/issues/3))** — the AUR kill switch. One command turns off every AUR-aware path — update detection, install routing, handoff (pacman-only) — for incident response during an active attack. State persists in the new nog-owned `/etc/nog/sources.toml` (written via `sudo tee`, tier-pins style); your `nog.conf` helper setting is never touched and resumes exactly on activation. An unreadable state file fails **closed**.
- 🔌 **`nog deactivate chaotic-aur` / `activate chaotic-aur` ([#4](https://github.com/jetomev/nog/issues/4))** — the binary-repo kill switch. nog comments the `[chaotic-aur]` section in/out of `/etc/pacman.conf` itself: timestamped backup first, `#nog# ` marker so activation restores exactly and only what nog disabled (your comments inside the section survive), DB refresh after. With the section out, *nothing* on the system — pacman, helpers, libalpm GUIs — can resolve from the repo; installed chaotic packages sit frozen. Restore verified byte-exact live (`diff` against the pre-toggle backup: identical).
- 📅 **Held table sorted by days remaining ([#6](https://github.com/jetomev/nog/issues/6))** — soonest-to-release on top, Tier 1 heavyweights at the bottom: the hold list now reads as a release calendar, and — a happy accident — visualizes the tier gradient itself. The CSV log mirrors the order.
- ⚠️ **Released during the AUR push freeze** — the AUR package updates to 1.0.9 the moment Arch reopens pushes ([context](https://github.com/jetomev/KognogOS/blob/main/docs/operation-ironhold.md)); until then: `git clone https://github.com/jetomev/nog.git && cd nog && cargo build --release`.

Internals: new pure `sources` module (state parse/render + the pacman.conf section toggler) and `holds::foreign_fence()` + `pacman::foreign_package_names()`. Every feature was field-verified on the reference machine the day it was built — including one full 176-update run with the fence live and both kill-switch round trips. Unit tests 42 → 54; warnings unchanged at 7.


*The complete history lives in [docs/CHANGELOG.md](docs/CHANGELOG.md).*

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