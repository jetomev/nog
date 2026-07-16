# nog — v1.0.6 dogfood test results

Focused hotfix for [issue #1](https://github.com/jetomev/nog/issues/1) — `lib32-<X>` released from hold while its base `<X>` stayed held, producing an unresolvable pacman transaction. No separate Test Matrix for this cycle; the regression surface is the four new `holds::tests` coupling cases (unit tests 29 → 33), which run in the AUR build's `check()` on every install. This doc records the live end-to-end dogfood.

## Run metadata

- **Package under test:** `nog 1.0.6-1`, built + installed via `makepkg -si` from the AUR clone, then confirmed against the pushed AUR package
- **AUR source:** `aur.archlinux.org/nog.git` commit `61316e7 nog 1.0.6 — lib32/base hold coupling (#1)`
- **GitHub source:** `jetomev/nog` tag `v1.0.6` → commit `5ce7d3b docs: version bump + changelog for v1.0.6 (#1)`
- **Tarball sha256:** `136f3c00bbe97fa9591a43cb93eda3aa6da7122adc29ca77612b5baf4a2924be` (verified by makepkg from the GitHub tag tarball)
- **`check()` phase:** `cargo test --release --locked` → **33 passed, 0 failed** during the AUR build
- **Test run:** 2026-07-16
- **Tester:** Javier (`jetomev`) + Claude (Anthropic), per [[nog-development-discipline]] and [[github-surface-completeness]]
- **Trigger:** v1.0.6 couples version-locked `lib32-<X>`/base `<X>` pairs at hold-release time. This dogfood validates the fix on the *same host that hit the original abort* (the nvidia stack), with the exact split still pending.

## The live proof — the split pair the bug is about

The originating failure: on 2026-07-14, `nog update` (then v1.0.5) put `lib32-nvidia-utils` in **Ready** ("hold just expired") while `nvidia-utils` stayed **Held**. Handing off to pacman then aborted the *entire* transaction:

```
warning: cannot resolve "nvidia-utils=610.43.03", a dependency of "lib32-nvidia-utils"
:: The following package cannot be upgraded due to unresolvable dependencies:
      lib32-nvidia-utils
error: failed to prepare transaction (could not satisfy dependencies)
```

The `lib32-nvidia-utils` upgrade was skipped that day, so the split was **still pending** when v1.0.6 was installed — a natural live reproduction.

**After — `nog 1.0.6-1`, 2026-07-16 ~7:29 PM.** `lib32-nvidia-utils` no longer appears in Ready. It is now the last line of the **Held** bucket:

```
Held (194):
  ...
  nvidia-open-dkms 610.43.02-3 -> 610.43.03-3   [Tier 3 · 3 days remaining]
  nvidia-utils 610.43.02-3 -> 610.43.03-3       [Tier 3 · 3 days remaining]
  ...
  lib32-nvidia-utils 610.43.02-1 -> 610.43.03-1 [Tier 3 · coupled to nvidia-utils · 3 days]
```

### What this verifies

| Assertion | Evidence | Result |
|---|---|---|
| **Split pair demoted** | `lib32-nvidia-utils` left Ready, entered Held with the `coupled to nvidia-utils` reason | ✅ |
| **Countdown inherited** | Coupled row shows `· 3 days`, matching `nvidia-utils`' own `3 days remaining` — pair clears together | ✅ |
| **Genuinely withheld** (not just relabeled) | yay handoff logged `lib32-nvidia-utils: ignoring package upgrade (610.43.02-1 => 610.43.03-1)` — it made the ignore list | ✅ |
| **No abort** | Transaction resolved and installed 16 packages cleanly; the `cannot resolve "nvidia-utils=..."` error did not occur | ✅ |
| **No false positives** | Non-split lib32 pairs present the same run — `lib32-fontconfig`/`fontconfig`, `lib32-libffi`/`libffi`, `lib32-libssh2`/`libssh2`, `lib32-p11-kit`/`p11-kit` — all stayed together in **Ready** and installed normally | ✅ |

The non-split cases are the important negative control: coupling only fires on an actual Ready/Held split, so the four `lib32-*` packages whose bases were *also* Ready were correctly left alone. This matches the `no_coupling_when_pair_not_split` unit test on real data.

## Verdict

**No findings.** The reported abort is resolved, the fix is withheld-correct (not cosmetic), and it produces no collateral on non-split pairs. Issue #1 closed with this evidence. The broader depends/provides-graph generalization remains noted for a future cycle; the shipped name-pattern coupling fully covers the reported failure.
