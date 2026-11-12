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

**Resume here (written 2026-11-11).** The package is built by makepkg from the published GitHub release in `~/Programs/aur-nog/build-212/` (the tests ran inside the build; its checksum and signature checked). Nothing else waits on Claude before this. Hand Javier one step at a time, in plain words: what it does, the one command, what he should see, then wait for his pasted output. Deadline: before the monthly round, **Saturday 2026-11-14, 9 AM**.

1. Install: `nog install ~/Programs/aur-nog/build-212/nog-2.1.2-1-x86_64.pkg.tar.zst` — nog asks his password (pacman underneath) and swaps 2.1.1 for 2.1.2, nothing else.
2. Version: `nog --version` → `nog 2.1.2`.
3. Look first: `nog update`, answer **n** at "Begin the handoff?". dotnet-runtime and dotnet-host are now in the Held table.
4. The real retry, when he says go (tonight, or Saturday's round is this step): `nog update`, answer **y**. pacman's step runs to the end with no "conflicting files".
5. Then Claude runs §4, fills the Result columns, tallies with `tally-matrix.py`.
6. Only on his pass: the AUR push of **2.1.2-1** from `~/Programs/aur-nog-remote/` (one push: the 2.1.1 commit there is folded into it, never pushed; ssh agent per `~/.claude/rules/aur.md`; pre-flight pkgdesc ≤ 255 bytes) → close #67 and #68 with the story → kognogos-dev gets 2.1.2 before its round (his command, prepared by Claude) → kognogos.org card + Vault note.

A finding during his test goes into the same unpublished 2.1.2 (rebuild, re-test), never a second AUR push.

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | `nog install ~/Programs/aur-nog/build-212/nog-2.1.2-1-x86_64.pkg.tar.zst` | asks his password, then only nog changes (2.1.1 → 2.1.2); no `.sig` question | | |
| 3.2 | `nog --version` | `nog 2.1.2` | | |
| 3.3 | `nog update`, answer **n** at "Begin the handoff?" | the Held table has dotnet-runtime `coupled to openssl` and dotnet-host `coupled to dotnet-runtime`, beside openssl and its set; answering n changes nothing | | |
| 3.4 | `nog update`, answer **y** (his call when: tonight, or Saturday's round) | pacman's step finishes; no `conflicting files`; the Ready packages installed, the Held ones untouched | | |

## §4 · Claude, after Javier's install

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 4.1 | `NOG_NEW=/usr/bin/nog NOG_OLD=/usr/bin/nog scripts/dry-run-212.sh` (before 3.4) | the installed nog holds the seven and the dotnet pair; R4 nothing new; OK | | |
| 4.2 | after 3.4: his run's log | `nog: … done`, exit 0; openssl still 3.6.5, openssl-3 not installed | | |

## Findings

*(none yet)*
