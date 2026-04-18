# 📦 nog

> A tier-aware package manager for Arch Linux — pacman with a safety net, written in Rust.

![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)
![Platform: Linux](https://img.shields.io/badge/Platform-Linux-lightgrey.svg)
![Base: Arch Linux](https://img.shields.io/badge/Base-Arch%20Linux-1793d1.svg)
![Language: Rust](https://img.shields.io/badge/Language-Rust-dea584.svg)
![Status: Alpha](https://img.shields.io/badge/Status-Alpha-orange.svg)
![Version: 0.6.0](https://img.shields.io/badge/Version-0.6.0-purple.svg)
[![AUR](https://img.shields.io/aur/version/nog)](https://aur.archlinux.org/packages/nog)

---

## Why nog?

Arch Linux is fast, current, and beautifully simple. But rolling releases treat every package the same — when an update is available, it gets installed. Your kernel and core libraries update automatically alongside a trivial icon theme. One bad kernel update and your machine doesn't boot.

There is no safety net. One bad sync and you're in single-user mode at 2 AM.

**nog exists to change that.**

nog is a thin, readable Rust wrapper around pacman that adds a single idea: **not all packages deserve equal urgency**. Every package on your system belongs to one of three tiers, and each tier has its own update rules. The kernel, bootloader, and glibc sit behind a hold. Your desktop environment gets a shorter hold. Everything else flows through as usual.

We believe managing your system should be:
- **Safe** — critical packages are never updated without your knowledge
- **Transparent** — nog is a pacman wrapper, not a replacement; no magic, no surprises
- **Familiar** — if you know pacman, you know nog; same commands, same flags, same mental model
- **Readable** — the whole source is a few hundred lines of Rust, deliberately simple

nog was born from a simple frustration: why does Arch give you everything except control over _which_ updates reach you and _when_? It doesn't have to be that way.

---

## Features

- 🎚 **Three-tier package classification** — every package is Tier 1, Tier 2, or Tier 3
- 🔒 **Tier 1 protection** — kernel, bootloader, glibc, systemd, mesa held from automatic updates
- ⏸ **Tier 2 awareness** — desktop environment and key applications flagged during installs
- ⚡ **Tier 3 fast track** — everything else flows through pacman unchanged
- 🎨 **Color-coded search** — every `nog search` result tagged with its tier
- 📌 **Persistent tier pinning** — `nog pin <pkg> --tier=<N>` writes to `/etc/nog/tier-pins.toml`
- 🔓 **Manual Tier 1 promotion** — explicit opt-in required with `nog unlock --promote`
- 🛡 **Pacman-native** — uses `pacman --ignore` for Tier 1 holds, no patching or shadowing
- 📖 **Man page included** — `man nog` for full reference

---

## The Three-Tier System

Every package nog manages falls into one of three tiers. Tier assignments live in `/etc/nog/tier-pins.toml` and can be adjusted at any time with `nog pin`.

### Tier 1 — Manual Sign-Off Required
The most critical packages on your system. These are **never updated automatically** — not even during a full system upgrade. To update a Tier 1 package you must explicitly unlock it first with `nog unlock <package> --promote`.

**Default Tier 1 packages:**
`linux`, `linux-zen`, `linux-lts`, `linux-hardened`, `systemd`, `systemd-libs`, `glibc`, `grub`, `efibootmgr`, `mkinitcpio`, `pacman`, `mesa`

### Tier 2 — 10-Day Hold (Notification)
Key desktop applications and system services. In v0.6.0 these pass through to pacman but are flagged during install so you know what's changing.

**Default Tier 2 packages:**
`plasma-meta`, `plasma-desktop`, `sddm`, `pipewire`, `pipewire-pulse`, `wireplumber`, `networkmanager`, `firefox`, `dolphin`, `konsole`, `kate`, `grubforge`, `alacritty`, `fish`, `alacrittyforge`

### Tier 3 — Fast Track
Everything else. No hold, no ceremony — updates flow through pacman on the next `nog update`.

---

## Requirements

- Arch Linux (or Arch-based distribution)
- `pacman` and `pacman-contrib`
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

```bash
# Install a package (respects tier rules)
sudo nog install <package>

# Update the system (Tier 1 packages automatically held)
sudo nog update

# Search with tier annotations
nog search <query>

# Pin a package to a specific tier
sudo nog pin <package> --tier=<1|2|3>

# Unlock a Tier 1 package for manual upgrade
sudo nog unlock <package> --promote

# Remove a package
sudo nog remove <package>

# Version
nog --version

# Help
nog --help
```

### How `nog update` works

When you run `sudo nog update`, nog:

1. Loads `/etc/nog/tier-pins.toml` and identifies all Tier 1 packages
2. Displays them clearly as `[HELD]` — they will not be touched
3. Passes the upgrade to `pacman -Syu` with Tier 1 packages excluded via `--ignore`
4. Tier 2 and Tier 3 packages update normally

### Example: `nog search`

```
extra/firefox 138.0-1 [Tier 2 — 10d hold]
    Fast, Private & Safe Web Browser
extra/linux-zen 6.19.10-1 [Tier 1 — manual sign-off]
    The Linux ZEN kernel
extra/htop 3.4.1-1 [installed] [Tier 3 — fast-track]
    Interactive process viewer
```

### Example: `nog update`

```
nog: checking tier holds before update...

  Tier 1 packages (held — manual sign-off required):
    [HELD] linux
    [HELD] linux-zen
    [HELD] systemd
    [HELD] glibc

nog: running update (Tier 1 packages excluded)...
:: Starting full system upgrade...
```

---

## Screenshots

*Terminal screenshots coming in v1.0*

---

## Configuration

nog reads two configuration files from `/etc/nog/`.

### `nog.conf`

General nog settings — version, logging, paths, and the hold durations for each tier.

```toml
[general]
version = "0.6.0"
log_level = "info"

[paths]
tier_pins = "/etc/nog/tier-pins.toml"
pacman_conf = "/etc/pacman.conf"
log_file = "/var/log/nog.log"

[holds]
tier1_days = 30
tier2_days = 15
tier3_days = 7
```

### `tier-pins.toml`

The tier assignment file. Anything not listed here falls into Tier 3 by default.

```toml
[tier1]
hold_days = 0
manual_signoff = true
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
hold_days = 10
manual_signoff = false
packages = [
    "plasma-desktop",
    "firefox",
    # ...
]

[tier3]
hold_days = 3
manual_signoff = false
# everything not listed above falls here automatically
```

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
|   |-- pacman.rs              # pacman subprocess wrapper
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
2. **Transparency** — Tier 1 and Tier 2 packages are always reported before a change is made
3. **Pacman-native enforcement** — Tier 1 holds use pacman's own `--ignore` mechanism, so there is no way for nog to silently bypass them

nog does not replace pacman. It does not patch pacman. It does not shadow pacman commands. It is a small, readable wrapper — you can read the entire source in an afternoon.

---

## Roadmap

### v0.6.0 — Current
- [x] CLI skeleton with all subcommands
- [x] Three-tier classification engine
- [x] Real pacman subprocess integration
- [x] `nog search` with color-coded tier annotations
- [x] System-wide install at `/usr/bin/nog`
- [x] `nog update` with Tier 1 excluded via `pacman --ignore`
- [x] `nog pin` with persistent tier changes to `tier-pins.toml`
- [x] AUR package
- [x] Man page

### v1.0 — Planned
- [ ] **Date-based hold system** — 30/15/7 day automatic holds replacing manual sign-off
- [ ] **Build date query** — read package release dates from pacman sync database
- [ ] **Selective update logic** — only pass packages whose hold has expired to pacman
- [ ] **AUR helper detection** — auto-detect `yay` or `paru` for AUR package queries
- [ ] **AUR package support** — classify and hold AUR packages using the detected helper
- [ ] **Chaotic-AUR verified** — confirm binary repo works natively (no special handling required)
- [ ] **Updated man page** — reflect new tier model and AUR support
- [ ] **Terminal screenshots** — add visual examples for all major commands

### Future
- [ ] Chaotic-AUR binary package (submit once v1.0 is stable)
- [ ] `nog history` — log of all tier changes and package actions
- [ ] `nog status` — dashboard showing what's held, what's ready, what's overdue
- [ ] `nog rollback` — revert a recent update using pacman cache
- [ ] Hook support for notifying a GUI companion like `nogforge`

---

## Changelog

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
