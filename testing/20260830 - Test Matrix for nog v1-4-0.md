# Test Matrix — nog v1.4.0

**Date:** 2026-08-30 · **Release:** v1.4.0 (reboot advice, [#9](https://github.com/jetomev/nog/issues/9))
**Binary under test:** `target/release/nog` at `1.4.0`, plus the installed AUR binary at step 10 of the checklist.

Run against [RELEASE-CHECKLIST.md](RELEASE-CHECKLIST.md). Results are recorded as
they fell. A verdict the matrix cannot reach is written as **CANNOT TEST** rather
than passed — the reboot notice needs a real reboot-critical upgrade to land, and
this machine did not produce one during the release.

---

## §1 · Baseline sanity

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 1.1 | `nog --version` | `nog 1.4.0` | **PASS** | |
| 1.2 | `nog --help` | exit 0, commands listed | **PASS** | |
| 1.3 | `strings target/release/nog \| grep -i CARGO_MANIFEST_DIR` | empty | **PASS** | F2 regression guard from v1.0.2 still holds |
| 1.4 | `cargo test --release --locked` | green | **PASS** | 128 passed, 0 failed, 2 ignored |
| 1.5 | Warning delta vs v1.3.1 | unchanged | **PASS** | 6 → 6, none from `reboot.rs` |
| 1.6 | `grep -rn "TODO\|FIXME\|XXX" src/` | empty | **PASS** | |

## §2 · Version sync

| # | Surface | Expected | Result | Notes |
|---|---|---|---|---|
| 2.1 | `Cargo.toml` | 1.4.0 | **PASS** | |
| 2.2 | `Cargo.lock` | 1.4.0 | **PASS** | `--locked` refused the first test run until the lock was refreshed — the gate worked |
| 2.3 | `config/nog.conf` | 1.4.0 | **FAIL → fixed** | Still 1.3.1. See **F-1** |
| 2.4 | `nog.1` `.TH` | nog v1.4.0 | **PASS** | |
| 2.5 | README badge | 1.4.0 | **PASS** | |
| 2.6 | README nog.conf example | 1.4.0 | **PASS** | |
| 2.7 | Tag message references version | v1.4.0 | **PASS** | |
| 2.8 | README sample-run transcript | shows v1.3.0 | **N/A** | Correct as-is: a captured transcript legitimately shows the version it was captured under. Editing it would fabricate a record. Noted in the checklist |

## §3 · Reboot advice — logic

Driven by constructed probes, so every branch is reachable without a real upgrade.
28 unit tests in `src/reboot.rs`.

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | Tier 1 package classified | Reboot | **PASS** | |
| 3.2 | `*-headers` classified | nothing | **PASS** | Never loaded into a running system; present beside every kernel |
| 3.3 | NVIDIA and `*-dkms` classified | Reboot | **PASS** | Tier 1 does not cover them; they are the case that produced #9 |
| 3.4 | `mesa`, `xorg-server`, `wayland`, `dbus` | Session | **PASS** | |
| 3.5 | Ordinary package | nothing | **PASS** | `libnvidia-container` must not match as NVIDIA |
| 3.6 | NVIDIA banner parsed, open module | version found | **PASS** | |
| 3.7 | NVIDIA banner parsed, proprietary module | version found | **PASS** | Field positions differ between builds; parsed by shape |
| 3.8 | NVIDIA parse on unrecognised text | None | **PASS** | |
| 3.9 | systemd version parsed | `261.2-1` from `systemd 261 (261.2-1-arch)` | **PASS** | |
| 3.10 | systemd parse without parentheses | None | **PASS** | |
| 3.11 | Distro suffix dropped, pkgrel kept | `261.2-1-arch` → `261.2-1` | **PASS** | |
| 3.12 | pkgrel dropped for NVIDIA compare | `610.57.04-1` → `610.57.04` | **PASS** | The running driver never reports a pkgrel |

## §4 · Reboot advice — silence

The point of the feature. A notice that fires on every run is one nobody reads,
which is how the original twenty minutes were lost.

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 4.1 | Kernel updated, running kernel still installed | silent | **PASS** | `/usr/lib/modules/<uname -r>` present |
| 4.2 | NVIDIA updated, loaded module already matches | silent | **PASS** | |
| 4.3 | Package cleared by nog, declined at pacman's prompt | silent | **PASS** | Installed versions are re-read after the handoff, not assumed from the request |
| 4.4 | Package pacman does not know at all | silent | **PASS** | |
| 4.5 | Ordinary run, nothing relevant | silent, no probing | **PASS** | `render(&[])` is empty; no probe runs when no candidate was handed off |

## §5 · Reboot advice — findings

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 5.1 | Running kernel no longer installed | verified, both versions | **PASS** | |
| 5.2 | The original 2026-08-10 incident replayed | verified, `610.43.03` and `610.57.04` both shown | **PASS** | The line a user would have seen instead of twenty minutes of blaming a game |
| 5.3 | systemd behind what is installed | verified | **PASS** | |
| 5.4 | Unprobeable package (`mkinitcpio`) | announced, says "advice, not a finding" | **PASS** | Must not claim a finding it did not observe |
| 5.5 | NVIDIA with no module loaded | announced, not claimed | **PASS** | |
| 5.6 | Kernel probe unavailable | announced, not claimed | **PASS** | |
| 5.7 | Banner wording | exactly the line #9 specified | **PASS** | |
| 5.8 | Verb agreement | "glibc was" / "glibc and mkinitcpio were" | **PASS** | |
| 5.9 | Session-only update | log-out notice, never "reboot the system" | **PASS** | |
| 5.10 | Reboot and session advice together | both blocks, not merged | **PASS** | |

## §6 · Live probes on this machine

Read from the real system, not constructed. Diagnostic, not a regression test —
its result depends on the box.

| # | Check | Observed | Result |
|---|---|---|---|
| 6.1 | `uname -r` | `7.0.5-zen1-1-zen` | **PASS** |
| 6.2 | Running kernel's module directory | present | **PASS** |
| 6.3 | `/proc/driver/nvidia/version` | `610.57.04` | **PASS** |
| 6.4 | `systemctl --version` | `261.2-1` | **PASS** |
| 6.5 | `pacman -Q` for candidates | answers | **PASS** |
| 6.6 | Verdict on this machine right now | all four match → nog stays silent | **PASS** | 

## §7 · End to end

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 7.1 | A real `nog update` installing a reboot-critical package prints the notice | banner appears after the handoff | **CANNOT TEST** | No reboot-critical upgrade was pending during this release. Every component is verified independently — classification, probes, assembly, rendering, and placement in `update()` — but the assembled chain firing on a real transaction is **unproven**, and this entry exists so nobody reads §3–§6 as if it were |
| 7.2 | Notice does not appear on an ordinary update | silent | **CANNOT TEST** | Same reason. The unit path is 4.5 |

## §8 · Packaging and docs

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 8.1 | Root `PKGBUILD` absent from `HEAD` | absent | **PASS** | |
| 8.2 | Man page renders without warnings | clean | **PASS** | `man --warnings` silent |
| 8.3 | New `REBOOT ADVICE` man section | present, both bases documented | **PASS** | |
| 8.4 | In-repo README links resolve | all resolve | **FAIL → fixed** | `docs/CHANGELOG.md` was not linked at all. See **F-2** |
| 8.5 | README Project Structure accurate | lists every module, real counts | **FAIL → fixed** | Claimed 54 tests against 128, and omitted `local_db.rs` from v1.3.1. Fixed in the docs commit |
| 8.6 | Changelog leads with the newest release | v1.4.0 first | **PASS** | v1.3.0 pushed down to `docs/`, per the locked convention |

---

## Roll-up

**44 checks · 40 PASS · 2 FAIL (both fixed before publication) · 1 N/A · 2 CANNOT TEST**

Nothing here was fixed by relaxing an expectation. The two failures are recorded
as failures because that is what they were when the matrix found them.

---

## Findings

| # | Check | Severity | Finding |
|---|---|---|---|
| **F-1** | 2.3 | medium | `config/nog.conf` shipped the default `[general] version = "1.3.1"` while every other surface read 1.4.0. A user installing v1.4.0 would have received a config file claiming to be the previous release. Caught by the checklist's own version-sync gate — the gate exists because this exact class of miss has happened before, and it earned its keep again. Fixed before publication. |
| **F-2** | 8.4 | medium | `docs/CHANGELOG.md` was unreachable from the README. The Roadmap section carries a pointer to `docs/ROADMAP.md`; the Changelog section never had the equivalent. The locked convention pushes the third-oldest release out of the README on every release, so every release since the convention landed has been quietly moving history into a file no reader could navigate to. Made concrete by this release pushing v1.3.0 out. Fixed. |

## Method notes

| # | Note |
|---|---|
| **M-1** | **The checklist's warning-count grep over-reports by one.** `cargo build --release 2>&1 \| grep "^warning:" \| wc -l` counts cargo's own `generated N warnings` footer alongside the warnings themselves, so it read 7 for an actual 6. A release that treated that as a delta would have chased a warning that was never there. The checklist now reads cargo's summary line instead of counting. |
| **M-2** | **The checklist listed a file that no longer exists.** Its version-sync gate required the in-tree `PKGBUILD`, deleted in this release. A gate naming a missing file either fails forever or gets skipped, and skipped is worse. Removed, with a note recording why the file must not return. |
| **M-3** | **A staged deletion landed in the wrong commit.** `git rm --cached PKGBUILD` was run at the moment the decision was made, several steps before the commit it belonged to — so it was already in the index when the *feature* commit was made, and rode along with it. The result was a docs commit whose message described a deletion its diff did not contain. Caught before pushing by reading the staged file list rather than trusting the intent; both commits were rebuilt from saved messages. **Stage a deletion when it belongs to a commit, not when the decision is taken.** |
| **M-4** | **`--locked` caught the stale `Cargo.lock`.** Bumping `Cargo.toml` leaves the lock recording the old version, and `cargo test --release --locked` refused to run rather than silently updating it. Worth stating because the refusal reads like a tooling failure and is actually the gate doing its job. |
