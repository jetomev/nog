# Test Matrix — nog v1.3.0 (#10: one package manager per source)

**Covers:** v1.2.1 (family coupling, #11) and v1.3.0 (handoff split, #10).
Both were built on 2026-08-25; v1.2.1 shipped that evening without a matrix, so
its checks are folded in here and marked **[carried]**.

**Machine:** tphome-linux (desktop Arch, KognogOS testing grounds)
**Binary under test:** `~/Programs/nog/target/debug/nog` unless stated
**Precondition for §2–§4:** at least one package Ready. On 2026-08-25 evening
everything was held, so the handoff was unreachable — see §0.

**Run 1 — 2026-08-26, 07:01 PM.** 171 pending (169 official + 2 AUR), 9 Ready,
162 Held. Evidence: `~/.local/share/nog/logs/20260826 nog-update.csv`
(second run block), `/var/log/pacman.log` 19:02:02–19:02:54, and the terminal
transcript. §5, §6.4–6.5 and most of §8 were **not reachable** in this run and
are carried to Run 2.

**Run 2 — 2026-08-28, 07:06 PM – 09:17 PM.** The session that closed the file.
239 pending at the start, 78 Ready. §5 was reached in full, §6.4–6.5 and §8.5–8.6
with it, and three findings came out of it — one of them a transaction-killing
bug nobody had filed (F-5). Binary under test carried the F-3/F-4 fixes from
commits `348f093` and `2f85878`.

§2, §3 and §4 were re-exercised on the fixed binary during the same session:
76 official packages upgraded in one pacman step, `snapd` 2.76-1 → 2.76.2-2 built
and installed by yay in its own step, flatpak and snap silently dormant. All
Run 1 results held.

---

## §0 · Preconditions

| # | Check | Expected | Result |
|---|---|---|---|
| 0.1 | `checkupdates \| wc -l` | > 0 | ✅ 171 pending |
| 0.2 | At least one official package past its window | Ready is non-empty | ✅ 8 official Ready |
| 0.3 | At least one **AUR** package past its window | `fresh-editor-bin` or `snapd` Ready | ✅ `fresh-editor-bin` 0.4.9→0.4.10 |
| 0.4 | `nog --version` | `1.3.0` | ✅ |

> **0.3 is what makes §3 meaningful.** Without a cleared AUR package the helper
> step is skipped and never exercised. If it cannot be met, §3 must be recorded
> as **not run** rather than passed by inspection.

---

## §1 · Family coupling — #11 **[carried from v1.2.1]**

| # | Check | Expected | Result |
|---|---|---|---|
| 1.1 | pkgbase pair in one bucket | `elfutils`, `libelf`, `lib32-libelf` all Held | ✅ all three Held |
| 1.2 | Coupled rows name their partner | `coupled to lib32-libelf` | ✅ on `elfutils` and `libelf` |
| 1.3 | Coupled rows share one countdown | identical days on all three | ✅ 3 days on all three |
| 1.4 | Uniform cohort untouched | the ~67 nerd fonts all Held, none marked coupled | ✅ 67 held, 0 coupled |
| 1.5 | Uniform cohort untouched | the ~34 vlc packages all Held, none marked coupled | ✅ 34 held, 0 coupled |
| 1.6 | No over-firing | a Ready package with no held cohort sibling stays Ready | ✅ all 9 Ready stayed Ready |
| 1.7 | Convergence | no "coupling did not converge" message ever printed | ✅ never printed |

**§1 verdict: 7/7.** The cohort rule fired on the one real family and stayed
silent on the two large decoys, in the field, on data it had never seen.

---

## §2 · Step 1 — pacman

| # | Check | Expected | Result |
|---|---|---|---|
| 2.1 | Step announced by name | `Handing off official packages to pacman ...` | ✅ |
| 2.2 | pacman runs, not the helper | no `yay` banner before the pacman transaction | ✅ |
| 2.3 | Held packages ignored | `warning: <pkg>: ignoring package upgrade` for held ones | ✅ |
| 2.4 | **No duplicate narration** | each held package announced **once**, not by both tools | ✅ **pacman only; yay printed zero ignore lines** |
| 2.5 | chaotic-aur handled here | any chaotic package upgrades in the pacman step | ⚪ **not run** — no chaotic package was Ready (all 8 were `extra/`) |
| 2.6 | Ready packages install | every Ready official package lands | ✅ 8/8 upgraded |

> 2.4 is the headline of this release. Before the split, the 08-25 run printed
> ~140 `-> pkg: ignoring package upgrade` lines from yay **and** ~140
> `warning: pkg: ignoring package upgrade` lines from pacman — the same list
> twice. Tonight: one list, from pacman, and yay's section opens straight on
> `AUR Explicit (1): fresh-editor-bin-0.4.10-1`.

---

## §3 · Step 2 — the AUR helper

| # | Check | Expected | Result |
|---|---|---|---|
| 3.1 | Step announced with a count | `Handing off N AUR package(s) to yay ...` | ✅ `1 AUR package(s)` |
| 3.2 | Count matches the plan | N = Ready AUR + approved Unknown AUR | ✅ 1 Ready AUR, 0 Unknown |
| 3.3 | **Only cleared names passed** | held AUR packages appear nowhere in yay's output | ✅ `snapd` (held AUR) never mentioned |
| 3.4 | No second sysupgrade | yay does **not** re-list official packages | ✅ yay's transaction was 1 package |
| 3.5 | Cleared AUR package installs | the package actually upgrades | ✅ `fresh-editor-bin` 0.4.10-1 |
| 3.6 | Skipped when empty | with no cleared AUR package, yay is never invoked | ⚪ not applicable this run |

---

## §4 · Steps 3–4 — flatpak and snap

| # | Check | Expected | Result |
|---|---|---|---|
| 4.1 | No flatpaks installed → silent | no flatpak step, no error | ✅ |
| 4.2 | No cleared snaps → silent | no snap step, no error | ✅ (`snapd` itself was held) |
| 4.3 | Order preserved | pacman → helper → flatpak → snap | ✅ |

---

## §5 · Failure handling

**All seven pass.** §5.1 was proven with a *real* failure — a locked pacman
database, the same thing a second package manager leaves behind — not a
simulation. §5.2–§5.7 used a PATH shim that answers `yay -S` with exit 7 and
passes every query (`-Qua`, `-Sai`) through to the real yay, so detection,
build dates, tiers, the fence and the pacman step were all genuine; only the
install verb was intercepted. Exit code 7 was chosen to be unmistakable, and it
appears unaltered in every message and every log row: nog reports the helper's
status, it does not invent one.

| # | Check | Expected | Result |
|---|---|---|---|
| 5.1 | **pacman fails → cancel** | alert names the status; no other source runs | ✅ **real failure**, `status 1`, yay never invoked, `snapd` untouched |
| 5.2 | helper fails → ask | prompt `Continue with the remaining sources? [y/N]` | ✅ printed after `the yay step exited with status 7.` |
| 5.3 | Default is no | bare Enter stops the run | ✅ blank line stopped the run |
| 5.4 | EOF stops | closed stdin → `no input — stopping here.` | ✅ verbatim |
| 5.5 | Answering no | log says the aur step did not complete | ✅ `cancelled after the aur step did not complete (status 7)` × 326 rows |
| 5.6 | Answering yes | later sources still run | ⚠️ **partial** — the run carried through and completed, but flatpak and snap had nothing pending, so neither was invoked either way. The carry-through is proven; "the later steps still run" is not. |
| 5.7 | Summary printed | `Update finished, with N …` + the list | ✅ `Update finished, with 1 step(s) that did not complete.` / `Incomplete: aur (status 7)` |

> **How §5.1 was reached.** The plan written after Run 1 was to decline pacman's
> own `Proceed with installation?` prompt. That failed three times running — see
> F-4 — and was abandoned for `touch /var/lib/pacman/db.lck`, which makes pacman
> exit 1 before it prints anything or asks anything. One prompt in the whole run,
> nothing installable by accident, and a more realistic cause than a user decline.

---

## §6 · Fence and holds still intact

| # | Check | Expected | Result |
|---|---|---|---|
| 6.1 | Fence message reworded | `held back as a dependency` | ✅ |
| 6.2 | Fence still populated | count matches uncleared foreign packages | ✅ 20, exact |
| 6.3 | Held AUR package cannot move | absent from the helper's argument list | ✅ |
| 6.4 | AUR deactivated | helper never invoked | ✅ **stronger than written** — the `reported by yay` line disappears entirely; the switch gates *detection*, not just the handoff |
| 6.5 | Reactivate | restores step 2 | ✅ `1 AUR update(s) reported by yay.` returned; `sources.toml` back to four `true` |

---

## §7 · Report, log, and docs

| # | Check | Expected | Result |
|---|---|---|---|
| 7.1 | Per-source counts match execution | counts describe what ran | ✅ |
| 7.2 | CSV written | dated file under `~/.local/share/nog/logs/` | ✅ `20260828 nog-update.csv` |
| 7.3 | Outcome column accurate | one of the documented values | ✅ eight distinct outcomes exercised in one day-file |
| 7.4 | Man page matches behaviour | four-step description is what happened | ✅ |
| 7.5 | README sample matches | example resembles the real run | ✅ |
| 7.6 | Version sync | binary, man, badge, PKGBUILD all `1.3.0` | ✅ binary/man/badge; PKGBUILD bumps at release |

**Every outcome string nog can write was produced on 2026-08-28**, in one file:

| Rows | Outcome |
|---|---|
| 637 | `cancelled` |
| 482 | `pacman handoff did not complete (status 1)` |
| 326 | `cancelled after the aur step did not complete (status 7)` |
| 239 | `installed` |
| 163 | `installed with incomplete steps: aur (status 7)` |
| 162 | `all held` |

---

## §8 · Regression slice from v1.2.0

| # | Check | Expected | Result |
|---|---|---|---|
| 8.1 | Unknown prompt still works | per-package `[y/N]` | ⚪ not run — UNKNOWN was empty in every run, both nights |
| 8.2 | Kernel/headers desync warning | unchanged | ⚪ not run — `linux-zen` and its headers stayed in step |
| 8.3 | `--realign` | unchanged | ⚪ not run |
| 8.4 | `nog search` tier colours | unchanged | ✅ |
| 8.5 | `nog install` | routes through the helper | ✅ tier check printed for both packages, one transaction, clean |
| 8.6 | All-held early exit | `Nothing to install — every pending update is held.` | ✅ verbatim, outcome `all held` |

---

## Field observations (not checks)

**F-1 · v1.2.1 died on a transient mirror timeout; v1.3.0 walked through it.**
The 07:00 PM run on 08-26 in the same log file is the *installed* v1.2.1. Its
handoff — `yay -Syu` — reached yay's own `pacman -S -y`, which could not fetch
`chaotic-aur.db` (10 s timeout). yay aborted, and nog logged a failure against
all 171 rows: **nothing was installed.** Forty-two seconds later v1.3.0's
`pacman -Syu` hit the *same* timeout, treated one unreachable third-party DB as
non-fatal, and completed all 8 upgrades. Unplanned, and the strongest evidence
for #10 in the file: the split handoff removed a failure mode nobody had filed.

**F-2 · pacman.log misrepresents nog's ignore list.** nog passes one
comma-joined `--ignore`; pacman splits that string in place inside `argv` and
only then writes its `Running '...'` line, so the log records one name where 162
were passed. Verified by `--print`. Anyone auditing pacman.log after an incident
will conclude nog held exactly one package. Worth a troubleshooting note; not a
nog defect.

**F-3 · "failed" is the wrong word for a user cancel. FIXED (`348f093`).**
pacman exits 1 both when the user declines its prompt and when something
genuinely breaks; the status cannot distinguish them. Javier's ruling was not to
guess but to **say it could be either**. The terminal now prints *"That is either
a declined prompt or a pacman error — the exit status alone cannot tell the two
apart"*, and the run log — permanent, read long after the terminal is gone — says
`did not complete` rather than `failed`. Applied to the aur, flatpak and snap
steps too. A follow-on: the carried-through outcome briefly read
`installed, incomplete steps: …`, whose comma forced RFC-4180 quoting on a column
that had never needed it. Escaping was correct, but a quoted field trips naive
parsers — it tripped one during this very session — so it became
`installed with incomplete steps: …` (`2f85878`).

**F-4 · nog's gate was word-for-word pacman's. FIXED (`348f093`).** nog asked
`Proceed with installation? [Y/n]`; pacman asks `:: Proceed with installation?
[Y/n]` seconds later. The two are distinguishable only by a `nog:` prefix versus
`::`. During this session the tool's own author, holding a table that spelled out
which prompt was which, answered nog's gate as pacman's **three times in a row**.
The v1.0.7 double-confirm only buys safety if the user can tell the layers apart.
The gate now asks **`Begin the handoff?`**, borrowing the word already used in
every step message. The three misfires are the evidence, not an anecdote: a
prompt that misleads its own author will mislead anyone.

**F-5 · 🐛 A Ready package can break a Held one through a soname bump. NOT
FIXED — filed for the next cycle.** Live, unplanned, mid-matrix:

```
error: failed to prepare transaction (could not satisfy dependencies)
:: installing libbluray (1.5.0-1) breaks dependency 'libbluray.so=3-64'
   required by ffmpeg4.4
```

`libbluray` was Ready (hold just expired) and bumps `libbluray.so` from 3 to 4.
`ffmpeg4.4` was Held with 1 day remaining and still links the old soname. pacman
refused the **entire 78-package transaction**. All three coupling rules miss this
shape: the two packages share no pkgbase, no `lib32-` name pattern, and no version
cohort. A scan of all 78 Ready packages found exactly one such pair (`poppler` also
drops `libpoppler.so=162`, but nothing installed still needs it).

nog already holds the data — the sync DB carries `%PROVIDES%`, the local DB
carries `%DEPENDS%` — so a fourth rule falls out directly: *if a Ready candidate's
new version drops a soname that a Held package still depends on, demote it and
note `coupled to <pkg>`.* This is the depends/provides-graph generalization left
open since v1.0.6, now with a reproduction.

**Severity, stated honestly: this blocks updates, it does not break systems.**
pacman's resolver caught it and refused everything — the exact opposite of the
2026-08-25 qt6 split, which sailed through and killed SDDM. That difference is why
it is filed rather than hotfixed. Workaround, used live and cleanly:
`nog install ffmpeg4.4 libbluray` — both in one transaction, forward onto the new
soname, never back.

**F-6 · The continue prompt runs into the next line on non-interactive stdin.**
`nog: Continue with the remaining sources? [y/N] nog: no input — stopping here.`
Interactively the user's Enter supplies the newline, so this is invisible in normal
use; it only shows when stdin is a pipe or `/dev/null`. Cosmetic, one `eprintln!()`
to fix, deliberately **not** changed after the matrix went green — bundled with F-5
for the next cycle.

---

## Not run, and why

| Check | Why not run |
|---|---|
| §2.5 chaotic-aur | No chaotic package was ever Ready across both nights — every one of them was inside its hold window. The chaotic DB *fetch* happened in the pacman step, which is partial evidence only. |
| §3.6 helper skipped when empty | An AUR package was cleared in every run that reached the handoff. §6.4 shows yay uninvoked, but by the kill switch, which is a different code path. |
| §5.6 (partial) | flatpak and snap had nothing pending, so "the later steps still run" could not be distinguished from "there was nothing for them to do". |
| §8.1 Unknown prompt | UNKNOWN was empty in every single run. |
| §8.2 kernel/headers desync | `linux-zen` and `linux-zen-headers` were held at matching versions throughout. |
| §8.3 `--realign` | Not exercised by an update run. |

---

## Verdict

**v1.3.0 is releasable.** The release-blocker (§5.1) passes on a real failure,
not a simulation. Every failure-handling path has now executed at least once —
this was the release's whole risk, since #10's code had never run before tonight.
Two findings were fixed and re-verified in-session; two more (F-5, F-6) are filed
with reproductions, neither a v1.3.0 regression — F-5 has been latent since at
least v1.0.6.

Nothing carried forward from Run 1 remains unexplained.
