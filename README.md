# 📦 nog

> A tier-aware package manager for Arch Linux — pacman with a safety net, written in Rust.

![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)
![Platform: Linux](https://img.shields.io/badge/Platform-Linux-lightgrey.svg)
![Base: Arch Linux](https://img.shields.io/badge/Base-Arch%20Linux-1793d1.svg)
![Language: Rust](https://img.shields.io/badge/Language-Rust-dea584.svg)
![Status: Stable](https://img.shields.io/badge/Status-Stable-brightgreen.svg)
![Version: 1.5.5](https://img.shields.io/badge/Version-1.5.5-purple.svg)
[![AUR](https://img.shields.io/aur/version/nog?color=1793d1&cacheSeconds=1801)](https://aur.archlinux.org/packages/nog)

> 🛡 **Security** — every release is GPG-signed and every commit is GitHub-Verified. **[Where We Stand](https://github.com/jetomev/KognogOS/blob/main/docs/where-we-stand.md)** covers our response to the 2026 AUR supply-chain attacks and how to check us yourself.

---

## Why nog?

Arch Linux is fast, current, and beautifully simple. But it treats every package the same. When an update exists, it installs. Your kernel updates on the same schedule as an icon theme — and one bad kernel update means your machine doesn't boot.

There's no safety net. One bad sync and you're in single-user mode at 2 AM.

**nog adds one idea: not every package deserves the same urgency.**

Every package belongs to a tier, and each tier waits a different length of time before updates land:

| Tier | What's in it | Waits |
|---|---|---|
| **1** | Kernel, bootloader, glibc, systemd, mesa | **30 days** |
| **2** | Desktop and key applications | **15 days** |
| **3** | Everything else | **7 days** |

The waiting is the point. Thousands of Arch users install an update before you do. If it's broken, they find out first and it gets fixed before it reaches your machine. Your everyday software stays current; the parts that can ruin your week don't move until they've been proven.

nog is a wrapper around pacman, not a replacement. Same commands, same flags, same mental model. It never patches pacman, never shadows it, and cannot bypass its signature checks.

---

## Features

**The tier system**
- Every package is Tier 1, Tier 2, or Tier 3, with 30 / 15 / 7-day holds
- **A hold always ends** *(v1.5.2)* — the wait counts from the first new version, so a stream of newer builds can't keep a package back forever. The version that installs must still be a few days old (the safety wait)
- Pin anything to any tier — `nog pin <pkg> --tier=<N>`
- Need a held package now? `nog unlock <pkg> --promote`
- Expert mode: require your explicit approval for *every* Tier 1 update

**Every source, one set of rules**
- **Official repos** through pacman
- **AUR** through yay or paru, detected automatically
- **Flatpak** *(v1.1.0)* — aged by the pending release's publish date
- **Snap** *(v1.2.0)* — aged by the publish date in the channel you track, and since v1.4.1 the hold is placed in snapd itself, so its own four-times-a-day auto-refresh cannot walk past it

  Flatpak and Snap are optional in both directions. If the program isn't installed, that source sits quietly dormant — never an error. Turn any of them on or off with `nog activate|deactivate <source>`.

**Clear reporting**
- A **summary table** first *(v1.5.4)* — one row per repository (`core`, `extra`, `multilib`, `chaotic-aur`…), then AUR, Flatpak and Snap, with what is ready now and what is on hold. A source that could not be checked says `could not check`, never `0`
- Updates grouped into **Ready** / **Held** / **Unknown**, with tier colours
- Every row has a **Source** column *(v1.5.4)*: the repository it comes from, or `AUR`, `Flatpak`, `Snap` — each non-official source in its own colour
- Held packages sorted by how soon they release, so the list reads as a calendar
- Packages with no usable date are never guessed at — nog asks you, one at a time
- Every run is logged to a dated CSV you can open in a spreadsheet, kept 90 days — one row per package with its source and what actually happened to it, and the reason whenever a step did not complete *(v1.4.3)*
- **Reboot advice** *(v1.4.0)* — when a kernel or driver update leaves the running system out of step with what is now installed, nog says so at the end of the run. Where it can check, it says `verified` and shows both versions; where it cannot, it names the package and says plainly that this is advice rather than a finding

**Maintenance**
- **`nog clean`** *(v1.5.5)* — clears old downloads from pacman's cache, keeping more of what hurts to lose: 3 versions of each Tier 1 package, 2 of Tier 2, 1 of Tier 3. The installed version is never removed, and packages you no longer have are cleared. It shows what it would remove, per tier, and asks first

**Security**
- **One manager per source** *(v1.3.0)* — pacman upgrades official packages; your AUR helper is handed only the AUR packages nog cleared, **by name**. Nothing unnamed can move, so a failed AUR lookup cannot release a hold by omission. Born from a real bypass we caught on our own machine, originally patched by the foreign fence *(v1.0.9)*, which now backs it up as a second layer.
- **Kill switches** *(v1.0.9)* — `nog deactivate aur` or `nog deactivate chaotic-aur` cuts off a supply chain in one command during an incident. `nog activate` puts it back exactly as it was.
- **Runs as you, not as root** — nog escalates only at the specific moments root is genuinely needed, and you see every prompt. See [Privilege model](#privilege-model).
- **Holds use pacman's own `--ignore`**, so there's no mechanism by which nog could quietly skip one.

---

## The Three-Tier System

Tier assignments live in `/etc/nog/tier-pins.toml` and can be changed any time with `nog pin`. Hold durations live in `/etc/nog/nog.conf`.

### Tier 1 — 30 days

The packages that can stop your machine from booting. Held for 30 days after the first new version is published upstream — a full month of everyone else testing it first. When the hold expires it installs normally.

**Default members:** `linux`, `linux-zen`, `linux-lts`, `linux-hardened`, `systemd`, `systemd-libs`, `glibc`, `grub`, `efibootmgr`, `mkinitcpio`, `pacman`, `mesa`

> **Expert mode.** Set `manual_signoff = true` under `[tier1]` in `tier-pins.toml` and Tier 1 stops auto-releasing entirely — every kernel, glibc and systemd update then needs an explicit `nog unlock <pkg> --promote`. Worth it only if you want to personally look at each one.

### Tier 2 — 15 days

Your desktop and the applications you'd notice breaking. Long enough for real problems to surface, short enough that you don't fall behind.

**Default members:** `plasma-meta`, `plasma-desktop`, `sddm`, `pipewire`, `pipewire-pulse`, `wireplumber`, `networkmanager`, `firefox`, `dolphin`, `konsole`, `kate`, `grubforge`, `alacritty`, `fish`, `alacrittyforge`

### Tier 3 — 7 days

Everything else, which is most of your system. A short buffer with no meaningful delay.

### How long a hold lasts, when new versions keep coming *(v1.5.2)*

A hold is counted from **the day the first new version appeared**, not from the newest one. Arch keeps only the newest build of each package, and busy packages get a new build every week or two. Until v1.5.2 each new build restarted the countdown, so a Tier 1 kernel could be held forever: on the development machine, `linux-zen` sat at 7.0.5 from May to the end of September while nine newer versions came and went ([#27](https://github.com/jetomev/nog/issues/27)).

Now the window runs from the first sighting, and a newer build doesn't restart it. When the window is over, nog installs the newest version — but only once that version is itself a few days old, so a build from yesterday can't walk straight in. That short **safety wait** is 7 days for Tier 1, 3 for Tier 2 and 1 for Tier 3, set in `nog.conf`.

The tables show it: `waiting since Jul 29 · 8 newer versions skipped`, and `newest build too new` while the safety wait runs. nog remembers the first sightings in a small file in your home folder (`~/.local/state/nog/holds.tsv`). Holds that were already running when you upgraded are dated from your run logs.

### Packages that must move together

Some packages are only safe to update as a set. The clearest case is a kernel and its `-headers` package.

They're built from the same recipe and must always match, because graphics drivers and similar modules are rebuilt against whichever headers are on disk and installed into a folder named after the kernel version. If the headers move ahead of the kernel, the rebuild has nowhere to put its output — and your GPU driver fails to load after the next reboot.

So nog **automatically ties `<kernel>-headers` to its kernel's tier**. If `linux-zen` is Tier 1, `linux-zen-headers` is too. They hold together and release together. This is always on and not configurable: the naming convention is universal and the failure is severe.

For kernels that don't follow that naming, group them explicitly in `/etc/nog/tier-pins.toml`:

```toml
[groups]
cachyos-bundle = [
    "linux-cachyos",
    "linux-cachyos-headers",
    "linux-cachyos-cacule-headers",
]
```

Every member of a group inherits the highest tier any member has. You can use the same mechanism to pull extra packages into a kernel's tier — for example `linux + nvidia-utils + nvidia-open-dkms` if you want maximum caution around graphics.

The driver modules themselves don't need grouping. Once the kernel and headers agree, their rebuilds succeed on their own.

---

## Requirements

- Arch Linux, or an Arch-based distribution
- `pacman` and `pacman-contrib`
- `yay` or `paru` — optional, adds AUR support. nog works fine without one; you just get official repos only.
- A Rust toolchain, only if building from source

---

## Installation

### From the AUR (recommended)

```bash
yay -S nog
```

[aur.archlinux.org/packages/nog](https://aur.archlinux.org/packages/nog)

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

### What gets installed

| File | Location |
|------|----------|
| `nog` binary | `/usr/bin/nog` |
| `nog.conf` | `/etc/nog/nog.conf` |
| `tier-pins.toml` | `/etc/nog/tier-pins.toml` |
| Man page | `/usr/share/man/man1/nog.1` |

---

## Usage

> Run `nog` as your normal user — never with `sudo`. It escalates only where root is genuinely needed, and you'll see the password prompt at that moment. See [Privilege model](#privilege-model).

```bash
# Install a package (respects tier rules, routes to your AUR helper if needed)
nog install <package>

# Install a package file you built yourself, e.g. with makepkg
nog install ./<package>-<version>-<arch>.pkg.tar.zst

# Update everything (tier holds applied across all sources)
nog update

# Search, with each result's tier shown
nog search <query>

# Move a package to a different tier
nog pin <package> --tier=<1|2|3>

# Install a held Tier 1 package right now
nog unlock <package> --promote

# Remove a package
nog remove <package>

# Clear old downloads from pacman's cache (asks first)
nog clean

# Kill switches — cut off a source during a security incident
nog deactivate aur           # every AUR path refuses until reactivated
nog deactivate chaotic-aur   # repo commented out of pacman.conf (backup taken first)
nog activate aur             # restore
nog activate chaotic-aur     # restore, byte for byte, then refresh

nog --version
nog --help
```

### Example: `nog clean`

A real report on the development machine (30 Sep 2026), with the question answered no:

```
nog: pacman's download cache — 6706 files, 18.4 GB.

WHAT nog clean WOULD REMOVE:
============================

What                               Versions      Size
-----------------------------------------------------
No longer installed                     346    1.9 GB
Tier 2 · older than the newest 2         10     45 MB
Tier 3 · older than the newest 1       1489    8.9 GB
Leftover download folders               123   size not readable
-----------------------------------------------------
Total                                  1845   10.8 GB

Kept: 1520 package versions (7.6 GB), including every installed version.

nog: Remove them? [y/N]
```

Every installed version stays, so a bad update can always be rolled back from the cache. The kernels keep three versions each — the rollback path a black-screen night needs.

### What `nog update` actually does

1. Asks `checkupdates` for pending official-repo updates. This syncs into its own private database, so your system's package database is left alone.
2. If an AUR helper is configured, adds pending AUR updates to the same list.
3. Reads build dates from **the fresh database `checkupdates` just synced** — not the older system copy. For AUR packages, it reads the helper's cached information instead.
4. Works out each package's tier and whether its hold has expired. The hold counts from when an update for your installed version was **first seen**, kept in `~/.local/state/nog/holds.tsv`, and the newest version must also be past its tier's safety wait *(v1.5.2)*.

   If the date it finds doesn't belong to the exact version about to be installed, nog refuses to use it and files the package under **Unknown** rather than guessing.
5. Sorts everything into three groups:
   - **Ready to install** — the hold has expired
   - **Held** — still inside its window, or a Tier 1 package awaiting your sign-off
   - **Unknown** — no trustworthy date (built locally, repo disabled, or the lookup failed)
6. Asks you about each **Unknown** package individually.
7. Runs the upgrade, telling pacman and your helper to skip everything held.
8. If everything is held, exits cleanly without running anything at all.
9. Writes the run to a dated CSV log — each package with its source and its own outcome — and prunes logs older than 90 days. If logging fails it warns you — it never blocks an update. Any reboot advice goes to a companion `nog-reboot.csv` beside it.

Everything is classified **before** anything is touched, so you always see the plan first.

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

A real report on the development machine (30 Sep 2026), with the Held section trimmed. That day everything was on hold, so nog stopped before handing anything off:

```
=============
nog v1.5.4
Update!
=============

Date: 09/30/2026
Time: 05:52 PM
User: jetomev

nog: Checking for pending updates ...
Checking installed programs for 1 library version(s) this update removes...

SUMMARY:
========

Source        Ready now   On hold   Total
-----------------------------------------
core                  0        13      13
extra                 0        38      38
multilib              0         6       6
chaotic-aur           0         2       2
AUR                   0         1       1
Flatpak               0         0       0
Snap                  0         0       0
-----------------------------------------
Total                 0        60      60

READY TO INSTALL:
=================

(none)

ON HOLD FROM INSTALL:
=====================

Package (60)         Source       Old Version               New Version               Tier  Note
---------------------------------------------------------------------------------------------------------------------------------------------------------------------------
alsa-card-profiles   extra        1:1.6.8-1                 1:1.6.9-1                 2     1 day remaining
linux-lts            core         6.18.29-1                 6.18.54-1                 1     1 day · newest build too new · waiting since Jul 29 · 11 newer versions skipped
mkinitcpio           core         41-4                      42.1-1                    1     3 days · newest build too new · waiting since Aug 13 · 2 newer versions skipped
fresh-editor-bin     AUR          0.5.1-1                   0.5.2-1                   3     4 days remaining
systemd              core         261.2-1                   262-1                     1     12 days remaining · waiting since Sep 13 · 1 newer version skipped
glibc                core         2.44+r24+g16be1518495f-1  2.44+r50+g1848099f063e-1  1     26 days remaining
  ⋮                  (54 more)

UNKNOWN:
========

(none)

nog: Nothing to install — every pending update is held.
nog: run logged to /home/jetomev/.local/share/nog/logs/20260930 nog-update.csv
```

The tier digit is colour-coded — red, yellow, green — and non-official sources get their own colour in the Source column. Details worth spotting: `fresh-editor-bin` is marked `AUR`; `linux-lts` finished its window but its newest build is too new for the safety wait; and `systemd` has been waiting since 13 September — the newer build that arrived since did not restart its countdown.

---

## Configuration

nog reads its configuration from `/etc/nog/`.

### `nog.conf`

General settings, and **the authoritative hold durations**.

```toml
[general]
version = "1.5.5"
log_level = "info"

[paths]
tier_pins = "/etc/nog/tier-pins.toml"
pacman_conf = "/etc/pacman.conf"
log_file = "/var/log/nog.log"
# Per-run CSV logs, kept 90 days. nog runs unprivileged, so these live in
# your home directory. A leading ~/ expands against $HOME.
run_logs = "~/.local/share/nog/logs"
# When each pending update was first seen, so a newer build can't restart
# its hold (v1.5.2). Optional; this is the default.
hold_record = "~/.local/state/nog/holds.tsv"

[holds]
tier1_days = 30
tier2_days = 15
tier3_days = 7
# The safety wait (v1.5.2): once a hold is over, the newest version must
# still be at least this many days old. Optional; these are the defaults.
tier1_safety_days = 7
tier2_safety_days = 3
tier3_safety_days = 1

[clean]
# How many versions of each package `nog clean` keeps (v1.5.5). The installed
# version always counts as one. Optional; these are the defaults.
tier1_keep = 3
tier2_keep = 2
tier3_keep = 1

[aur]
# Which AUR helper to use.
#   "auto" — prefer yay, fall back to paru, skip AUR if neither is installed
#   "yay"  — require yay
#   "paru" — require paru
#   "none" — turn off all AUR support
helper = "auto"
```

### `sources.toml`

Holds the on/off state for each source. Managed by `nog activate` and `nog deactivate` — you shouldn't need to edit it, though nothing breaks if you do.

```toml
[sources]
aur = true
"chaotic-aur" = true
flatpak = true
snap = true
```

A missing file means everything is active. An **unreadable** file fails **closed** — every source is treated as off, loudly — because a broken kill switch must never quietly re-open a supply chain. Running any `activate` or `deactivate` command rewrites the file cleanly.

### `tier-pins.toml`

Which packages are Tier 1 or Tier 2. Anything not listed falls to Tier 3 automatically.

```toml
[tier1]
# false (default): Tier 1 auto-updates once the 30-day hold expires.
# true (expert):   Tier 1 stays held until you run `nog unlock <pkg> --promote`.
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
# everything not listed above lands here automatically
```

`manual_signoff` only means something on `[tier1]`. Tiers 2 and 3 ignore it.

---

## Project Structure

```
nog/
|-- src/
|   |-- main.rs           # Entry point and command-line definitions
|   |-- commands/mod.rs   # Every subcommand's implementation
|   |-- tiers.rs          # Tier classification, including auto-coupling and [groups]
|   |-- holds.rs          # Hold evaluation and the foreign fence (pure functions)
|   |-- sightings.rs      # When each update was first seen, so holds always end  (v1.5.2)
|   |-- cache.rs          # nog clean: pacman's version order, per-tier cache retention  (v1.5.5)
|   |-- local_db.rs       # Reads pacman's local database: dependency graph (v1.3.1), who links what (v1.4.2)
|   |-- elf.rs            # Reads which libraries a program needs  (v1.4.2)
|   |-- reboot.rs         # Reboot advice: probes the running system  (v1.4.0)
|   |-- pacman.rs         # pacman wrapper
|   |-- aur.rs            # AUR helper detection and handoff
|   |-- flatpak.rs        # Flatpak source  (v1.1.0)
|   |-- snap.rs           # Snap source     (v1.2.0)
|   |-- sources.rs        # Kill-switch state and the pacman.conf toggle
|   |-- sync_db.rs        # Reads pacman's databases for build dates
|   |-- runlog.rs         # CSV run logging
|   |-- handoff.rs        # Runs each source's tool, keeping why it failed  (v1.4.3)
|   |-- config.rs         # Configuration loader
|-- config/               # Default nog.conf and tier-pins.toml
|-- testing/              # Test matrix, results, and release checklist for every version
|-- docs/                 # Full changelog, full roadmap, v2 design notes
|-- nog.1                 # Man page
|-- Cargo.toml / Cargo.lock
```

Around 10,700 lines of Rust, with 217 tests that run on every release.

Packaging lives in the AUR repository, not here. A second `PKGBUILD` in this tree diverged from it silently through two releases while both files reported the same version, so it was removed in v1.4.0 rather than kept in step by hand.

---

## Safety Philosophy

nog is built around one rule: **never surprise you with a kernel update.**

Three things enforce it:

1. **Classification** — every package gets a tier before anything happens
2. **Transparency** — you see what's held, for how long, and why, before any change
3. **Pacman does the enforcing** — holds use pacman's own `--ignore`, so there's no path by which nog could skip one

Commands you type directly — `install`, `remove`, `pin` — do exactly what you asked without argument. Tier protection applies to the passive path, `update`. Installing `linux-lts` is always allowed; what's governed is when the *next* kernel update arrives on its own.

nog doesn't replace pacman, patch it, or shadow its commands. Every install, removal and upgrade goes through pacman's signature verification and conflict resolution. nog cannot bypass them.

---

## Privilege model

**Run nog as your normal user. Never `sudo nog`.**

nog escalates only at the exact moments root is required, and you always see the prompt.

If you forget and type `sudo nog` while an AUR helper is configured, nog notices and stops with a clear error — because yay and paru both refuse to run as root, by design.

### Where nog escalates

Five places. That's the complete list.

| What | Command | When |
|---|---|---|
| Package transactions | `sudo pacman ...` | `nog update` always hands the official repositories to `sudo pacman -Syu` itself (v1.3.0), and `nog install` of a package file always uses `sudo pacman -U` (v1.5.0). `install` by name, `remove` and `unlock --promote` use it **only when no AUR helper is configured**; with a helper, nog calls the helper as you, and the helper runs its own `sudo pacman` internally. |
| Snap holds and updates | `sudo snap refresh ...` | Placing a tier hold (`--hold`, v1.4.1) and applying snap updates. snapd requires root for both; nothing else about snap does. |
| Its own config files | `sudo tee <file>` | Writing `tier-pins.toml` (during `nog pin`) and `sources.toml` (during `activate`/`deactivate`). The new contents are built in memory and piped to `tee` — nog itself never runs as root, only `tee` does. |
| Cache cleanup | `sudo rm -f --` | Only in `nog clean` (v1.5.5), only after you answer yes, and only on package files and day-old leftover `download-XXXXXX` folders directly inside pacman's cache directories. |
| pacman.conf backup | `sudo cp --preserve=all` | Only during `nog activate|deactivate chaotic-aur`, to take a timestamped backup before editing that one section. |

### What nog reads

All world-readable, so nog reads them as you: `/etc/nog/nog.conf`, `/etc/nog/tier-pins.toml`, `/etc/pacman.conf`, and pacman's sync databases.

### What nog writes

Three system files, each with one well-defined writer:

- `/etc/nog/tier-pins.toml` — during `nog pin`
- `/etc/nog/sources.toml` — during `nog activate` / `nog deactivate`
- `/etc/pacman.conf` — **only** by `activate|deactivate chaotic-aur`, which comments the `[chaotic-aur]` section in or out using a `#nog#` marker, after a timestamped backup. Restoring is byte-exact, and your own comments inside that section survive. No other command touches this file.

In your home folder, as you: the run logs (`~/.local/share/nog/logs/`) and the hold record (`~/.local/state/nog/holds.tsv`, v1.5.2), both written by `nog update`.

### What nog never touches

Everything else. Mirrorlists, pacman's installed-package state, its cache, its GPG keyring and signature checks, `/etc/sudoers`, PAM, and every system binary directory. Every byte of `pacman.conf` outside that one section survives untouched — there's a unit test for it.

### Working with your AUR helper

When a helper is configured, nog asks it for pending AUR updates and hands transactions to it, always **as your user**. The helper fetches and builds as you, then runs its own `sudo pacman` when it reaches that step — that prompt comes from the helper, not from nog.

nog never runs `sudo yay` or `sudo paru`. That's a deliberate refusal, for the same reason those tools refuse it themselves: building packages as root is unsafe.

---

## Troubleshooting

### `ERROR: Missing <KVER> kernel modules tree for module <name>/<version>`

This comes from a driver package like `nvidia-open-dkms` after an update. It means the driver is trying to build against a kernel version that isn't installed.

This is the kernel/headers mismatch described in [Packages that must move together](#packages-that-must-move-together). Before v1.0.3, nog held kernels but not their headers, so the two could drift apart.

**Fix:**

```sh
nog update --realign
```

This pulls held kernels into the upgrade when their pending version matches your installed headers, so both end up on the same version in one coherent step. The driver rebuild then succeeds.

**If `--realign` doesn't apply** — for instance your headers are already ahead of any pending kernel update:

```sh
# 1. Bring the kernels forward to match the headers
sudo pacman -S linux-zen linux-lts            # adjust to your kernels

# 2. Reinstall the driver to retrigger its build
sudo pacman -S nvidia-open-dkms                # or whichever one broke

# 3. Check it worked
dkms status
```

**To confirm coupling is active:**

```sh
nog search linux-zen-headers
# expect a red [Tier 1 — 30d hold] tag
```

If it shows green Tier 3, you're on an old nog — upgrade before your next update.

### The summary says `AUR  could not check`

Your AUR helper's update check failed this run, and nog is saying so instead of reporting `0`. The helper's own error is quoted in the warning just above the summary. Holds stay safe — an AUR package nog never heard about is never handed off, and the foreign fence keeps every foreign package where it is — but AUR updates go unseen until the check works again. Try the helper directly (`yay -Qua` or `paru -Qua`) to see the full error.

### A big batch of updates is suddenly Ready after upgrading to v1.5.2

Expected. Packages that had been held for a long time because newer builds kept restarting their countdown — usually the kernels, `mesa` and `mkinitcpio` — are released on the first run, each marked `waiting since <date> · N newer versions skipped`. A kernel can jump several versions at once. Let the update finish, including any driver rebuild, and **restart** when nog advises it.

### More packages are Held than before I upgraded

Expected, in three cases.

**Coming from v1.2.0 or earlier:** nog now keeps version-locked families together. If a group of packages all sit on one version and all move to the next, and even one of them is still inside its window, the whole group waits. You'll see rows marked `coupled to <package>`, all showing the same countdown, and they'll clear together on a later run.

This is the fix for [#11](https://github.com/jetomev/nog/issues/11), and it is deliberately cautious. Some families — the Qt6 stack is the reference case — are version-locked by a build convention that appears nowhere in the package metadata, so there is nothing to check against; nog goes on the pattern instead. That means it will occasionally hold a group that would have been fine. The alternative is what v1.2.0 did on 25 August: release nineteen Qt modules, hold `qt6-base`, and leave the machine unable to reach a login screen. A few extra days is the cheaper mistake.

**Coming from v1.0.2 or earlier:** kernel headers moved from Tier 3 to Tier 1 to match their kernels. They'll release in lockstep from now on. This is the protection working.

**Coming from before v1.0.5:** hold windows used to be dated from your system's older database, so updates being seen for the first time were often waved straight through. nog now dates every hold from the fresh snapshot, so new updates serve their full window. Days-remaining figures on existing holds may shift a little too — the clock is now measured from the true build date.

### `installing <A> breaks dependency '<lib>.so=N' required by <B>`

pacman refuses the whole transaction and nothing installs. This happens when a
package whose hold has expired bumps a shared library version, while a package
that still links the old one is inside its window. nog cleared one and held the
other, and the two cannot be split.

Nothing is broken — pacman caught it and declined. Move the pair forward together:

```bash
nog install <A> <B>
```

One transaction, onto the new library. Never downgrade the cleared package back;
the safe direction is always forward onto the version the repositories already
carry. Waiting also works — the held package releases on its own schedule, and
the wall disappears when it does.

**As of v1.3.1, nog holds the pair rather than letting you reach this.** The
row that would have been Ready moves to Held and names its partner:

```
libbluray  1.4.1-1  1.5.0-1  3  1 day · coupled to ffmpeg4.4
```

Both then clear on the same run. If the partner has no pending update at all —
a foreign or AUR package built against the old library — there is no countdown
to inherit and the note reads `blocked by <package>` instead, which means the
hold will not lift on its own: rebuild or update that package, or move the pair
forward with `nog install` as above.

**As of v1.4.2, nog also reads the programs themselves.** A package can link a library without declaring it at the right version — `ffmpeg-obs` declared plain `libbluray` while linking `libbluray.so.3` — and then pacman sees no conflict at all: the update goes through and the programs break silently. When a pending update drops a library version, nog reads which libraries every installed program actually needs, and treats a real user of the old version exactly like a declared one. You will see `Checking installed programs for 1 library version(s) this update removes...` when it runs; it is rare.

You can still hit the raw pacman error on v1.3.0 and earlier, or if nog cannot
read `/var/lib/pacman/local` (it says so, and carries on without the rule).

### pacman's log shows only one ignored package

After an incident you may check `/var/log/pacman.log` and find:

```
[PACMAN] Running 'pacman -Syu --ignore archlinux-appstream-data'
```

nog passes a single comma-joined `--ignore` argument. pacman splits that string
in place inside its own `argv` and only afterwards writes the `Running '...'`
line, so the log records the first name and drops the rest. One name in the log
can mean a hundred and sixty were passed. The holds were applied correctly — the
`warning: <pkg>: ignoring package upgrade` lines above it are the accurate
record. This is a pacman logging artefact, not a nog defect.

### `warning — checkupdates DB not found; using the system sync DB`

nog couldn't find the private database `checkupdates` syncs into, and fell back to the system one — which may date holds from stale information. Usually a `TMPDIR` or `CHECKUPDATES_DB` mismatch between the two tools. Check `ls "${TMPDIR:-/tmp}/checkup-db-$(id -u)/sync"` right after a run, and if the layout has moved, please file a bug.

### `warning — /etc/nog/sources.toml is unreadable`

The kill-switch file failed to parse, usually after a hand-edit. nog fails **closed**: every source is treated as switched off until the file is valid again, so updates will skip the AUR and warn you. Fix it by running `nog activate aur` (and `nog activate chaotic-aur` if needed) — each rewrites the file properly.

---

## Roadmap

> **v1.5.5 shipped 2026-09-30** — `nog clean`: tier-aware cleanup of pacman's download cache ([#15](https://github.com/jetomev/nog/issues/15)). **v1.5.4 shipped the same day** — a summary table and a Source column ([#28](https://github.com/jetomev/nog/issues/28)), `could not check` ([#25](https://github.com/jetomev/nog/issues/25)), no-keyboard AUR installs refused ([#26](https://github.com/jetomev/nog/issues/26)). Earlier that day: v1.5.3 ([#29](https://github.com/jetomev/nog/issues/29)) and v1.5.2 ([#27](https://github.com/jetomev/nog/issues/27)).

### Next — the install chain, C3 ([#7](https://github.com/jetomev/nog/issues/7) · v1.6.0)

- [ ] `nog install` tries pacman, then the AUR, then Flatpak, then Snap — and always shows which source it picked before installing.

**Validated 2026-09-29: nog works with paru** ([#12](https://github.com/jetomev/nog/issues/12)). With yay removed, `helper = "auto"` fell back to paru, a full update plan dated its AUR package correctly, and nog installed an AUR package through paru's build path. Record: [testing/](testing/20260929%20-%20Test%20Results%20for%20nog%20v1-5-1-paru.md).

### Later

- [ ] **A zero-day lane for `archlinux-keyring`** — holding the keyring back *is itself* the breakage, because signature checks then fail on every later update until it lands. It needs a special class that always releases immediately.
- [ ] **Automatic dependency coupling** — read the exact-version dependencies out of the sync DB and hold those pairs together, rather than inferring them. An audit found 736 such pairs across the repos. v1.2.1 covers the ones that share a pkgbase, which is most of them; this would close the rest and let the version-cohort heuristic step back to handling only families that declare nothing at all. **The soname half of this shipped in v1.3.1** ([#13](https://github.com/jetomev/nog/issues/13)) — a Ready package that would stop providing a library something installed still needs is now held — and since v1.4.2 ([#16](https://github.com/jetomev/nog/issues/16)) "still needs" is read from the programs themselves, not only their declarations. What remains is the versioned `=` dependency case, where the declaration is exact rather than a soname.
- [ ] **First-run setup** — on your first `nog update`, ask whether Tier 1 should auto-release after 30 days or wait for your explicit approval each time.
- [ ] `nog status` — a dashboard of what's held, ready, and overdue
- [ ] `nog history` — a log of every tier change and package action
- [ ] `nog rollback` — undo a recent update using pacman's cache
- [ ] A Chaotic-AUR binary package

### The v2 arc — one tool for every source ([design](docs/v2-design.md) · [tracking issue #7](https://github.com/jetomev/nog/issues/7))

- [x] **C1 · v1.1.0** — Flatpak
- [x] **C2 · v1.2.0** — Snap
- [ ] **C3 · v1.6.0** — Install chain: pacman → AUR → Flatpak → Snap, always showing the source before installing *(numbers moved up one: v1.5.0 went to `nog install <file>`, #17)*
- [ ] **C4 · v1.7.0** — Full command surface plus `--json` output
- [ ] **C5 · v1.8.0** — Maintenance and cleanup: orphans, AUR build caches, unused Flatpak runtimes, old Snap revisions. *Its first piece shipped early: `nog clean` for pacman's cache, v1.5.5 ([#15](https://github.com/jetomev/nog/issues/15)).*
- [ ] **C6** — nogForge, the visual companion, built on forgekit *(its gate, paru validation [#12](https://github.com/jetomev/nog/issues/12), was cleared 2026-09-29)*
- [ ] **C7 · v2.0.0** — the crown release

*Every released version's roadmap lives in [docs/ROADMAP.md](docs/ROADMAP.md).*

---

## Changelog

### v1.5.5 — September 30, 2026

**`nog clean` — tier-aware cleanup of pacman's download cache** ([#15](https://github.com/jetomev/nog/issues/15)). pacman keeps every package it ever downloads and never removes one by itself; on the development machine the cache had grown to 28 GB by August and was back to 18.4 GB by the end of September. Tools like `paccache` keep the last N of everything. nog already knows how much each package matters, so it keeps more of what hurts to lose:

- **Tier 1 keeps 3 versions, Tier 2 keeps 2, Tier 3 keeps 1** — set under `[clean]` in `nog.conf`. Tier 1 is the rollback path: the August black-screen night needed a known-good package in the cache.
- **The installed version is never removed**, so a held package always has what it runs. A newer version already downloaded stays too.
- **Packages you no longer have are cleared**, and so are day-old leftover `download-…` folders from interrupted downloads.
- **Report first, then a `[y/N]` question.** With no answer, nothing is removed. It refuses while pacman is running.
- **Versions are ordered exactly as pacman orders them** — pacman's own comparison, ported and checked against `vercmp` over every version pair in a real cache.

On the development machine: 10.8 GB of 18.4 GB could go — 1,845 old versions and 123 leftover folders — while every installed version and three of each kernel stay. The "no longer installed" count matches `paccache` exactly (346 versions, 1.9 GB).

Also in this release: the test-tally script now reads only a matrix's Result column. It had counted a check *titled* "Failure reason…" as a FAIL, so the published v1.5.0 test record said 4 FAIL where the truth was 3; that record now carries a dated correction.

Tests: 204 → 217. Warnings unchanged at 6.

### v1.5.4 — September 30, 2026

**See where everything comes from, and when a source could not be checked** ([#28](https://github.com/jetomev/nog/issues/28), [#25](https://github.com/jetomev/nog/issues/25), [#26](https://github.com/jetomev/nog/issues/26)). Asked for by Javier: *"more information, more power."*

- **A SUMMARY table replaces the four "N update(s) reported by …" lines.** One row per official repository in `pacman.conf` order, then AUR, Flatpak and Snap: Ready now, On hold, Total. An **Ask you** column appears only when something has no trustworthy date.
- **A Source column on every table.** Official packages name their repository (`core`, `extra`, `multilib`, `chaotic-aur`…); the rest say `AUR`, `Flatpak` or `Snap`, each in its own colour (peach, blue, mauve — clear of the three tier colours). AUR rows used to carry no mark at all. The Note column is hold information again.
- **A failing AUR helper reads `could not check`, never `0`** (#25). Both yay and paru answer "nothing to update" with an empty reply and exit 1, so nog took any empty reply as "nothing". A failed check looks the same, except the helper says why on its error output. nog now reads that: an empty reply with an error is a failure, shown in the summary and quoted in the warning. Tested with a helper made to fail the way paru did on 30 September: v1.5.3 said `0 AUR update(s)`; v1.5.4 says `could not check`.
- **`nog install` refuses an AUR package when nobody is at the keyboard** (#26). The helper stops to let you review each build recipe; from a script it read end-of-input in its menu and died there, with the reason buried in its output. nog now says so before starting, in plain words. It does not answer the review for you — that review is the one moment of scrutiny the AUR offers.

Tests: 193 → 204. Warnings unchanged at 6.

*Every earlier release is recorded in [docs/CHANGELOG.md](docs/CHANGELOG.md), newest-first.*

## Related Projects

- **[KognogOS](https://github.com/jetomev/KognogOS)** — the distribution nog was built for. Arch-based, KDE Plasma on Wayland, tier-aware by default.
- **[forgekit](https://github.com/jetomev/forgekit)** — the shared foundation every Forge app is built on.
- **[nogForge](https://github.com/jetomev/nogforge)** — a visual companion for nog, covering every source in one interface. In development.
- **[grubForge](https://github.com/jetomev/grubforge)** — GRUB bootloader manager.
- **[alacrittyForge](https://github.com/jetomev/alacrittyforge)** — Alacritty terminal configurator.
- **[bitlaForge](https://github.com/jetomev/bitlaforge)** — solo Bitcoin mining, honestly framed.

---

## Authors

**jetomev** — idea, vision, direction, testing

**Claude (Anthropic)** — co-developer, architecture, implementation

A collaboration between a human with a clear idea of what Linux package management should feel like, and an AI that helped design and build it — one command at a time.

---

## License

nog is free software, released under the **GNU General Public License v3.0**. See [LICENSE](LICENSE) for the full text.

---

## Contributing

nog has been stable since v1.0.0 (April 2026). Every release follows the checklist in [`testing/RELEASE-CHECKLIST.md`](testing/RELEASE-CHECKLIST.md) and ships through GitHub and the AUR with a fresh-install check on the maintainer's own machine.

Ideas, bug reports, and pull requests are all welcome. If you hit something [Troubleshooting](#troubleshooting) doesn't cover, include the output of `nog --version` and `pacman -Qi nog`, plus the failing part of your `nog update` run.
