# 📦 nog

> A tier-aware package manager for Arch Linux — pacman with a safety net, written in Rust.

![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)
![Platform: Linux](https://img.shields.io/badge/Platform-Linux-lightgrey.svg)
![Base: Arch Linux](https://img.shields.io/badge/Base-Arch%20Linux-1793d1.svg)
![Language: Rust](https://img.shields.io/badge/Language-Rust-dea584.svg)
![Status: Stable](https://img.shields.io/badge/Status-Stable-brightgreen.svg)
![Version: 1.4.0](https://img.shields.io/badge/Version-1.4.0-purple.svg)
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
- **Snap** *(v1.2.0)* — aged by the publish date in the channel you track

  Flatpak and Snap are optional in both directions. If the program isn't installed, that source sits quietly dormant — never an error. Turn any of them on or off with `nog activate|deactivate <source>`.

**Clear reporting**
- Updates grouped into **Ready** / **Held** / **Unknown**, with tier colours
- Every row shows which source it came from
- Held packages sorted by how soon they release, so the list reads as a calendar
- Packages with no usable date are never guessed at — nog asks you, one at a time
- Every run is logged to a dated CSV you can open in a spreadsheet, kept 90 days
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
9. Writes the run to a dated CSV log and prunes logs older than 90 days. If logging fails it warns you — it never blocks an update.

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
version = "1.4.0"
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
|   |-- local_db.rs       # Reads pacman's local database for the dependency graph (v1.3.1)
|   |-- reboot.rs         # Reboot advice: probes the running system  (v1.4.0)
|   |-- pacman.rs         # pacman wrapper
|   |-- aur.rs            # AUR helper detection and handoff
|   |-- flatpak.rs        # Flatpak source  (v1.1.0)
|   |-- snap.rs           # Snap source     (v1.2.0)
|   |-- sources.rs        # Kill-switch state and the pacman.conf toggle
|   |-- sync_db.rs        # Reads pacman's databases for build dates
|   |-- runlog.rs         # CSV run logging
|   |-- config.rs         # Configuration loader
|-- config/               # Default nog.conf and tier-pins.toml
|-- testing/              # Test matrix, results, and release checklist for every version
|-- docs/                 # Full changelog, full roadmap, v2 design notes
|-- nog.1                 # Man page
|-- Cargo.toml / Cargo.lock
```

Around 6,950 lines of Rust, with 128 tests that run on every release.

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
| Package transactions | `sudo pacman ...` | `install`, `remove`, `update`, `unlock --promote` — **only when no AUR helper is configured**. With a helper, nog calls the helper as you, and the helper runs its own `sudo pacman` internally. |
| Snap updates | `sudo snap refresh ...` | Only when applying snap updates. snapd requires root; nothing else about snap does. |
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

> **v1.4.0 shipped 2026-08-30** — reboot advice ([#9](https://github.com/jetomev/nog/issues/9)), written after an NVIDIA upgrade left the old module loaded and cost twenty minutes of blaming a game. v1.3.1 shipped 2026-08-28 ([#13](https://github.com/jetomev/nog/issues/13), soname coupling), found during v1.3.0's own release dogfood. The queue is priority-labelled on the [issue tracker](https://github.com/jetomev/nog/issues) — `priority-1` first.

### Next — validate against paru ([#12](https://github.com/jetomev/nog/issues/12) · `priority-3`)

- [ ] **Run nog against paru.** nog has supported paru since v1.0.0 and has never once been run against it — every release so far was built and dogfooded on a machine running yay. Scheduled deliberately for **before C6 (nogForge)**, since nogForge builds a UI over these same code paths and helper-level surprises are far cheaper to find first.

### Later

- [ ] **A zero-day lane for `archlinux-keyring`** — holding the keyring back *is itself* the breakage, because signature checks then fail on every later update until it lands. It needs a special class that always releases immediately.
- [ ] **Automatic dependency coupling** — read the exact-version dependencies out of the sync DB and hold those pairs together, rather than inferring them. An audit found 736 such pairs across the repos. v1.2.1 covers the ones that share a pkgbase, which is most of them; this would close the rest and let the version-cohort heuristic step back to handling only families that declare nothing at all. **The soname half of this shipped in v1.3.1** ([#13](https://github.com/jetomev/nog/issues/13)) — a Ready package that would stop providing a library something installed still needs is now held. What remains is the versioned `=` dependency case, where the declaration is exact rather than a soname.
- [ ] **First-run setup** — on your first `nog update`, ask whether Tier 1 should auto-release after 30 days or wait for your explicit approval each time.
- [ ] `nog status` — a dashboard of what's held, ready, and overdue
- [ ] `nog history` — a log of every tier change and package action
- [ ] `nog rollback` — undo a recent update using pacman's cache
- [ ] A Chaotic-AUR binary package

### The v2 arc — one tool for every source ([design](docs/v2-design.md) · [tracking issue #7](https://github.com/jetomev/nog/issues/7))

- [x] **C1 · v1.1.0** — Flatpak
- [x] **C2 · v1.2.0** — Snap
- [ ] **C3 · v1.5.0** — Install chain: pacman → AUR → Flatpak → Snap, always showing the source before installing
- [ ] **C4 · v1.6.0** — Full command surface plus `--json` output
- [ ] **C5 · v1.7.0** — Maintenance and cleanup: orphans, caches, unused runtimes, old snap revisions
- [ ] **C6** — nogForge, the visual companion, built on forgekit *(gated on [#12](https://github.com/jetomev/nog/issues/12) — validate against paru first)*
- [ ] **C7 · v2.0.0** — the crown release

*Every released version's roadmap lives in [docs/ROADMAP.md](docs/ROADMAP.md).*

---

## Changelog

### v1.4.0 — August 30, 2026

**nog now tells you when the machine you are running is no longer the machine you have installed.**

Found live on August 10. A `nog update` installed `nvidia-utils`, `lib32-nvidia-utils` and `nvidia-open-dkms`. DKMS rebuilt the modules correctly and the desktop kept working — until the first 3D application, which died with `Failed to initialize NVML: Driver/library version mismatch`. The old module was still loaded in memory. Twenty minutes went to suspecting the game, then Wine, then the server. nog knew exactly what it had just installed and said nothing.

It says something now, and the rule is that it may never say it anonymously:

- **Where nog can check, it checks, and marks the line `verified`** — the running kernel against what is installed, the loaded NVIDIA module against the installed driver, the running init system against the installed systemd. Those lines are observations, and they carry both versions.
- **Where nog cannot check, it names the packages instead** and says in words that this is advice rather than a finding. `glibc`, `mkinitcpio` and `grub` offer no reliable way to ask what is currently running, so nog does not pretend otherwise.
- **Session components are separated out.** `mesa`, `xorg-server`, `wayland` and `dbus` get "log out and back in", not "reboot". Demanding a reboot when a logout is enough is the same noise this feature exists to prevent.

**The kernel check deliberately parses no version numbers.** A running kernel reports `7.0.5-zen1-1-zen` while its own package reports `7.0.5.zen1-1`; comparing those two strings is a false-alarm generator, and normalising them is a second one waiting for the next kernel flavour. `/usr/lib/modules/` is named for the running kernel and that directory is removed when the kernel is replaced — so its absence *is* the finding, with nothing to parse.

**Silence is the common case, and it is enforced by test.** A package nog cleared but pacman never installed produces nothing — you can still decline individual packages at pacman's own prompt, and nog re-reads what actually landed rather than trusting its own request. A driver whose loaded module already matches produces nothing. An ordinary run performs no probing at all. Four tests exist for no purpose other than proving nog stays quiet, because a notice that appears after every run is one nobody reads — which is how the original twenty minutes were lost.

**nog recommends. nog never reboots anything.**

Also in this release: **the root `PKGBUILD` is gone.** It fetched `archive/refs/tags/` with `sha256sums=('SKIP')` and no `validpgpkeys`, while the AUR copy has used the signed release asset since v1.0.9 — and both files reported the same version, so every version check passed it. It was the only root PKGBUILD across seven repositories, it can never hold a correct checksum at the moment it is committed, and `makepkg` testing already happens against the AUR copy. Deleting it ends the divergence instead of promising to watch for it.

Tests: 100 → 128. Warnings unchanged at 6.

### v1.3.1 — August 28, 2026

**A package whose hold expires can no longer break one that is still waiting.**

Found the same evening v1.3.0 shipped, during its own release dogfood. `libbluray` had cleared its hold and moves `libbluray.so` from version 3 to version 4. `ffmpeg4.4` still linked the old one and had a day left on its window. nog put one in Ready and the other in Held, and pacman refused the whole transaction — seventy-eight packages, of which seventy-six had nothing to do with either:

```
error: failed to prepare transaction (could not satisfy dependencies)
:: installing libbluray (1.5.0-1) breaks dependency 'libbluray.so=3-64'
   required by ffmpeg4.4
```

Nothing broke — pacman caught it and declined, which is the opposite of the Qt6 split that prompted v1.2.1. But nothing installed either, and nog is supposed to hand pacman a plan that works.

**The three existing coupling rules all match on names** — a shared PKGBUILD, the `lib32-` prefix, a version cohort — and these two packages share none of them. The relationship exists only in the dependency graph, which nog had never had a reason to read. It does now: a new reader for pacman's local database supplies what is installed and what it requires, the repository metadata supplies what each pending package will provide, and the rule asks one question per candidate — *for each shared library this upgrade stops providing, will anything still provide it afterwards?* If nothing will, every installed package that still requires it would break, so the candidate waits and its row says who it is waiting for.

**Sonames are compared as whole strings, architecture suffix included, and that detail is the whole rule.** On the machine this was written for, eleven library names exist at two versions simultaneously — `ffmpeg4.4` provides `libavcodec.so=58` while `ffmpeg-obs` provides `libavcodec.so=63`, `libxcrypt-compat` sits beside `libxcrypt` — and a further 118 differ only by `-32` against `-64`. A rule that compared library *names* would couple every one of those pairs to each other and wedge the update queue permanently. All eleven are now negative tests.

**Dependents are drawn from every installed package, not just the pending ones.** A foreign or AUR package built against the old library has no repository update to wait for and breaks in exactly the same way. Such a partner has no countdown to inherit, so its row reads `blocked by <package>` rather than borrowing a countdown that would claim it releases today.

If the local database cannot be read, the rule goes quiet and nog behaves exactly as v1.3.0 did. It only pre-empts a refusal pacman would issue anyway, so failing closed would cost more than it saves.

Validated before a line of it was written, and again afterwards: the failure was replayed from the cached packages, a sweep of 161 pending updates against 1397 installed packages produced no false positives, and finally v1.3.0 and v1.3.1 were run against byte-identical restored state — the first putting `libbluray` in Ready, the second holding it, coupled, to clear with its partner.

Tests: 86 → 100. Warnings unchanged at 6. No measurable runtime cost.

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
