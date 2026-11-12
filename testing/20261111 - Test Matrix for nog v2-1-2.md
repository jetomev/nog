# Test Matrix for nog v2.1.2

*Built 2026-11-11, the evening Javier's run of 2.1.1 found F-31, so the fix is in his hands before the monthly update round (Saturday 2026-11-14, 9 AM, this desktop and kognogos-dev). §1–§2 are Claude's; §3 is Javier's, on this desktop, with the locally built AUR package; §4 is Claude's check after his install. The roll-up is `tally-matrix.py`'s, never a hand count.*

**What this version adds:** a Ready package whose new version brings in a package not installed yet, one carrying a library version a held package still owns, waits with that held package (`coupled to openssl`). Everything 2.1.1 added stays.

**Where it came from:** [the 2.1.1 sheet](20261110%20-%20Test%20Matrix%20for%20nog%20v2-1-1.md), row 3.3: `dotnet-runtime` (Ready) needs the new `openssl-3` compat package; pacman installed it beside the held openssl 3.6.5 and refused everything on `/usr/lib/libcrypto.so.3` and `/usr/lib/libssl.so.3`. Issue [#68](https://github.com/jetomev/nog/issues/68).

## §1 · Baseline sanity

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 1.1 | `cargo test --release --locked` | green | **PASS** | 319 passed, 0 failed, 6 ignored (313 at v2.1.1); the end-to-end dotnet test seen failing with the rule switched off |
| 1.2 | Warning delta vs v2.1.1 | unchanged | **PASS** | 6 → 6 (`generated 6 warnings`) |
| 1.3 | `grep -rn "TODO\|FIXME" src/` | empty | **PASS** | |
| 1.4 | `strings target/release/nog \| grep CARGO_MANIFEST_DIR` | empty | **PASS** | |
| 1.5 | `man --warnings -l nog.1` | no warnings | **PASS** | the new section included |
| 1.6 | Version sync | 2.1.2 in Cargo.toml, Cargo.lock, nog.conf, nog.1, README | **PASS** | |

## §2 · Claude, this desktop — dry run (nothing changes)

`scripts/dry-run-212.sh`: the installed 2.1.1 and the new build each print `nog update --json` from the same copy of the hold record, in a temporary home; then, without nog, the script works out from the package databases what pacman would install new for the Ready set. Log: `logs/dry-run-212-latest.log`.

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 2.1 | R1: the seven | curl, git, krb5, libssh2, openssh, sudo, qca-qt6 stay Held, coupled to openssl | **PASS** | |
| 2.2 | R2: the dotnet pair | dotnet-runtime and dotnet-host: Ready with 2.1.1, Held with 2.1.2 | **PASS** | dotnet-runtime coupled to openssl, dotnet-host coupled to dotnet-runtime (same recipe), 17 days |
| 2.3 | R3: the cost, by name | every package moved listed; none moved from Held to Ready | **PASS** | 2 moved (Ready 182 → 180, Held 173 → 175) |
| 2.4 | R4: pacman's view | nothing new for the Ready set takes a held package's files | **PASS** | "nothing new". The check itself seen failing on 2.1.1's list: `openssl-3 for dotnet-runtime: takes files of held ['openssl']` |

## §3 · Javier, this desktop — step by step

**Resume here → [the 2.1.3 sheet](20261111%20-%20Test%20Matrix%20for%20nog%20v2-1-3.md).** Javier's run of 2.1.2 (2026-11-11 20:34) found F-32 (#69): node and zathura's PDF plugin lost their library. 2.1.2 stays on GitHub only and never goes to the AUR; the fix is 2.1.3, and its sheet carries the install and retry steps.

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | `nog install ~/Programs/aur-nog/build-212/nog-2.1.2-1-x86_64.pkg.tar.zst` | asks his password, then only nog changes (2.1.1 → 2.1.2); no `.sig` question | **PASS** | 20:34, `20261111-203423 install nog-2.1.2-1-x86_64.pkg.tar.zst.log` |
| 3.2 | `nog --version` | `nog 2.1.2` | **PASS** | his run's banner reads `nog v2.1.2` |
| 3.3 | `nog update`, answer **n** at "Begin the handoff?" | the Held table has dotnet-runtime `coupled to openssl` and dotnet-host `coupled to dotnet-runtime`, beside openssl and its set; answering n changes nothing | **N/A** | he went straight to the real run (3.4); its tables show both, held as written |
| 3.4 | `nog update`, answer **y** (his call when: tonight, or Saturday's round) | pacman's step finishes; no `conflicting files`; the Ready packages installed, the Held ones untouched | **FAIL** | 20:34–20:42, exit 0, no conflicting files, openssl still 3.6.5, openssl-3 not installed. But simdjson (Ready, declares no sonames) took `libsimdjson.so.34` from held nodejs, and libmupdf took `libmupdf.so.28.5` from held zathura-pdf-mupdf: `node` would not start → **F-32** (`logs/damage-20261111-2034-run.log`) |

## §4 · Claude, after Javier's install

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 4.1 | `NOG_NEW=/usr/bin/nog NOG_OLD=/usr/bin/nog scripts/dry-run-212.sh` (before 3.4) | the installed nog holds the seven and the dotnet pair; R4 nothing new; OK | **N/A** | 3.4 ran before it; 4.2 reads the run itself |
| 4.2 | after 3.4: his run's log | `nog: … done`, exit 0; openssl still 3.6.5, openssl-3 not installed | **PASS** | `Done · update`, exit 0; `openssl-3.6.5-1` installed, no `openssl-3`, dotnet-runtime still 10.0.13 |

## Findings

- **F-32 ([#69](https://github.com/jetomev/nog/issues/69))**, row 3.4: simdjson 1:5.0.5 → 2:5.1.1 and libmupdf 1.28.5 → 1.28.6, Ready, declare no sonames, so the soname rule (#13) never saw `libsimdjson.so.34` and `libmupdf.so.28.5` go; their users nodejs and zathura-pdf-mupdf (rebuilt for the new versions) were held. A full check of every program against every removed library file found exactly those two. Fixed in **2.1.3** (`holds::undeclared_soname_demotions`); 2.1.2 never goes to the AUR.
