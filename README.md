# 📦 nog

> A tier-aware package manager for Arch Linux — pacman with a safety net, written in Rust.

> 🖥 **Where it runs:** **Arch Linux and Arch-based distributions** (it drives pacman) · **no desktop needed**: any desktop, or none · **works on a plain text console** (a tty).

![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)
![Platform: Linux](https://img.shields.io/badge/Platform-Linux-lightgrey.svg)
![Base: Arch Linux](https://img.shields.io/badge/Base-Arch%20Linux-1793d1.svg)
![Language: Rust](https://img.shields.io/badge/Language-Rust-dea584.svg)
![Status: Stable](https://img.shields.io/badge/Status-Stable-brightgreen.svg)
![Version: 2.1.0](https://img.shields.io/badge/Version-2.1.0-purple.svg)
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
- **Keys are never held** *(v1.5.6)* — `archlinux-keyring` and `chaotic-keyring` skip the tiers and install first, because holding the keys back makes every later signature check fail. A key store that was never set up is set up for you
- **Installs and removals ask first** *(v1.5.8)* — pacman shows exactly what will change, dependencies included, and waits for your answer.
- **Every run starts and ends the same way** *(v1.5.8)* — a banner with nog's version, the command, date, time and user; then "Done" or "Stopped", the logs written and a thank-you. Every run is logged (`nog-runs.csv`)
- **Every run kept whole** *(v1.6.0)* — what a run showed on screen (nog's tables, pacman's questions, an AUR build, any error) is kept as `YYYYMMDD-HHMMSS <command>.log` beside the CSV logs, for 30 days. Recorded with util-linux `script`: you answer prompts as usual, and what you type (a password included) is never in the file. nogForge's nog Logs opens it. It is the log the closing note names; the CSV logs stay as nog's own bookkeeping (the hold clock, nogForge's tables)
- **One command set for every source** *(v2.0.0 — the v2 arc complete)* — `install`, `remove`, `reinstall`, `search`, `list`, `info`, `tier`, `lock`/`unlock`, `status`, `history` and `clean` all work across the repositories, the AUR, Flatpak and Snap, each source through the tool that owns it, in its own voice; every query command answers in JSON with `--json`
- **Locks** *(v2.0.0)* — `nog lock <pkg>` keeps a package at its installed version until `nog unlock <pkg>`: `nog update` lists it as held, *"locked by you"*, pacman is told to ignore it, and `--promote` never pulls it in. Written to `/etc/nog/locks.toml`, so every user of the computer and nogForge see the same locks
- **Linchpins** *(v2.0.0, [#5](https://github.com/jetomev/nog/issues/5))* — when `glibc` or `gcc-libs` has a held update that changes its series (2.44 → 2.45, which brings new `GLIBC_2.45` symbol versions), the fast-lane packages built after that new build are held with it (*"12 days · built after the new glibc, moves with it"*), because they were probably linked against it and would fail at runtime. A same-series rebuild moves nothing. The way out is to promote the linchpin, never to downgrade the fast lane
- **Interpreters and their module folders** *(v2.1.0, [#61](https://github.com/jetomev/nog/issues/61))* — perl and python install compiled modules into a folder named for their version (`/usr/lib/perl5/5.42/`). When one of them changes series, it waits for every installed package with files in the old folder (*"1 day · coupled to apparmor"*), or reads `blocked by` a package with no rebuild yet. Arch states this link only in the paths, so nog reads the file lists, and only when such a move is pending
- **One install command for every source** *(v1.9.0)* — `nog install <name>` looks in the repositories, then the AUR, then Flatpak, then Snap, and the table says which one answered before anything is downloaded or built. A Flatpak is matched exactly (the app ID, its last word, or the app's name — *gimp*, *spotify*); when several apps answer to a name nog numbers them and asks which one. `flatpak/<app-id>` and `snap/<name>` name the source. A backend that isn't set up is offered, once, to a person at the keyboard — never installed unasked
- **What nog installs, nog removes** *(v1.9.0)* — `nog remove` recognises an installed Flatpak or snap by name and hands it to its own tool
- **A removal says what else goes, and who still uses it** *(v1.9.0, F-13)* — before pacman runs, every package the removal would take along that another installed app still uses *optionally* is named in plain words (*"ell goes too, but bluez-utils uses it (optional: for btpclient)"*), with a Keep for each, default yes. `nog keep <pkg>` does the same on its own: marked as installed on purpose, never taken along again
- **`--yes` for scripts** *(v1.9.0)* — `nog install --yes` and `nog remove --yes` answer the question for you (pacman's, flatpak's, nog's own for Snap). Never the AUR's build review. Without it and with no terminal, nog now says that nothing is at the keyboard, instead of blaming you for a "no" nobody gave
- **See where it comes from before installing** *(v1.8.0)* — `nog install` shows each package's version, source and tier before anything is downloaded or built; a repository package standing in for the name is said plainly (*"chaotic-aur — unifetch provides it"*). When the AUR is involved nog asks first, then says it's handing off to yay; a name found nowhere stops before anything happens
- **Install from the source you name** *(v1.7.0)* — `nog install aur/<name>` is always built from the AUR, `nog install extra/<name>` comes from that repository only; nothing can be swapped in because it "provides" the same name
- **Steps for programs** *(v1.7.0)* — with `NOG_EVENTS=<file>`, nog writes each step it takes (and its own questions) as a line a program can read; nogForge shows them as a checklist with a progress bar
- **Update only what you name** *(v1.6.1)* — `nog update a b c` shows one table with just those and installs only them. A named package that is on hold must be promoted first (`--promote`); otherwise nog stops before anything runs and says what to do
- **An install installs only what you asked** *(v1.6.0)* — like `pacman -S`: no update first, and never a list refresh on its own (that would be a partial upgrade). If an install fails and the package lists are old, nog says so: `nog update`, then try again
- **A hold always ends** *(v1.5.2)* — the wait counts from the first new version, so a stream of newer builds can't keep a package back forever. The version that installs must still be a few days old (the safety wait)
- Pin anything to any tier — `nog tier <pkg> --set <N>` (or `nog pin <pkg> --tier=<N>`); `nog tier <pkg>` says which tier and why *(v2.0.0)*
- Need a held package now? `nog unlock <pkg> --promote`
- Expert mode: require your explicit approval for *every* Tier 1 update

**Every source, one set of rules**
- **Official repos** through pacman
- **AUR** through yay or paru, detected automatically
- **Flatpak** *(v1.1.0)* — aged by the pending release's publish date; installed and removed through nog since v1.9.0; searched, listed, reinstalled and cleaned up since v2.0.0
- **Snap** *(v1.2.0)* — aged by the publish date in the channel you track, and since v1.4.1 the hold is placed in snapd itself, so its own four-times-a-day auto-refresh cannot walk past it; installed and removed through nog since v1.9.0; searched, listed and cleaned up since v2.0.0

  Flatpak and Snap are optional in both directions. If the program isn't installed, that source sits quietly dormant — never an error — and `nog install` offers to set it up only when the chain reaches it with a person at the keyboard. Turn any of them on or off with `nog activate|deactivate <source>`.

**Clear reporting**
- A **summary table** first *(v1.5.4)* — one row per repository (`core`, `extra`, `multilib`, `chaotic-aur`…), then AUR, Flatpak and Snap, with what is ready now and what is on hold. A source that could not be checked says `could not check`, never `0`
- Updates grouped into **Ready** / **Held** / **Unknown**, with tier colours
- Every row has a **Source** column *(v1.5.4)*: the repository it comes from, or `AUR`, `Flatpak`, `Snap` — each non-official source in its own colour
- Held packages sorted by how soon they release, so the list reads as a calendar
- Packages with no usable date are never guessed at — nog asks you, one at a time
- Every run is logged to a dated CSV you can open in a spreadsheet, kept 90 days — one row per package with its source and what actually happened to it, and the reason whenever a step did not complete *(v1.4.3)*
- **Reboot advice** *(v1.4.0)* — when a kernel or driver update leaves the running system out of step with what is now installed, nog says so at the end of the run. Where it can check, it says `verified` and shows both versions; where it cannot, it names the package and says plainly that this is advice rather than a finding

**Maintenance**
- **`nog clean`, every source** *(v2.0.0, C5)* — one report of what each source leaves behind, a numbered row per category with its size: pacman's old downloads (since v1.5.5: 3 versions of each Tier 1 package kept, 2 of Tier 2, 1 of Tier 3, never the installed one), **orphans** (installed as dependencies, needed by nothing now), the **AUR helper's build trees**, **Flatpak runtimes no app uses**, and **the old snap revisions** snapd keeps. You pick the categories (*"Clean up which? [all, numbers like 1 3, or n for none]"*); each is removed by the tool that owns it — pacman shows the orphans and asks, flatpak works out its exact list and removes it (your pick is the yes), nog removes the files it owns. `--only a,b` picks, `--yes` answers, `--json` is the report alone (nogForge's Maintenance). On the test desktop the first report found 10 GB
- **`nog status`** *(v2.0.0)* — the dashboard: what is installed from where, which sources are on, what is ready and what is held with what is released next, the locks, the last run, the last update, the last reboot advice
- **`nog history`** *(v2.0.0)* — everything nog has done, newest first, from its own logs: every run and its outcome, every package an update decided on, every reboot advice
- **`nog info`** *(v2.0.0)* — everything nog knows about one package, from whichever source has it: source, version, tier and why, how long its updates wait, the details its own tool gives
- **`nog reinstall`** *(v2.0.0)* — puts back the files of installed packages, each through its own tool; a snap cannot be reinstalled in place, so nog says the way and what it costs instead of doing it

**Security**
- **One manager per source** *(v1.3.0)* — pacman upgrades official packages; your AUR helper is handed only the AUR packages nog cleared, **by name**. Nothing unnamed can move, so a failed AUR lookup cannot release a hold by omission. Born from a real bypass we caught on our own machine, originally patched by the foreign fence *(v1.0.9)*, which now backs it up as a second layer.
- **Kill switches** *(v1.0.9)* — `nog deactivate aur` or `nog deactivate chaotic-aur` cuts off a supply chain in one command during an incident. `nog activate` puts it back exactly as it was.
- **Runs as you, not as root** — nog escalates only at the specific moments root is genuinely needed, and you see every prompt. See [Privilege model](#privilege-model).
- **Holds use pacman's own `--ignore`**, so there's no mechanism by which nog could quietly skip one.
- **Signed and verifiable** — every release is GPG-signed and every commit is GitHub-Verified. **[Where We Stand](https://github.com/jetomev/KognogOS/blob/main/docs/where-we-stand.md)** covers our response to the 2026 AUR supply-chain attacks and how to check us yourself.

---

## The Three-Tier System

Tier assignments live in `/etc/nog/tier-pins.toml` and can be changed any time with `nog pin`. Hold durations live in `/etc/nog/nog.conf`.

### Tier 1 — 30 days

The packages that can stop your machine from booting. Held for 30 days after the first new version is published upstream — a full month of everyone else testing it first. When the hold expires it installs normally.

**Default members:** `linux`, `linux-zen`, `linux-lts`, `linux-hardened`, `systemd`, `systemd-libs`, `glibc`, `grub`, `efibootmgr`, `mkinitcpio`, `pacman`, `mesa`

> **Expert mode.** Set `manual_signoff = true` under `[tier1]` in `tier-pins.toml` and Tier 1 stops auto-releasing entirely — every kernel, glibc and systemd update then needs an explicit `nog unlock <pkg> --promote`. Worth it only if you want to personally look at each one.

### Tier 2 — 15 days

The services and apps you'd notice breaking, pinned there by you or by your distribution. Long enough for real problems to surface, short enough that you don't fall behind.

**Default members:** `pipewire`, `pipewire-pulse`, `wireplumber`, `networkmanager`: generic plumbing. Your desktop and apps are your choice to pin (`nog tier <pkg> --set 2`); KognogOS ships its own, fuller list *(trimmed in v2.1.0)*

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

**Everything it needs** to build, run and test, with why, is in [docs/PACKAGING.md](docs/PACKAGING.md), measured from the code (the Forge Suite's [dependency audit](https://github.com/jetomev/forge-suite/issues/91), 2026-10-11).

- Arch Linux, or an Arch-based distribution
- `pacman`, `pacman-contrib` and `fakeroot` (pacman-contrib's update check needs it)
- `sudo`: every change nog makes runs through it (with `-A` for a password window)
- `zstd`: nog links the system's zstd library (pacman brings it on every Arch)
- `flatpak`, `snapd` — optional: nog updates and installs them when they're there, and leaves them out when they're not
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
# Install a package (shows what will install and asks; routes to your AUR helper if needed)
nog install <package>

# Install a package file you built yourself, e.g. with makepkg
nog install ./<package>-<version>-<arch>.pkg.tar.zst

# Update everything (tier holds applied across all sources)
nog update

# Search every source — the repositories, the AUR, Flatpak, Snap — with each result's tier (v2.0.0)
nog search <query>

# Everything about one package, from whichever source has it (v2.0.0)
nog info <name>

# The dashboard, and everything nog has done (v2.0.0)
nog status
nog history --days 7

# Install from that source only: the AUR, or one repository (v1.7.0); a Flatpak or a snap (v1.9.0)
nog install aur/<package>
nog install extra/<package>
nog install flatpak/<app-id>
nog install snap/<name>

# From a script: answer the install's or removal's question for you — never the AUR's review (v1.9.0)
nog install --yes <package>
nog remove --yes <package>

# Mark a package as installed on purpose, so removing something else never takes it along (v1.9.0)
nog keep <package>

# Update only these: nothing else moves; a held one must be promoted (v1.6.1)
nog update <package> <package>
nog update <package> <held-package> --promote <held-package>

# Keep some updates back this time; what must stay with them follows (v1.6.0)
nog update --keep <package>,<package>

# Promote held updates: ready now, installed with the rest; partners they need come too (v1.6.0)
nog update --promote <package>,<package>

# Installed packages, Flatpak apps and snaps with their tier and source (v1.6.0; every source since v2.0.0)
nog list

# A package's tier and why; move it to another tier (v2.0.0; `nog pin <package> --tier=<N>` still works)
nog tier <package>
nog tier <package> --set <1|2|3>

# Lock a package at its installed version until you unlock it (v2.0.0)
nog lock <package>
nog lock                     # the list
nog unlock <package>

# Install a held Tier 1 package right now
nog unlock <package> --promote

# Put back the files of an installed package, through its own tool (v2.0.0)
nog reinstall <package>

# Remove a package, a Flatpak or a snap (asks first; says what else goes and who still uses it)
nog remove <package>

# Clean up what every source leaves behind: old downloads, orphans, build trees, unused runtimes, old snap revisions (v2.0.0)
nog clean
nog clean --only cache,orphans --yes

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
9. Writes the run to a dated CSV log — each package with its source and its own outcome — and prunes logs older than 90 days. Every nog command, not only updates, also adds one line to the day's `nog-runs.csv` *(v1.5.8)*, and its closing notice names the logs it wrote. If logging fails it warns you — it never blocks an update. Any reboot advice goes to a companion `nog-reboot.csv` beside it.

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

### nog for programs *(v1.6.0)*

[nogForge](https://github.com/jetomev/nogforge), the Forge Suite's package app, shows what nog decides and never decides itself. So nog says it as data: `--json` gives exactly one JSON document on stdout (everything nog usually prints goes to stderr), and a JSON run writes no banner and no line in the run history.

| Command | Gives |
|---|---|
| `nog list --json` | every installed package: version, description, tier, source, whether you chose it, what needs it, and why it may not be removed (*the system needs this to start*, *part of the base system*, *needed by …*); since v2.0.0 the installed Flatpak apps and snaps too, each with `remove_as` |
| `nog search --json <q>` | every source *(v2.0.0)*: the repositories, the AUR through your helper, Flatpak and Snap; a Flatpak or snap row carries `install_as` (`flatpak/<app-id>`, `snap/<name>`), the exact name to give `nog install` |
| `nog info --json <name>` | one package *(v2.0.0)*: source, installed, version, description, tier and why, hold in words, locked, the source's own fields |
| `nog tier --json <name>` · `nog lock --json` | the tier with its reason and hold days; the lock list *(v2.0.0)* |
| `nog status --json` · `nog history --json` | the dashboard (counts, sources, the plan, locks, last runs); the runs, update decisions and reboot advice as rows *(v2.0.0)* |
| `nog clean --json` | the maintenance report alone, nothing removed: categories with `id`, `items`, `bytes`, `names`, `how`; then `nog clean --only a,b` runs the clean-up with the tools asking in the program's window *(v2.0.0)* |
| `nog update --json [--keep a,b]` | the plan, from the same code as a real update, stopped before any question or change; with `--keep`, what you keep back and what must stay with it (`coupled_to`) |

`--promote` is the opposite: the packages named are ready now and install with the rest of the update (nogForge's Promote). When a coupling rule would pull one back because its partner is still held, the partner is promoted with it instead: promoting `linux-zen` brings `linux-zen-headers` along ("promoted with linux-zen"). Keeping a package back wins over promoting it. `nog unlock <pkg> --promote` still installs one at once, on its own.

`--keep` works in a real `nog update` too: the packages named are held before the coupling rules run, so their partners stay back with them. Keeping `ldb` back, for example, also holds `libwbclient` and `smbclient`, which come from the same Samba build.

A program (or a script) that installs or removes on its own passes `--yes` *(v1.9.0)*: pacman, flatpak and nog's own Snap question are answered, what another app still uses is kept on a removal, and an AUR name stops before anything, because the AUR's build review needs a person. Without `--yes` and without a terminal, nog stops first and says nothing is at the keyboard.

With `NOG_ASKPASS=1` in the environment, every `sudo` nog runs gets `-A`, and the AUR helper gets `--sudoflags -A`: the system's own password window asks (set `SUDO_ASKPASS`, e.g. KDE's `ksshaskpass`), never a terminal a program is drawing on. nogForge sets it.

## Configuration

nog reads its configuration from `/etc/nog/`.

### `nog.conf`

General settings, and **the authoritative hold durations**.

```toml
[general]
version = "2.1.0"
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
    "pipewire",
    "networkmanager",
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
|   |-- commands/mod.rs   # install, remove, keep, update: the engine
|   |-- commands/surface.rs # search, list, info, reinstall, tier, lock/unlock, status, history  (v2.0.0)
|   |-- commands/maintain.rs # nog clean across every source  (v2.0.0)
|   |-- locks.rs          # /etc/nog/locks.toml  (v2.0.0)
|   |-- maint.rs          # What each source leaves behind: orphans, build trees, the choice  (v2.0.0)
|   |-- history.rs        # Reads nog's own CSV logs  (v2.0.0)
|   |-- tiers.rs          # Tier classification, including auto-coupling and [groups]
|   |-- holds.rs          # Hold evaluation, the coupling rules, the linchpin rule (#5, v2.0.0), interpreters (#61, v2.1.0), the foreign fence
|   |-- sightings.rs      # When each update was first seen, so holds always end  (v1.5.2)
|   |-- cache.rs          # nog clean: pacman's version order, per-tier cache retention  (v1.5.5)
|   |-- local_db.rs       # Reads pacman's local database: dependency graph (v1.3.1), who links what (v1.4.2)
|   |-- elf.rs            # Reads which libraries a program needs  (v1.4.2)
|   |-- reboot.rs         # Reboot advice: probes the running system  (v1.4.0)
|   |-- pacman.rs         # pacman wrapper
|   |-- aur.rs            # AUR helper detection and handoff
|   |-- removal.rs        # What a removal takes along, and who still uses it  (v1.9.0)
|   |-- flatpak.rs        # Flatpak source: updates (v1.1.0), install and remove (v1.9.0)
|   |-- snap.rs           # Snap source: updates (v1.2.0), install and remove (v1.9.0)
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

Around 16,000 lines of Rust, with 285 tests that run on every release.

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

Eight places. That's the complete list.

| What | Command | When |
|---|---|---|
| Package transactions | `sudo pacman ...` | `nog update` always hands the official repositories to `sudo pacman -Syu` itself (v1.3.0), and `nog install` of a package file always uses `sudo pacman -U` (v1.5.0). `install` by name, `remove` and `unlock --promote` use it **only when no AUR helper is configured** — or with `--yes` (v1.9.0), when there is nothing for a helper to ask; otherwise nog calls the helper as you, and the helper runs its own `sudo pacman` internally. |
| Keeping a package | `sudo pacman -D --asexplicit` | `nog keep`, and the Keep answered yes in `nog remove` (v1.9.0): pacman's own install reason, nothing else. |
| Snap holds, updates, installs and removals | `sudo snap refresh / install / remove ...` | Placing a tier hold (`--hold`, v1.4.1), applying snap updates, and since v1.9.0 installing or removing a snap through nog. snapd requires root for all of it; nothing else about snap does. |
| Setting up a backend | `sudo pacman -S flatpak` · `sudo systemctl enable --now snapd.socket` · `sudo ln -s /var/lib/snapd/snap /snap` | Only after you answer yes to nog's offer in `nog install` (v1.9.0); snapd itself is built from the AUR by the helper, as you. Flatpak installs and removals run as you — flatpak asks the system's own permission service (polkit) for a system-wide install, which is the prompt you see. |
| Its own config files | `sudo tee <file>` | Writing `tier-pins.toml` (during `nog pin`) and `sources.toml` (during `activate`/`deactivate`). The new contents are built in memory and piped to `tee` — nog itself never runs as root, only `tee` does. |
| Cache cleanup | `sudo rm -f --` | Only in `nog clean` (v1.5.5), only after you answer yes, and only on package files and day-old leftover `download-XXXXXX` folders directly inside pacman's cache directories. |
| pacman.conf backup | `sudo cp --preserve=all` | Only during `nog activate|deactivate chaotic-aur`, to take a timestamped backup before editing that one section. |

### What nog reads

All world-readable, so nog reads them as you: `/etc/nog/nog.conf`, `/etc/nog/tier-pins.toml`, `/etc/pacman.conf`, and pacman's sync databases.

### What nog writes

System files, each with one well-defined writer:

- `/etc/nog/tier-pins.toml` — during `nog pin` and `nog tier --set`
- `/etc/nog/locks.toml` — during `nog lock` / `nog unlock` (v2.0.0)
- `/var/lib/nog/last-update` — the record of the computer's last completed update, by `nog update` right after pacman's step (v2.1.0; before, one per user)
- `/etc/nog/sources.toml` — during `nog activate` / `nog deactivate`
- `/etc/pacman.conf` — **only** by `activate|deactivate chaotic-aur`, which comments the `[chaotic-aur]` section in or out using a `#nog#` marker, after a timestamped backup. Restoring is byte-exact, and your own comments inside that section survive. No other command touches this file.

In your home folder, as you: the run logs (`~/.local/share/nog/logs/`: the update log by `nog update`, the runs log by every command since v1.5.8, and each run whole as a `.log` since v1.6.0, kept 30 days), the hold record (`~/.local/state/nog/holds.tsv`, v1.5.2) and, only where sudo may run pacman alone (a timer's rule), the record of the last completed update (`~/.local/share/nog/last-update`, v1.5.7).

### What nog never touches

Everything else. Mirrorlists, pacman's installed-package state (except a package's *install reason*, through pacman's own `-D`, when you keep it), its cache, its GPG keyring and signature checks, `/etc/sudoers`, PAM, and every system binary directory. Every byte of `pacman.conf` outside that one section survives untouched — there's a unit test for it.

### Working with your AUR helper

When a helper is configured, nog asks it for pending AUR updates and hands transactions to it, always **as your user**. The helper fetches and builds as you, then runs its own `sudo pacman` when it reaches that step — that prompt comes from the helper, not from nog.

nog never runs `sudo yay` or `sudo paru`. That's a deliberate refusal, for the same reason those tools refuse it themselves: building packages as root is unsafe.

---

## Troubleshooting

### `checkupdates failed: Cannot find the fakeroot binary`

nog's update check comes from `pacman-contrib`, and it needs `fakeroot`, which pacman-contrib lists only as optional. A freshly installed system may not have it. nog depends on it since v1.5.7; if you see this anyway, install it together with a full update:

```sh
sudo pacman -Syu fakeroot
```

Not `pacman -S fakeroot` on its own: with old package lists that is a partial upgrade.

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

> **v2.1.0** (built, waits on Javier's run) — unattended updates: `nog update --yes`, and an update that doesn't finish says so in its exit status.
>
> **v2.0.0** — the crown release: one tool for every source. The command surface across the repositories, the AUR, Flatpak and Snap (`search`, `list`, `info`, `reinstall`, `tier`, `lock`, `status`, `history`, all with `--json`), maintenance across every source (`nog clean`), and the linchpin rule for glibc and gcc-libs ([#7](https://github.com/jetomev/nog/issues/7) complete, [#5](https://github.com/jetomev/nog/issues/5)). **v1.9.0 shipped 2026-10-14** — the install chain is whole: `nog install` looks in the repositories, then the AUR, then Flatpak, then Snap, and removes what it installed (C3); a removal says what else goes and who still uses it, with a Keep ([#49](https://github.com/jetomev/nog/issues/49)); `--yes` for scripts ([#32](https://github.com/jetomev/nog/issues/32)).

### v2.1.0 — unattended updates ([#56](https://github.com/jetomev/nog/issues/56), [#57](https://github.com/jetomev/nog/issues/57), [#58](https://github.com/jetomev/nog/issues/58), [#59](https://github.com/jetomev/nog/issues/59), [#60](https://github.com/jetomev/nog/issues/60), [#61](https://github.com/jetomev/nog/issues/61), [#62](https://github.com/jetomev/nog/issues/62), [#63](https://github.com/jetomev/nog/issues/63)) — built 2026-10-22/23, waits on Javier's run

- [x] `nog update --yes` (F-20 #56): the handoff answered, pacman's question too; Unknown packages kept back and listed, never decided; with nobody at the keyboard the AUR's build review waits for a person; a failed later step takes the default and stops
- [x] Nobody at the keyboard and no `--yes`: the cause said before anything changes (was "Cancelled")
- [x] Long AUR builds keep sudo awake (`--sudoloop`); a password nobody typed in time is named as the cause (F-21 #57, from a real update on 2026-10-22)
- [x] An update that leaves a step incomplete exits **3** and its runs line says **incomplete** (F-22 #58); the man page gains EXIT STATUS
- [x] `--yes` with nobody at the keyboard asks sudo up front, and says how to set it up if a password would be needed (F-23 #59)
- [x] Error messages say the exit status once (F-24 [#60](https://github.com/jetomev/nog/issues/60)): "exited with status 1", not "exited with status exit status: 1"
- [x] 285 → 304 tests (each new one seen failing, but one written with its code: the folder reader, proven on the live machine instead), warnings 6; the KognogOS VM: `scripts/vm-run-210.sh` 4/4 PASS on 2026-10-23 (Y1 nobody at the keyboard → Not updating, 147 pending unchanged; Y2 sudo would ask → Not updating, unchanged; Y3 a pacman-only sudo rule → 41 updated, the AUR kept back with a hint, exit 0; Y4 a failing flatpak step → "no (--yes takes the default)", exit 3, runs line incomplete); the desktop: `scripts/dry-run-210.sh` OK
- [x] **From Javier's run of rc1** (2026-10-23, 25/25): perl moved to a new version while apparmor and subversion, rebuilt for it, were still held — **interpreters now wait for the packages using their old module folder** (F-25 [#61](https://github.com/jetomev/nog/issues/61)); `nog history` printed a run's reboot advice upside down (F-26 [#62](https://github.com/jetomev/nog/issues/62))
- [x] From the VM, proving `--sudoloop`: when sudo had forgotten the password, yay's keep-awake loop asked for it in the middle of yay's own menu — **nog now asks sudo itself, alone, right before the helper starts** (F-27 [#63](https://github.com/jetomev/nog/issues/63))
- [x] Javier's answers: the record of the last completed update is **one for the whole computer** (`/var/lib/nog/last-update`); nog's own tier list **trimmed to generic packages** (PipeWire, WirePlumber, NetworkManager), the desktop and its apps being a distribution's choice
- [x] Javier's run of rc1: 25/25 ([testing/](testing/20261023%20-%20Test%20Matrix%20for%20nog%20v2-1-0.md))
- [ ] Javier's re-run on rc2 (§5 of the matrix); tag, GitHub Release, AUR after it

### v2.0.0 — the crown release: one tool for every source (C4 + C5 + C7: [#7](https://github.com/jetomev/nog/issues/7); [#5](https://github.com/jetomev/nog/issues/5)) — released 2026-10-16 (GitHub + AUR, after Javier's bench: 54/54)

- [x] `nog search` across every source (Flatpak and Snap rows, `--json` with `install_as`)
- [x] `nog list` with the installed Flatpak apps and snaps beside the packages (`remove_as`)
- [x] `info`, `reinstall`, `tier`, `lock`/`unlock`, `status`, `history`, each with `--json`
- [x] `nog clean` across every source: pacman's cache and orphans, the helper's build trees, unused Flatpak runtimes, old snap revisions; each category picked on its own; `--json`, `--only`, `--yes`
- [x] Tier ABI skew ([#5](https://github.com/jetomev/nog/issues/5)): the linchpin rule, keyed on a glibc or gcc-libs series change
- [x] Desktop dry runs (`scripts/dry-run-200.sh`, 41 cases), the KognogOS VM (`scripts/vm-run-200.sh`, `vm-run-200b.sh`); 259 → 285 tests, warnings 6
- [x] Javier's bench, 54/54 ([testing/](testing/20261015%20-%20Test%20Matrix%20for%20nog%20v2-0-0.md)); tag, GitHub Release, AUR (16 Oct)

**Validated 2026-09-29: nog works with paru** ([#12](https://github.com/jetomev/nog/issues/12)). With yay removed, `helper = "auto"` fell back to paru, a full update plan dated its AUR package correctly, and nog installed an AUR package through paru's build path. Record: [testing/](testing/20260929%20-%20Test%20Results%20for%20nog%20v1-5-1-paru.md).

### Later

- [ ] **Automatic dependency coupling** — read the exact-version dependencies out of the sync DB and hold those pairs together, rather than inferring them. An audit found 736 such pairs across the repos. v1.2.1 covers the ones that share a pkgbase, which is most of them; this would close the rest and let the version-cohort heuristic step back to handling only families that declare nothing at all. **The soname half of this shipped in v1.3.1** ([#13](https://github.com/jetomev/nog/issues/13)) — a Ready package that would stop providing a library something installed still needs is now held — and since v1.4.2 ([#16](https://github.com/jetomev/nog/issues/16)) "still needs" is read from the programs themselves, not only their declarations. What remains is the versioned `=` dependency case, where the declaration is exact rather than a soname.
- [ ] **First-run setup** — on your first `nog update`, ask whether Tier 1 should auto-release after 30 days or wait for your explicit approval each time.
- [ ] `nog rollback` — undo a recent update using pacman's cache
- [ ] A Chaotic-AUR binary package

### The v2 arc — one tool for every source ([design](docs/v2-design.md) · [tracking issue #7](https://github.com/jetomev/nog/issues/7))

- [x] **C1 · v1.1.0** — Flatpak
- [x] **C2 · v1.2.0** — Snap
- [x] **C3 · v1.8.0 → v1.9.0** — Install chain: pacman → AUR → Flatpak → Snap, always showing the source before installing. *The source shown and the AUR asked first in v1.8.0; Flatpak and Snap, the backend offers, and removal of what nog installed in v1.9.0.* *(Numbers moved up: v1.5.0 went to `nog install <file>`, #17, v1.6.0 and v1.7.0 to nogForge's needs.)*
- [x] **C4 · v2.0.0** — Full command surface plus `--json` output. *Its `--json` half shipped early, in v1.6.0: `list`, `search` and the update plan, for nogForge. The rest — `search` and `list` across every source, `info`, `reinstall`, `tier`, `lock`/`unlock`, `status`, `history` — in v2.0.0.*
- [x] **C5 · v2.0.0** — Maintenance and cleanup: orphans, AUR build caches, unused Flatpak runtimes, old Snap revisions. *Its first piece shipped early: `nog clean` for pacman's cache, v1.5.5 ([#15](https://github.com/jetomev/nog/issues/15)); the rest in v2.0.0.*
- [x] **C6** — nogForge, the visual companion, built on forgekit. *Its first beta, v0.3.0, shipped 2026-10-03 with nog v1.6.0; [v1.0.0](https://github.com/jetomev/nogforge/releases), its first stable release, on 2026-10-04 with nog v1.6.1 (Javier's call); v1.1.0 the same day runs nog inside its own window, on nog v1.7.0's steps. Flatpak and Snap installs and removals through nog since v1.9.0; its Flatpak and snap rows and its Maintenance page follow nog 2.0.0's `search`, `list` and `clean --json` ([nogforge](https://github.com/jetomev/nogforge)).*
- [x] **C7 · v2.0.0** — the crown release: the arc complete, the docs swept, the man page whole

*Every released version's roadmap lives in [docs/ROADMAP.md](docs/ROADMAP.md).*

---

## Changelog

### v2.1.0 — October 2026

**Unattended updates** ([#56](https://github.com/jetomev/nog/issues/56) · F-21 [#57](https://github.com/jetomev/nog/issues/57) · F-22 [#58](https://github.com/jetomev/nog/issues/58) · F-23 [#59](https://github.com/jetomev/nog/issues/59)). The KognogOS installer and welcome app, a timer or a script can now run an update with nobody at the keyboard, and know how it ended.

- **`nog update --yes`**: nog answers its own question and pacman's. The Ready packages go in. **Unknown** packages are kept back and listed: each has no usable build date, so updating it is a decision, and `--yes` never makes it (`yes | nog update` used to). With nobody at the keyboard, the AUR's build review waits for a person: those packages stay Ready, with a hint. When a later step fails, `--yes` takes the question's default and stops there.
- **Nobody at the keyboard, said as the cause**: without `--yes`, nog says so before anything changes, instead of reading end-of-input as "no" and printing "Cancelled". With `--yes`, nog first checks that sudo can run pacman without a password, and if not, says how to set that up.
- **Long AUR builds keep sudo awake** (`--sudoloop`, in yay and paru alike). On 2026-10-22 a seven-minute snapd build ended at a password prompt nobody was there to answer; now the password is asked once, when the step starts. When a password does time out, nog says that was the cause.
- **An unfinished update says so where programs read it**: exit status **3**, and **incomplete** in the day's runs log (it was 0 and "done", and nogForge showed a green "done"). The man page gains an EXIT STATUS section.

- **Error messages say the exit status once** (F-24 [#60](https://github.com/jetomev/nog/issues/60)): seven messages read "exited with status exit status: 1"; now "exited with status 1", or the signal that stopped the program.

- **perl and python wait for the packages built for their new version** (F-25 [#61](https://github.com/jetomev/nog/issues/61), found in Javier's run): on 2026-10-23 perl went 5.42 → 5.44 while apparmor and subversion, already rebuilt for 5.44, were held a day longer, so their perl modules sat where the new perl never looks. Arch says this only in the folder names, so nog now reads who has files in the interpreter's old folder, only when such a move is pending, and the interpreter waits with them (`coupled to apparmor`, or `blocked by` a package with no rebuild yet). A new rule in the coupling chain.
- **`nog history` keeps a run's lines in order** (F-26 [#62](https://github.com/jetomev/nog/issues/62)): newest run first, each run's own lines as written. A six-line reboot advice used to print upside down, and `nog status` showed its last line as the headline.
- **The password is asked alone, before an AUR build starts** (F-27 [#63](https://github.com/jetomev/nog/issues/63), found in the KognogOS VM): when a long pacman step outlasted sudo's memory of the password, yay's keep-awake loop asked for it in the middle of yay's own menu, two programs reading the keyboard at once. nog now runs `sudo -v` itself right before the helper: a fresh ticket asks nothing, an expired one asks on its own line, and the loop keeps it fresh after. If sudo is not given the password, the build does not start and nog says so.
- **One record of the last completed update for the whole computer**, `/var/lib/nog/last-update` (Javier's call): an update by any user, or through nogForge, counts for everyone. Written while sudo still remembers the password, so it never asks; where sudo may run pacman only (a timer's rule), the per-user record is the fallback.
- **nog's own tier list is generic** (Javier's call): Tier 2 holds PipeWire, WirePlumber and NetworkManager. The desktop and its apps (Plasma, SDDM, Firefox, Dolphin, Konsole, Kate, grubForge, Alacritty, fish, alacrittyForge) are a distribution's or your choice; KognogOS ships its own, fuller list. pacman keeps an edited `/etc/nog/tier-pins.toml` as it is.

Tests: 285 → 304 (each new one seen failing on purpose, except the folder reader's, written with its code and proven against this desktop's real file lists). Warnings unchanged at 6.

### v2.0.0 — October 2026

**The crown release: one tool for every source** ([#7](https://github.com/jetomev/nog/issues/7) C4 + C5 + C7 · [#5](https://github.com/jetomev/nog/issues/5)). Javier, 2026-10-14 night: *"I want all issues done, nothing open."* The v2 arc designed on 2026-08-10 is complete.

- **The command surface (C4)**: `nog search` looks in every source — the repositories, the AUR, Flatpak (`flatpak search`), Snap (`snap find`) — results per source with their tier, 30 per source on screen, all in `--json`, where a Flatpak or snap row carries `install_as`. `nog list` shows the installed Flatpak apps and snaps beside the packages (`remove_as`). **`nog info <name>`**: everything nog knows about one package, what is installed first. **`nog reinstall`**: pacman puts the files back, the helper rebuilds, `flatpak install --reinstall`; a snap cannot be reinstalled in place and nog says the way instead. **`nog tier <pkg>`** says the tier *and why* (pinned, the headers of a pinned kernel, the 32-bit half, a split-package sibling, a group member, the default); `--set` moves it. **`nog lock`/`nog unlock`**: a locked package is never updated until unlocked — held in the plan as *"locked by you"*, ignored by pacman, never promoted; `/etc/nog/locks.toml`, so every user and nogForge see the same locks. **`nog status`**: the dashboard. **`nog history`**: every run, every update decision, every reboot advice, from nog's own logs. Every query command has `--json`.
- **Maintenance across every source (C5)**: `nog clean` reports what each source leaves behind — pacman's old downloads (the v1.5.5 rule) *and its orphans*, the AUR helper's build trees, Flatpak runtimes no installed app uses (nog's estimate; flatpak decides the exact list), the old snap revisions snapd keeps — a numbered row per category with its size, then *"Clean up which? [all, numbers like 1 3, or n for none]"*. Each category is removed by the tool that owns it, in its own voice: pacman shows the orphans and asks, flatpak works out its exact list and removes it (your pick is the yes), nog removes the cache files (root) and the build trees (yours, no password), `snap remove --revision` one by one. `--only a,b` picks and still asks; `--yes` answers; `--json` is the report alone. The first report on the test desktop: 10 GB.
- **The linchpin rule (#5, from the 2026-08-04 spectacle breakage)**: when `glibc` or `gcc-libs` has a held update that changes its *series* (2.44 → 2.45, new `GLIBC_2.45` symbol versions; gcc-libs 15 → 16), every repository package in Ready that was built after the new build moves to Held with it — *"12 days · built after the new glibc, moves with it"* — because it was probably linked against it and would fail at runtime. A same-series rebuild (most months) moves nothing: measured on the test desktop on 2026-10-14, 0 of 160. The way out is to promote the linchpin, never to downgrade the fast lane. The ruling was Javier's to make; with *"nothing open"* it was made this way: the safe default, the promote as the door.
- **The man page is whole**: every command, every JSON document, the locks file, the linchpin rule, new examples.

Dry runs on the desktop ([scripts/dry-run-200.sh](scripts/dry-run-200.sh): 41 cases, every one stopping before a change — a read-only command, a refusal, an answer of no, sudo's window refused), then the KognogOS VM from the state the 1.9.0 run left ([scripts/vm-run-200.sh](scripts/vm-run-200.sh), [vm-run-200b.sh](scripts/vm-run-200b.sh): a lock seen by the plan and undone, a tier moved and back, reinstalls through pacman and flatpak, garbage made in every source and `nog clean` taking it). Javier's own run on the desktop (15–16 Oct, 22 steps, 54/54; three findings, fixed in rc2 before the release: F-17 [#53](https://github.com/jetomev/nog/issues/53) a lock with nothing pending now named below the tables, F-18 [#54](https://github.com/jetomev/nog/issues/54) flatpak does not ask for `uninstall --unused` and nog no longer says it does, F-19 [#55](https://github.com/jetomev/nog/issues/55) a *no* at pacman's question is a choice, not a failure): [testing/](testing/20261015%20-%20Test%20Matrix%20for%20nog%20v2-0-0.md).

Tests: 259 → 285. Warnings unchanged at 6.

*The complete history lives in [docs/CHANGELOG.md](docs/CHANGELOG.md).*

## Related Projects

- **[KognogOS](https://github.com/jetomev/KognogOS)** — the distribution nog was built for. Arch-based, tier-aware by default; its desktop is hypeForge, on Sway.
- **[forgekit](https://github.com/jetomev/forge-suite/tree/main/forgekit)** — the shared foundation every Forge app is built on.
- **[nogForge](https://github.com/jetomev/forge-suite/tree/main/nogforge)** — packages, the KognogOS way: nog's decisions in a terminal app (installed, install, updates with choices, maintenance, history).
- **[grubForge](https://github.com/jetomev/forge-suite/tree/main/grubforge)** — GRUB bootloader manager.
- **[alacrittyForge](https://github.com/jetomev/forge-suite/tree/main/alacrittyforge)** — Alacritty terminal configurator.
- **[bitlaForge](https://github.com/jetomev/forge-suite/tree/main/bitlaforge)** — solo Bitcoin mining, honestly framed.

All of them live in the [Forge Suite](https://github.com/jetomev/forge-suite) repository since 2026-10-22.

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
