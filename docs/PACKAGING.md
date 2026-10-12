# nog — what the package needs

*Measured from the code on 2026-10-11 (Javier: "check all Forge Suite apps for all dependencies needed to build and
run them"), with the Forge Suite's audit, `forge-suite/scripts/needs-audit.py`, and proven by building the
AUR recipe in a clean Arch system (devtools' `makechrootpkg`, tests included). Kept current by running the audit at every release.
Arch's `base` packages (bash, coreutils, glibc, sed, grep, util-linux, systemd…) are on every Arch install and
aren't listed.*

## Needs (the recipe's `depends`)

| Package | What uses it | Module / program |
|---|---|---|
| pacman | everything goes through pacman | (program) |
| pacman-contrib | update checks (checkupdates) | (program) |
| sudo | every change runs through sudo (with -A for the password window) | (program sudo) |
| zstd | linked on purpose: the system libzstd (Cargo.toml's zstd-sys pkg-config feature) | library libzstd.so.1 |
| fakeroot | building AUR packages (makepkg); KognogOS#11 | (through the AUR helper) |

## Optional (`optdepends`)

| Package | What uses it | Module / program |
|---|---|---|
| yay | the AUR (one of the two) | (program) |
| paru | the AUR (one of the two) | (program) |
| flatpak | Flatpak updates and installs (switched off when missing) | (program) |
| snapd | Snap updates and installs (switched off when missing) | (program snap) |

## To build (`makedepends`)

| Package | Why |
|---|---|
| rust | cargo build --release --locked |
| cargo | the same (Arch's rust provides it) |

## The AUR recipe

AUR 1.8.0-1 lacks **sudo** (not in Arch's base: some use doas; nog would install and fail at the first change), **zstd** (linked directly), and the optional **flatpak** and **snapd**. A recipe-only change: 1.8.0-2.
