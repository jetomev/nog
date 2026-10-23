# nog — project rules

*How this project is built, tested and shipped. Written for the AI co-developer, and public on purpose: it is part of how the human + AI method is documented. The general working rules (release order, AUR hygiene, the update checklist) live in `~/.claude/CLAUDE.md` and `~/.claude/rules/`; this file holds only what is nog's own.*

## What it is
A tier-aware package manager for Arch Linux, written in Rust, and the package updater of KognogOS. It wraps **pacman**, an **AUR helper** (yay or paru, which do the AUR builds; nog never builds itself), **Flatpak** and **Snap** (each switched off quietly when not installed). Every package has a tier, and each tier's updates wait a set number of days so a bad update gets noticed before it lands: Tier 1 (kernel, bootloader, glibc, systemd) 30 days, Tier 2 (desktop and key apps) 15, Tier 3 (everything else) 7. `nog update` sorts every pending update into **Ready / Held / Unknown** before anything runs. v2.0.0 (16 Oct 2026) completed the v2 arc (#7): one tool for every source, `--json` on every query, `nog clean` across every source.

## Where things live
- **Code:** `src/commands/mod.rs` is the engine (install, remove, keep, update); `surface.rs` the query commands; `maintain.rs` `nog clean`. The rules themselves are pure functions: `tiers.rs` (which tier, and why), `holds.rs` (hold windows, coupling rules, linchpin, the foreign fence), `sightings.rs` (when an update was first seen). Each module opens with a comment saying why it exists; read it before changing the module.
- **System files** (written through `sudo tee`, so every user and nogForge see the same): `/etc/nog/nog.conf` (hold days, safety waits, AUR helper, cache keeps), `/etc/nog/tier-pins.toml` (which packages are Tier 1/2, plus `[groups]`), `/etc/nog/locks.toml` (`nog lock`), `/etc/nog/sources.toml` (`nog deactivate/activate <source>`). nog never rewrites the hand-edited `nog.conf`; its own state goes in its own files.
- **User files** (nog runs as the user): CSV logs and whole-run `.log` files in `~/.local/share/nog/logs`, the hold record in `~/.local/state/nog/holds.tsv`.
- **Defaults:** `config/nog.conf` and `config/tier-pins.toml` are what the package installs (marked `backup=`, so pacman keeps a user's edits). **KognogOS ships its own, much fuller `tier-pins.toml`** from the KognogOS repository into its ISO. Distro choices go there, never into nog's default; nog stays usable on any Arch system. The tests read `config/tier-pins.toml`, and the man page's TIER SYSTEM section must list it, so a change there touches both.

## Correctness rules (each one cost a broken update to learn)
Packages that must move together are held together. Every rule below is a coupling rule; all of them run in `holds::coupling_demotions` and repeat until nothing changes (a fixpoint, #11). **A new rule joins that chain**, never a one-off pass: a single pass missed `elfutils` after `libelf` moved.
- **Kernel and headers** (v1.0.3): `<kernel>-headers` takes its kernel's Tier 1. Headers ahead of the kernel broke the NVIDIA module build and two of three screens.
- **Same pkgbase** (v1.0.4): packages built from one recipe (the pipewire family) move as one, highest tier wins. A split family made pacman refuse the whole update.
- **`lib32-<X>` with `<X>`** (v1.0.6, #1): released together as one hold unit, **even in Tier 3**. `lib32-nvidia-utils` went ahead of `nvidia-utils` and the transaction aborted.
- **Sonames, matched as WHOLE strings, architecture suffix included** (v1.3.1, #13): a Ready package that drops a shared library a Held or installed package still needs waits for it. Matching on the base name would couple 129 pairs that legitimately coexist (`ffmpeg4.4` beside `ffmpeg-obs`, `-32` beside `-64`) and wedge the queue forever; those pairs are negative tests. A foreign package in the way reads `blocked by <pkg>`. Unreadable local database = the rule stays off, never guesses.
- **Real linking** (v1.4.2, #16): what a program declares is not enough; `elf.rs` reads what each binary actually loads.
- **Linchpins** (v2.0.0, #5): when glibc or gcc-libs changes series, Ready packages built after the new one wait with it. A same-series rebuild moves nothing.
- **A hold is counted from the first new version seen, plus a safety wait** (v1.5.2, #27). Counting from the newest build kept linux-zen held from May to the end of September.
- **The foreign fence:** AUR packages nog did not clear this run stay `--ignore`d; when the AUR cannot be checked, nothing foreign moves, and the summary says `could not check`, never 0 (#25).
- **The user's intent is the scope:** `nog install` installs only what was asked, never an update first (F-8 #41); `nog update a b` touches only those (#44); nothing is promoted past a hold unless named.
- **The AUR always needs a person:** no terminal = refuse up front, `--yes` never covers an AUR build (#26, #32); `nog update --yes` with nobody at the keyboard keeps the AUR back with a hint (F-20 #56). Every helper build gets `--sudoloop`, so a long build never ends at a password nobody is there to type (F-21 #57). A broken or unreadable state file is said out loud, never guessed (`sources.toml` garbage = every source off).
- **Say what was seen:** reboot advice checks the machine where it can and names the package where it cannot, never an anonymous warning.

## The contract with nogForge
nogForge shows what nog decides and never decides itself, so nog's machine output is a public contract:
- `--json`: **exactly one JSON document on stdout**, everything else on stderr, no banner, no run-history line. nogForge reads `install_as`, `remove_as`, `coupled_to`, and `nog clean --json` categories (`id`, `items`, `bytes`, `names`, `how`).
- `NOG_EVENTS=<file>`: one JSON line per step (`steps`, `step`, `ask`, `end`), in nog's own words. `NOG_ASKPASS=1`: every sudo gets `-A`, the helper gets `--sudoflags -A`.
- **Exit status** (v2.1.0, F-22 #58): 0 done, 1 stopped, 2 a command line not understood, **3 `nog update` ran but a later step did not complete** (the runs log says `incomplete`). Programs read it; nogForge's run window shows anything but 0 as stopped, with nog's own screen open.
- A field is never renamed or removed silently: change nogForge in step, raise its `nog>=` requirement, and update the man page (it documents every JSON document).

## How to build and test
- `cargo test --release --locked`; **report the count every time** (294 passing + 5 ignored at v2.1.0; warnings 6). Read warnings from cargo's own `generated N warnings` line, not a grep count.
- **Never run a real install, removal, update or clean on the development desktop to test.** Unit tests feed pure functions strings and sample files; tests that read the live machine are `#[ignore]` and change nothing. Desktop checks go through `scripts/dry-run-*.sh`: the fresh binary, a throwaway HOME, sudo refused at once, every case stopping before a change; they log to `logs/`.
- **Real changes happen in the KognogOS test VM** (`kognog-hypeforge`), from a named snapshot: `scripts/vm-put.py` copies the binary in, `vm-exec.py` runs a command, `vm-drive.py` answers prompts as they appear (typed-ahead answers get thrown away by sudo and pacman). The AUR recipe is built in a clean Arch chroot.
- A release candidate is `scripts/make-rc-package.sh N` → `dist-rc/`, built with the AUR recipe and every test; Javier installs that and runs the written matrix.
- The release binary must not carry the build path: the `config/` fallback exists in debug builds only (`strings target/release/nog | grep CARGO_MANIFEST_DIR` must be empty).

## Releases and records (nog's own parts)
- `testing/RELEASE-CHECKLIST.md` lists every surface that carries the version (`Cargo.toml`, `Cargo.lock`, `config/nog.conf`, `nog.1`, README badge and example). Where its install steps differ from the umbrella release order, the umbrella wins: Javier tests the locally built package before any AUR push.
- Roll up a matrix only with `python3 testing/tally-matrix.py "<matrix>"`; hand counts were wrong twice. Every FAIL row has a numbered finding.
- **No `PKGBUILD` in this repository** (removed in v1.4.0; it drifted from the AUR copy). Packaging lives in the AUR repository alone. `docs/PACKAGING.md` lists what the package needs; a new crate or a new program nog calls updates it in the same commit.
- The man page `nog.1` lists every command in `src/main.rs`; a command change updates it in the same commit. The README's sample run is a real capture: never edit its version by hand.
- Findings are `F-n`, numbered on from the last one (TODO.md / the issues), each an issue opened and closed with the full story. Commit prefixes: `fix(<area>): … (F-n)`, `todo: …`, `test: …`, `docs: README + man page + version bump for vX.Y.Z`.
- Signatures: tags signed; release assets carry `.asc`; a package `.sig` never sits beside the `.pkg.tar.zst` Javier installs (pacman then demands the key).

## Names and words
`nog` (always lowercase), `nogForge`, KognogOS, the Forge Suite. Buckets: Ready, Held, Unknown. Verbs: promote, keep, lock, clean. Sources: the repositories, the AUR, Flatpak, Snap. Messages follow the house style (blank line around every notice, the heading says what happened, small actions show only what changes). Credit what nog builds on; never compare nog with other tools.
