# 📦 nog

> A tier-aware package manager for Arch Linux — pacman with a safety net, written in Rust.

![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)
![Platform: Linux](https://img.shields.io/badge/Platform-Linux-lightgrey.svg)
![Base: Arch Linux](https://img.shields.io/badge/Base-Arch%20Linux-1793d1.svg)
![Language: Rust](https://img.shields.io/badge/Language-Rust-dea584.svg)
![Status: Stable](https://img.shields.io/badge/Status-Stable-brightgreen.svg)
![Version: 1.5.1](https://img.shields.io/badge/Version-1.5.1-purple.svg)
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
- Updates grouped into **Ready** / **Held** / **Unknown**, with tier colours
- Every row shows which source it came from
- Held packages sorted by how soon they release, so the list reads as a calendar
- Packages with no usable date are never guessed at — nog asks you, one at a time
- Every run is logged to a dated CSV you can open in a spreadsheet, kept 90 days — one row per package with its source and what actually happened to it, and the reason whenever a step did not complete *(v1.4.3)*
- **Reboot advice** *(v1.4.0)* — when a kernel or driver update leaves the running system out of step with what is now installed, nog says so at the end of the run. Where it can check, it says `verified` and shows both versions; where it cannot, it names the package and says plainly that this is advice rather than a finding

**Security**
- **One manager per source** *(v1.3.0)* — pacman upgrades official packages; your AUR helper is handed only the AUR packages nog cleared, **by name**. Nothing unnamed can move, so a failed AUR lookup cannot release a hold by omission. Born from a real bypass we caught on our own machine, originally patched by the foreign fence *(v1.0.9)*, which now backs it up as a second layer.
- **Kill switches** *(v1.0.9)* — `nog deactivate aur` or `nog deactivate chaotic-aur` cuts off a supply chain in one command during an incident. `nog activate` puts it back exactly as it was.
- **Runs as you, not as root** — nog escalates only at the specific moments root is genuinely needed, and you see every prompt. See [Privilege model](#privilege-model).
- **Holds use pacman's own `--ignore`**, so there's no mechanism by which nog could quietly skip one.

---

## The Three-Tier System

Tier assignments live in `/etc/nog/tier-pins.toml` and can be changed any time with `nog pin`. Hold durations live in `/etc/nog/nog.conf`.

### Tier 1 — 30 days

The packages that can stop your machine from booting. Held for 30 days after the update is published upstream — a full month of everyone else testing it first. When the hold expires it installs normally.

**Default members:** `linux`, `linux-zen`, `linux-lts`, `linux-hardened`, `systemd`, `systemd-libs`, `glibc`, `grub`, `efibootmgr`, `mkinitcpio`, `pacman`, `mesa`

> **Expert mode.** Set `manual_signoff = true` under `[tier1]` in `tier-pins.toml` and Tier 1 stops auto-releasing entirely — every kernel, glibc and systemd update then needs an explicit `nog unlock <pkg> --promote`. Worth it only if you want to personally look at each one.

### Tier 2 — 15 days

Your desktop and the applications you'd notice breaking. Long enough for real problems to surface, short enough that you don't fall behind.

**Default members:** `plasma-meta`, `plasma-desktop`, `sddm`, `pipewire`, `pipewire-pulse`, `wireplumber`, `networkmanager`, `firefox`, `dolphin`, `konsole`, `kate`, `grubforge`, `alacritty`, `fish`, `alacrittyforge`

### Tier 3 — 7 days

Everything else, which is most of your system. A short buffer with no meaningful delay.

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

# Kill switches — cut off a source during a security incident
nog deactivate aur           # every AUR path refuses until reactivated
nog deactivate chaotic-aur   # repo commented out of pacman.conf (backup taken first)
nog activate aur             # restore
nog activate chaotic-aur     # restore, byte for byte, then refresh

nog --version
nog --help
```

### What `nog update` actually does

1. Asks `checkupdates` for pending official-repo updates. This syncs into its own private database, so your system's package database is left alone.
2. If an AUR helper is configured, adds pending AUR updates to the same list.
3. Reads build dates from **the fresh database `checkupdates` just synced** — not the older system copy. For AUR packages, it reads the helper's cached information instead.
4. Works out each package's tier and whether its hold has expired.

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

From a real run, with the Held section trimmed:

```
=============
nog v1.3.0
Update!
=============

Date: 07/29/2026
Time: 08:52 PM
User: jetomev

nog: Checking for pending updates ...

nog: 75 official repository update(s) reported by pacman.
nog: 1 AUR update(s) reported by yay.

READY TO INSTALL:
=================

Package (3)     Old Version   New Version   Tier  Note
-------------------------------------------------------------------
libraqm         0.10.5-1      0.11.0-1      3     hold just expired
plasma-desktop  6.7.2-1       6.7.3-1       2     hold just expired
python-certifi  2026.06.17-1  2026.07.22-1  3     1 day past window

ON HOLD FROM INSTALL:
=====================

Package (73)       Old Version               New Version              Tier  Note
-----------------------------------------------------------------------------------------------------
archlinux-keyring  20260707.1-1              20260727-1               3     4 days remaining
glibc              2.43+r37+gfdf10644d6ee-1  2.44+r5+g7cba77790f32-1  1     28 days remaining
libnm              1.56.1-2                  1.58.0-1                 2     5 days remaining
linux-zen          7.0.5.zen1-1              7.1.5.zen1-2             1     28 days remaining
linux-zen-headers  7.0.5.zen1-1              7.1.5.zen1-2             1     28 days remaining
lib32-libnm        1.56.1-1                  1.58.0-1                 3     5 days · coupled to libnm
  ⋮                (67 more)

UNKNOWN:
========

(none)

nog: Begin the handoff? [Y/n] y

nog: Handing off official packages to pacman ...
:: Starting full system upgrade...
   (the pacman transaction runs here)

nog: Handing off 2 AUR package(s) to yay ...
     (yay shows its own build and transaction below)

nog: Update finished!
nog: run logged to /home/jetomev/.local/share/nog/logs/20260729 nog-update.csv

Thank you for using nog!
```

The tier digit is colour-coded — red, yellow, green. Two details worth spotting: `linux-zen` and its headers hold together at the same 28 days, and `lib32-libnm` is marked `coupled to libnm`, held because its 64-bit twin is.

---

## Configuration

nog reads its configuration from `/etc/nog/`.

### `nog.conf`

General settings, and **the authoritative hold durations**.

```toml
[general]
version = "1.5.1"
log_level = "info"

[paths]
tier_pins = "/etc/nog/tier-pins.toml"
pacman_conf = "/etc/pacman.conf"
log_file = "/var/log/nog.log"
# Per-run CSV logs, kept 90 days. nog runs unprivileged, so these live in
# your home directory. A leading ~/ expands against $HOME.
run_logs = "~/.local/share/nog/logs"

[holds]
tier1_days = 30
tier2_days = 15
tier3_days = 7

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

Around 8,650 lines of Rust, with 172 tests that run on every release.

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

Four places. That's the complete list.

| What | Command | When |
|---|---|---|
| Package transactions | `sudo pacman ...` | `nog update` always hands the official repositories to `sudo pacman -Syu` itself (v1.3.0), and `nog install` of a package file always uses `sudo pacman -U` (v1.5.0). `install` by name, `remove` and `unlock --promote` use it **only when no AUR helper is configured**; with a helper, nog calls the helper as you, and the helper runs its own `sudo pacman` internally. |
| Snap holds and updates | `sudo snap refresh ...` | Placing a tier hold (`--hold`, v1.4.1) and applying snap updates. snapd requires root for both; nothing else about snap does. |
| Its own config files | `sudo tee <file>` | Writing `tier-pins.toml` (during `nog pin`) and `sources.toml` (during `activate`/`deactivate`). The new contents are built in memory and piped to `tee` — nog itself never runs as root, only `tee` does. |
| pacman.conf backup | `sudo cp --preserve=all` | Only during `nog activate|deactivate chaotic-aur`, to take a timestamped backup before editing that one section. |

### What nog reads

All world-readable, so nog reads them as you: `/etc/nog/nog.conf`, `/etc/nog/tier-pins.toml`, `/etc/pacman.conf`, and pacman's sync databases.

### What nog writes

Three files, each with one well-defined writer:

- `/etc/nog/tier-pins.toml` — during `nog pin`
- `/etc/nog/sources.toml` — during `nog activate` / `nog deactivate`
- `/etc/pacman.conf` — **only** by `activate|deactivate chaotic-aur`, which comments the `[chaotic-aur]` section in or out using a `#nog#` marker, after a timestamped backup. Restoring is byte-exact, and your own comments inside that section survive. No other command touches this file.

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

> **v1.5.1 shipped 2026-09-29** — a clean build log ([#24](https://github.com/jetomev/nog/issues/24)). **v1.5.0 shipped the same day** — `nog install` takes a package file you built ([#17](https://github.com/jetomev/nog/issues/17)), and carries v1.4.2 (nog reads which libraries programs actually use, [#16](https://github.com/jetomev/nog/issues/16)) and v1.4.3 (a truthful run log, [#19](https://github.com/jetomev/nog/issues/19)–[#22](https://github.com/jetomev/nog/issues/22)). The queue is priority-labelled on the [issue tracker](https://github.com/jetomev/nog/issues) — `priority-1` first.

### Next — validate against paru ([#12](https://github.com/jetomev/nog/issues/12) · `priority-3`)

- [ ] **Run nog against paru.** nog has supported paru since v1.0.0 and has never once been run against it — every release so far was built and dogfooded on a machine running yay. Scheduled deliberately for **before C6 (nogForge)**, since nogForge builds a UI over these same code paths and helper-level surprises are far cheaper to find first.

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
- [ ] **C5 · v1.8.0** — Maintenance and cleanup: orphans, caches, unused runtimes, old snap revisions ([#15](https://github.com/jetomev/nog/issues/15) `nog clean` belongs here)
- [ ] **C6** — nogForge, the visual companion, built on forgekit *(gated on [#12](https://github.com/jetomev/nog/issues/12) — validate against paru first)*
- [ ] **C7 · v2.0.0** — the crown release

*Every released version's roadmap lives in [docs/ROADMAP.md](docs/ROADMAP.md).*

---

## Changelog

### v1.5.1 — September 29, 2026

**A clean build log.** One of the tests added in v1.4.3 starts a real program that deliberately fails with `error: no space left`, to prove nog captures the reason. It passed — but it let that fake error through to the real screen, so every package build printed an error line that was not one ([#24](https://github.com/jetomev/nog/issues/24), found during the v1.5.0 smoke build). Anyone reading an AUR build log would reasonably have stopped at it. The test now sends that output nowhere. nog itself is unchanged.

Also recorded: v1.5.0 was installed on the development machine twice — first with its own `nog install <file>`, then fresh from the AUR through yay. Both paths work.

Tests: 172 → 172. Warnings unchanged at 6.

### v1.5.0 — September 29, 2026

**`nog install` now installs a package file you built yourself** ([#17](https://github.com/jetomev/nog/issues/17)).

```bash
nog install ./grubforge-1.1.1-1-any.pkg.tar.zst
# nog: 'grubforge' 1.1.1-1 (local file) is Tier 3 — installing from ./grubforge-1.1.1-1-any.pkg.tar.zst.
```

Every Forge release ends with a `makepkg` build and a test run against the installed result. Until now that one step had to leave nog for raw `sudo pacman -U`, on the machine whose whole purpose is to exercise nog. nog now recognises a package file — anything whose name contains `.pkg.tar`, which no repository package name can — and hands it to `pacman -U`.

- **The tier comes from the file itself.** A local build has no repository entry, so nog reads the name and version from the package (`pacman -Qip`, with the language forced to English so the field labels cannot be translated out from under it).
- **Names and files cannot be mixed.** `pacman -S` and `pacman -U` are separate transactions; splitting one request into two could install half of it. nog says so and asks for two commands.
- **A mistyped path is a missing file**, reported as one — never searched for in the repositories as if it were a package name.
- **No signature gate.** Like every `nog install`, this is a command you typed, and it does what you asked; pacman's own `LocalFileSigLevel` applies exactly as it would without nog. Protection lives in `nog update`, the path that runs without you choosing each package.

**This release also carries v1.4.2 and v1.4.3**, tagged the same day without GitHub Releases of their own: nog now reads which libraries programs actually use before letting a library change ([#16](https://github.com/jetomev/nog/issues/16)), and the run log records each package's own outcome, its source, and why a step failed ([#19](https://github.com/jetomev/nog/issues/19)–[#22](https://github.com/jetomev/nog/issues/22)). Both entries follow below.

**A new check on the error relay from v1.4.3.** pacman asks `Proceed with installation? [Y/n]` without a newline and then waits. A relay that waited for whole lines would hide the question you were being asked. A test now times the question's arrival — and was run once against a deliberately broken relay to prove it fails when it should (the question took a full second; the test caught it).

**Two documentation corrections.** The man page listed the run log's twelve column names as one unbreakable word, which the formatter could not fit — introduced in v1.4.3 and caught by the release checklist's formatting check. And both privilege sections said nog runs `sudo pacman` for updates "only when no AUR helper is configured"; since v1.3.0 the update always hands the official repositories to `sudo pacman` itself. They now say so, and name the new `pacman -U`.

Tests: 166 → 172. Warnings unchanged at 6.

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
