# Test Results — nog v1.3.1 (#13: soname coupling)

**Machine:** tphome-linux (desktop Arch, KognogOS testing grounds)
**Date:** 2026-08-28, 09:30 PM – 10:10 PM
**Binaries compared:** `/usr/bin/nog` 1.3.0 (AUR release build) and
`~/Programs/nog/target/release/nog` 1.3.1

A hotfix cycle, so this is a results file rather than a matrix: one rule, and
the question is whether it fires when it should and stays silent when it
should not.

---

## §1 · Design validated **before** implementation

The rule was prototyped in Python and run against live system data before any
Rust was written — the same order used for v1.2.1, and for the same reason:
on 2026-08-25 an issue that read correctly would have shipped a fix that left
the failure intact.

| # | Check | Result |
|---|---|---|
| 1.1 | Local DB parses completely | ✅ **1397 of 1397** package directories — no silent skips |
| 1.2 | Replay the real failure from cached packages | ✅ fires on `libbluray` → `ffmpeg4.4` |
| 1.3 | Sweep: each pending update released alone | ✅ 161 candidates × 1397 installed, **0 false positives** |
| 1.4 | Sweep: all 161 released together (a plain `-Syu`) | ✅ **0** — as it must be; Arch's repositories are internally consistent |
| 1.5 | Is the sweep vacuous? | ⚠️ **nearly** — see below |

> **1.5 is the honest caveat.** Of 161 pending updates, 136 provide no soname
> at all, 24 provide sonames but drop none, and exactly **one** (`protobuf`,
> 35.1 → 36.0) drops any. So the sweep exercised the rule's *silence* far more
> than its judgement. `protobuf` was checked by hand and is a correct silence:
> nothing installed **declares** `libprotobuf.so=35.1.0-64`, verified against
> `pacman -Qi` independently of the prototype's own parser.
>
> That boundary is worth stating plainly. This rule catches **declared**
> soname dependencies — the thing that makes pacman refuse a transaction.
> Packages that link a library but declare a plain package-name dependency are
> invisible to it and would break at *runtime* instead. That is issue #5, a
> different bug.

---

## §2 · The false-positive mechanism, targeted directly

A clean sweep proves little on its own. The way this rule can go wrong is a
soname that exists at two versions at once, so that was measured rather than
assumed.

| # | Check | Result |
|---|---|---|
| 2.1 | Sonames with more than one provider | **0** of 694 — the `-32`/`-64` suffix makes every entry unique |
| 2.2 | Soname **bases** at 2+ versions simultaneously | **118** |
| 2.3 | …of those, same architecture (the hard case) | **11** |

The eleven, all live on this machine:

| Base | Coexisting providers |
|---|---|
| `libavcodec`, `libavdevice`, `libavfilter`, `libavformat`, `libavutil`, `libswresample`, `libswscale` | `ffmpeg4.4` (58/7/56/3/5) beside `ffmpeg-obs` (63/12/61/7/10) |
| `libcrypt.so` | `libxcrypt-compat` (=1) beside `libxcrypt` (=2) |
| `libmbedcrypto`, `libmbedtls`, `libmbedx509` | `mbedtls3` beside `mbedtls` |

**A rule comparing base names would couple all 129 of these and wedge the
update queue permanently.** Matching the whole string — architecture suffix
included — makes every one of them vanish. `ffmpeg4.4` and `ffmpeg-obs` were
both in the pending list at the time, so this was not hypothetical. All eleven
became negative unit tests.

---

## §3 · Unit tests

`86 → 100`. The fourteen new ones are `local_db` parsing (4) and the rule (10),
and every rule fixture is real data from this machine rather than an invention.

| Test | Asserts |
|---|---|
| `soname_rule_replays_the_libbluray_transaction_failure` | fires on the exact 2026-08-28 case |
| `soname_rule_ignores_same_base_at_different_versions` | `ffmpeg4.4` / `ffmpeg-obs` stay uncoupled |
| `soname_rule_ignores_the_32_and_64_bit_pair` | `libEGL.so=1-32` ≠ `libEGL.so=1-64` |
| `soname_rule_lets_a_pair_move_together` | the `nog install <both>` workaround is not undone |
| `soname_rule_accepts_a_surviving_second_provider` | another provider keeps the dependency satisfiable |
| `soname_rule_holds_for_a_foreign_package_with_no_update` | an AUR dependent still gets named |
| `soname_rule_prefers_a_held_partner_with_the_longest_wait` | the countdown shown is the one that gates release |
| `is_soname_accepts_exact_matches_and_rejects_ranges` | `libfoo.so>=3` is out of scope |
| `soname_rule_is_inert_without_a_local_database` | soft-fail leaves v1.3.0 behaviour |
| `a_block_with_no_end_date_says_so_instead_of_counting_down` | `blocked by X`, never `0 days` |

---

## §4 · Live A/B on byte-identical state

The decisive test. `libbluray` and `ffmpeg4.4` were downgraded from the pacman
cache to the exact pair that failed, both binaries were run against that state
fifteen seconds apart, and the pair was restored. Neither run could install:
stdin was closed, so each declined at its own gate.

| Binary | `libbluray` lands in | Note |
|---|---|---|
| **v1.3.0** | `READY TO INSTALL` | `hold just expired` → hands pacman a transaction it refuses |
| **v1.3.1** | `ON HOLD FROM INSTALL` | **`1 day · coupled to ffmpeg4.4`** |

v1.3.1's run ended `Nothing to install — every pending update is held.`
`ffmpeg4.4` kept its own `1 day remaining`, so the pair clears together on the
next run rather than one waiting on the other indefinitely.

Restoration was verified: `libbluray 1.5.0-1`, `ffmpeg4.4 4.4.8-5`.

---

## §5 · Regression and cost

| # | Check | Result |
|---|---|---|
| 5.1 | Existing couplings unaffected | ✅ `elfutils` / `libelf` still `1 day · coupled to lib32-libelf` |
| 5.2 | No new couplings on a consistent system | ✅ 161 pending, zero demotions from the new rule |
| 5.3 | Local DB read succeeds silently | ✅ no warning printed |
| 5.4 | Runtime cost | ✅ **4287 ms → 4282 ms** over 3 runs each — within noise; `checkupdates`' network sync dominates |
| 5.5 | `cargo test --release --locked` | ✅ 100/100 |
| 5.6 | Warning delta | ✅ 6 → 6 |
| 5.7 | No `CARGO_MANIFEST_DIR` in the release binary | ✅ |
| 5.8 | Version sync across seven surfaces | ✅ all `1.3.1` |

---

## Not tested, and why

| Check | Why |
|---|---|
| The `blocked by <pkg>` row in the field | Requires a foreign package depending on a soname a repo package is about to drop. Covered by unit test; no live instance existed on this machine. |
| Multi-provider survival path | **0** sonames on this system have more than one provider, so the branch is unit-tested only. It is defensive against a case Arch does not currently produce. |
| Fixpoint interaction with the other three rules | The new rule shares the existing loop, capped at 16 passes; no run this session took more than one pass. |

---

## Verdict

**Ships.** The rule fires on the failure it was written for, proven by A/B on
identical state; it is silent across 161 real candidates and correctly silent
on the one that could have tripped it; and the eleven same-architecture
coexistences that would break a naive implementation are all held as tests.

The honest limits: the live corpus contained exactly one soname drop, and the
rule sees only *declared* dependencies. Neither is a reason to hold the
release — the alternative is a transaction pacman refuses outright — but both
are reasons to keep watching it in the field.
