# Test Matrix — nog v2.1.1 (the reverse soname rule: F-30 #67)

*Built 2026-11-10, the evening the gap was found, so the fix is in Javier's hands before the monthly update round (Saturday 2026-11-14, 9 AM, this desktop and kognogos-dev). §1–§2 are Claude's; §3 is Javier's, on this desktop, with the locally built AUR package; §4 is Claude's check after his install. The roll-up is `tally-matrix.py`'s, never a hand count.*

**What this version adds:** a Ready package whose new version needs a library that only a held package will bring now waits with that package (`coupled to openssl`), instead of making pacman refuse the whole update. Rebuilds that link the new library without declaring it wait too.

**Where it came from:** `nog update --json` on this desktop, 2026-11-10 (read only): openssl 3.6.5 → 4.1.0 held 18 days (coupled to kmod), while curl, git, krb5, libssh2, openssh, sudo and qca-qt6, rebuilt for OpenSSL 4, sat in Ready. curl, sudo and openssh declare `libcrypto.so=4-64`; pacman would have refused everything.

## §1 · Baseline sanity

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 1.1 | `cargo test --release --locked` | green | **PASS** | 313 passed, 0 failed, 6 ignored (304 at v2.1.0); the two end-to-end tests seen failing with the rule switched off |
| 1.2 | Warning delta vs v2.1.0 | unchanged | **PASS** | 6 → 6 (`generated 6 warnings`) |
| 1.3 | `grep -rn "TODO\|FIXME" src/` | empty | **PASS** | |
| 1.4 | `strings target/release/nog \| grep CARGO_MANIFEST_DIR` | empty | **PASS** | |
| 1.5 | `man --warnings -l nog.1` | no warnings | **PASS** | the new section included |
| 1.6 | Version sync | 2.1.1 in Cargo.toml, Cargo.lock, nog.conf, nog.1, README | **PASS** | |

## §2 · Claude, this desktop — dry run (nothing changes)

`scripts/dry-run-211.sh`: the installed 2.1.0 and the new build each print `nog update --json` from the same copy of the hold record, in a temporary home. Log: `logs/dry-run-211-latest.log`.

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 2.1 | R1: the seven | curl, git, krb5, libssh2, openssh, sudo, qca-qt6: Ready with 2.1.0, Held and `coupled to openssl` with 2.1.1 | **PASS** | |
| 2.2 | R2: the cost, by name | every package moved listed; none moved from Held to Ready | **PASS** | 62 moved (Ready 237 → 175, Held 100 → 162), all coupled to openssl or a package coupled to it. nvidia-utils is among them, probably needlessly: it ships NVIDIA's own OpenSSL 3 plugin. Harmless this month: linux-zen is held the same 18 days |

## §3 · Javier, this desktop — step by step

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | `nog install ./nog-2.1.1-1-x86_64.pkg.tar.zst` (in `~/Programs/aur-nog-remote/`) | only nog changes; no `.sig` question | | |
| 3.2 | `nog --version` | `nog 2.1.1` | | |
| 3.3 | `nog update`, answer **n** at "Begin the handoff?" | the Held table has openssl, curl, git, sudo, openssh… each reading `18 days · coupled to openssl` (openssl itself `coupled to kmod`); answering n changes nothing | | |

## §4 · Claude, after Javier's install

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 4.1 | `NOG_NEW=/usr/bin/nog scripts/dry-run-211.sh` | the installed nog holds the seven, coupled to openssl; OK | | |

## Findings

*(none yet)*
