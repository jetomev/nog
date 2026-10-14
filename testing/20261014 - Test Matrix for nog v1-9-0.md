# Test Matrix — nog v1.9.0 (C3 complete #7 · F-13 #49 · #32)

**Why:** Javier, 2026-10-14 morning: *"work on open issues first, then implement Flatpak and Snap … have ready a detailed test bench, to go step-by-step, test-by-test."* Three things in one version: the install chain whole (the repositories, then the AUR, then Flatpak, then Snap; what nog installs, nog removes), a removal that says what else goes and who still uses it with a Keep and `nog keep` (F-13), and `--yes` for scripts with "nothing is at the keyboard" said plainly (#32).
**Where:** §2 this desktop, dry runs with the built program (`scripts/dry-run-190.sh`: every case stops before a password). §3 the KognogOS VM `kognog-hypeforge`, reset to `clean-install-3` (the previous state kept as snapshot `before-nog-190`), nothing set up: no yay, no flatpak, no snapd; rc1 installed with pacman; prompts answered by `scripts/vm-drive.py`. §4 **Javier, on this desktop**, with the rc1 package.
**Who:** §1–§3 Claude (14 Oct, 07:44–). §4 Javier, when he is back.

## §1 · Baseline sanity

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 1.1 | `cargo test --release --locked` | green | **PASS** | 258 passed, 0 failed, 5 ignored (242 at v1.8.0) |
| 1.2 | Warning delta vs v1.8.0 | unchanged | **PASS** | 6 → 6 (`generated 6 warnings`) |
| 1.3 | `grep -rn "TODO\|FIXME\|XXX" src/` | empty | **PASS** | |
| 1.4 | `strings target/release/nog \| grep -i CARGO_MANIFEST_DIR` | empty | **PASS** | |
| 1.5 | `man --warnings -l nog.1` | no warnings | **PASS** | |
| 1.6 | Version sync | 1.9.0 in Cargo.toml, Cargo.lock, nog.conf, nog.1, README | **PASS** | |
| 1.7 | rc1 package | built from the AUR recipe, tests in the build | **PASS** | `dist-rc/nog-1.9.0rc1-1-x86_64.pkg.tar.zst`, 258 tests in `check()` |

## §2 · Claude, this desktop — dry runs (nothing changes; `logs/dry-run-190-latest.log`)

This desktop has flatpak (Flatseal, Termius), snapd (hello) and yay, so the whole chain can be *looked up* here; every case stops before a password (sudo's window refused).

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 2.1 | `nog install zzz-nope` | "not found in the repositories, the AUR, Flatpak or Snap", stops first | **PASS** | A1 |
| 2.2 | `nog install wonderpen` (no terminal) | table: `Flatpak — flathub, as com.tominlab.wonderpen`, Tier 3; then "Nothing is at the keyboard…" | **PASS** | A2; the AUR has no wonderpen, so the chain fell through to Flatpak |
| 2.3 | `nog install hello-world`, answer n | table: `Snap — sandboxed`; nog's own "Install these? [Y/n]"; Cancelled | **PASS** | A3; nog asks because `snap install` never does |
| 2.4 | `nog install gimp` | table: extra (the chain stops at the first source that has it) | **PASS** | A4 |
| 2.5 | `nog install flatpak/org.gimp.GIMP` | table: `Flatpak — flathub` (the ID typed, so not repeated) | **PASS** | A5 |
| 2.6 | `nog install calculator` | "5 Flatpak apps answer to 'calculator'", the list, `nog install flatpak/org.gnome.Calculator`; stops | **PASS** | A6 |
| 2.7 | `nog install snap/hello`, answer n | table `Snap — sandboxed`; nog asks; Cancelled | **PASS** | A7 |
| 2.8 | `nog install --yes aur/neofetch` | stops: "--yes does not answer the AUR … yay stops to let you review each build recipe" | **PASS** | A8 |
| 2.9 | `nog install --yes cowsay` | "Handing off to pacman with your yes (--yes)"; sudo refused → "pacman stopped" | **PASS** | A9; `--yes` goes to pacman directly, not yay |
| 2.10 | `nog install cowsay flatpak/wonderpen snap/hello-world`, answer n | one table, three sources; nog asks (a snap is in it); Cancelled | **PASS** | A10 |
| 2.11 | `nog install snap/zzz-nope` · `flatpak/zzz-nope` | "not found in Snap" · "not found in Flatpak" (that source only) | **PASS** | A11, A12 (after the wording fix) |
| 2.12 | `nog install ./file.pkg.tar.zst` (no terminal) | "Nothing is at the keyboard…" | **PASS** | A13 |
| 2.13 | `nog remove zzz-nope` | "not installed on this computer: not a package, not a Flatpak, not a snap" | **PASS** | B1 |
| 2.14 | `nog remove flatseal`, answer n | "Removing com.github.tchx84.Flatseal" (found by its name); flatpak's own question; "Not removed" | **PASS** | B2 |
| 2.15 | `nog remove hello`, answer n | "Snaps: `snap remove` never asks, so nog asks here"; "Remove hello? [y/N]"; "Cancelled — the snaps stay" | **PASS** | B3 |
| 2.16 | `nog remove iwd`, answer n to Keep | **"ell goes too, but bluez-utils uses it (optional: for btpclient)"**; "Keep ell? [Y/n]"; then pacman (sudo refused) | **PASS** | B4 — F-13 in the failing direction, on real data |
| 2.17 | `nog remove --yes iwd` (no terminal) | "Keeping ell (--yes keeps what another app still uses)"; marking (sudo refused) → "Nothing was removed" | **PASS** | B5 |
| 2.18 | `nog remove iwd` (no terminal, no --yes) | "Nothing is at the keyboard…" before anything | **PASS** | B6 |
| 2.19 | `nog keep zzz-nope` | "not installed, so there is nothing to keep" | **PASS** | C1 |
| 2.20 | `nog keep vlc-plugin-ffmpeg` | "Keeping …"; sudo refused → "Not kept … you cancelled the password" | **PASS** | C2 |
| 2.21 | `NOG_EVENTS` on every case | steps `install` / `flatpak` / `snap` / `remove` / `keep`, nog's questions as `ask`, the end status | **PASS** | every case's events line in the log |

## §3 · Claude, the KognogOS VM — real installs from nothing

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | `nog install --yes cowsay` from a shell with no terminal (as javier) | pacman answered, cowsay in, exit 0, no question shown | **PASS** | V1; `logs/vm-190-V1.log` |
| 3.2 | `nog install wonderpen` with **no flatpak installed** | "Set up Flatpak and look there? [y/N]" → y; pacman asks → y; "Flatpak is set up. Looking there."; table (Flatpak — flathub, as com.tominlab.wonderpen); the handoff line; flatpak's own questions → y; installed | **PASS** | V2: the offer, pacman's install of flatpak and the table in `logs/vm-190-V2a-offer.log`; the install itself in `vm-190-V2b.log` (flatpak asks twice: the runtime, then the changes) |
| 3.3 | `nog remove wonderpen` | found as a Flatpak; flatpak asks → y; gone | **PASS** | V3; `flatpak list --app` empty after; `logs/vm-190-V3-V8-V6-V7.log` |
| 3.4 | `nog install hello-world` with **no snapd installed** (yay present) | repos no, AUR no, Flatpak no → "Set up Snap and look there? [y/N]" → y; yay builds snapd; socket started; `/snap` linked; table (Snap — sandboxed); "Install these?" → y; installed; `hello-world` runs | | V4 — the longest: a Go build |
| 3.5 | `nog remove hello-world` | nog asks → y; gone | | V5 |
| 3.6 | `nog remove iwd` (bluez-utils installed) | "ell goes too, but bluez-utils uses it"; Keep → y; `pacman -Qi ell` says *Explicitly installed*; iwd gone, ell still there | **PASS** | V6 — F-13 end to end: *ell: install reason has been set to 'explicitly installed'*, then pacman's list showed iwd alone; `pacman -Qi ell` → Explicitly installed; `pacman -Q iwd` → not found |
| 3.7 | `nog keep cowsay` then `nog remove --yes cowsay` | kept = explicit; the removal still removes cowsay itself (keep protects a package from going *along*, not from being named) | **PASS** | V7; no question under --yes; cowsay gone |
| 3.8 | `nog install --yes aur/neofetch` | stops before anything: --yes does not answer the AUR | **PASS** | V8; the table shown (AUR — built by yay), then the stop, exit 1 |
| 3.9 | `nog install cowsay flatpak/wonderpen snap/hello-world`, y to all | three transactions in the chain's order, each tool's own voice, all three in; then `nog remove` of all three | | V9 |

## §4 · Javier, this desktop — step by step

Your desktop already has flatpak, snapd and yay, so no set-up offers here (those were proven in the VM, §3). Every step: type the one line, compare with "You should see", then tell me **PASS** or paste what differed. The password window is sudo's (KDE); answer it when it comes.

| # | Type | You should see | Result | Notes |
|---|---|---|---|---|
| 4.1 | `nog install ~/Programs/nog/dist-rc/nog-1.9.0rc1-1-x86_64.pkg.tar.zst` | the usual package-file install; only nog changes; then `nog --version` says **1.9.0** | | |
| 4.2 | `nog install --help` | the options list has `--yes` with one sentence on what it answers (and that it never answers the AUR) | | |
| 4.3 | `nog install zzz-nope` | **Not installing** — "zzz-nope was not found in the repositories, the AUR, Flatpak or Snap"; nothing asked | | the whole chain named |
| 4.4 | `nog install calculator` | **Not installing** — "5 Flatpak apps answer to 'calculator'", the five listed, and the way out: `nog install flatpak/org.gnome.Calculator` | | nog lists, never guesses |
| 4.5 | `nog install wonderpen` | the table: `wonderpen · 3.4.0 · Flatpak — flathub, as com.tominlab.wonderpen · Tier 3`; "Handing off to flatpak: it shows what comes with it (the runtimes it needs) and asks"; flatpak's own questions — first "Required runtime … Do you want to install it? [Y/n]" if the runtime is new to this computer, then its table and "Proceed with these changes…? [Y/n]" → **y** to both; it downloads and installs; **Done** | | WonderPen is on Flathub only (a notes app); its runtime (freedesktop 26.08) may be a few hundred MB |
| 4.6 | `flatpak list --app` | `com.tominlab.wonderpen` in the list | | |
| 4.7 | `nog remove wonderpen` | **Removing com.tominlab.wonderpen** (found by its name); flatpak's own question → **y**; gone; **Done** | | what nog installs, nog removes |
| 4.8 | `nog install hello-world` | the table: `hello-world · 6.4 · Snap — sandboxed · Tier 3`; **nog asks "Install these? [Y/n]"** (snap never asks) → **y**; "Handing off to snap: it installs without asking — nog asked above"; sudo's window; snap's progress; **Done** | | `hello-world` is Canonical's test snap, 2 MB |
| 4.9 | `hello-world` | prints `Hello World!` | | the snap works |
| 4.10 | `nog remove hello-world` | **Removing hello-world** — "Snaps: `snap remove` never asks, so nog asks here"; **"Remove hello-world? [y/N]"** → **y**; sudo's window; gone; **Done** | | |
| 4.11 | `nog install --yes aur/neofetch` | **Not installing** — "--yes does not answer the AUR, and neofetch would be built from it … Run this in a terminal: nog install aur/neofetch"; nothing built | | the AUR's review stays yours |
| 4.12 | `nog install --yes cowsay` | the table (extra); "Handing off to pacman with your yes (--yes): … installs without asking"; sudo's window; **no "Proceed?" question**; cowsay in; **Done** | | for scripts and nogForge |
| 4.13 | `nog remove --yes cowsay` | "pacman shows the list and, with your yes (--yes), removes without asking"; gone; **Done** | | |
| 4.14 | `bash -c 'nog install cowsay' < /dev/null` | **Not installing** — "Nothing is at the keyboard to answer the question asked before anything changes … Run this in a terminal, or add --yes" | | the 7 Oct case (#32): the cause, not "you answered no" |
| 4.15 | `nog remove iwd` | **Going along, but still used** — "ell goes too, but bluez-utils uses it (optional: for btpclient)"; "Kept means marked as installed on purpose…"; **"Keep ell? [Y/n]"** → **Enter** (yes); "Marking ell as installed on purpose"; sudo's window; then **Removing iwd** and pacman's list **without ell** → answer **n** (keep iwd: it is your Wi-Fi daemon); "Not removed … Nothing was changed by this removal" | | F-13 on real data. Side effect of the yes: ell is now "explicitly installed" (`pacman -Qi ell`); harmless, `sudo pacman -D --asdeps ell` undoes it if you care |
| 4.16 | `nog keep vlc-plugin-ffmpeg` | **Keeping vlc-plugin-ffmpeg** — "Marked as installed on purpose, so removing another package never takes it along"; sudo's window; pacman's one line; **Done** | | the 8 Oct workaround, now a nog command |
| 4.17 | `nog keep zzz-nope` | **Not keeping** — "zzz-nope is not installed, so there is nothing to keep" | | |
| 4.18 | `nog install cowsay flatpak/wonderpen snap/hello-world` | one table with the three sources; nog asks once (a snap is in it) → **y**; then **three handoffs in order**: pacman (asks → y), flatpak (asks → y), snap (sudo); all three in; **Done** | | the chain in one request. Then `nog remove cowsay wonderpen hello-world` → pacman asks, flatpak asks, nog asks for the snap; all gone |
| 4.19 | `nog update` | exactly as before: nothing in this release touches the update | | sanity |
| 4.20 | nogForge → Search → install a repository package, and Uninstall one | as before (nogForge drives `nog install`/`remove` in its own window; `--yes` is not used yet) | | sanity; nogForge's Flatpak/Snap rows come with C4's search |

## Known, not in this release (own issues, C4)

- `nog search` and `nog list` show the repositories and the AUR only; Flatpak and Snap rows (and their `--json`) come with C4, and only then can nogForge offer a Flatpak or a snap in its Search.
- nogForge's Uninstall does not yet show F-13's "goes along, but still used" rows ([nogforge#27](https://github.com/jetomev/nogforge/issues/27)), it will once it calls the new nog.
- `--yes` is a flag for scripts; nogForge keeps asking in its window.
- #5 (Tier ABI skew) is not built: measured on this desktop on 14 Oct, the proposed rule would co-hold **0** of 160 pending packages, and today's held glibc is a same-series update (2.44 → 2.44) where the breakage cannot happen. The numbers and a question for Javier's ruling are in the issue.
