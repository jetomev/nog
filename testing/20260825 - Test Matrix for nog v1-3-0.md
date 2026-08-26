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

Hard to trigger naturally; simulate where noted. **§5.1 is a release-blocker.**

| # | Check | Expected | Result |
|---|---|---|---|
| 5.1 | **pacman fails → cancel** | alert names the status; **no other source runs**; log says `pacman handoff failed (status N)` | ⚪ **not run — BLOCKER** |
| 5.2 | helper fails → ask | prompt `Continue with the remaining sources? [y/N]` | ⚪ not run |
| 5.3 | Default is no | bare Enter stops the run | ⚪ not run |
| 5.4 | EOF stops | `</dev/null` → `no input — stopping here.` | ⚪ not run |
| 5.5 | Answering no | log says `cancelled after aur step failed (status N)` | ⚪ not run |
| 5.6 | Answering yes | flatpak/snap still run; log says `installed with failures: ...` | ⚪ not run |
| 5.7 | Summary printed | `Update finished, with N failed step(s).` + the list | ⚪ not run |

> **Simulation plan for Run 2 (revised).** No fake binaries needed. pacman and
> yay both exit non-zero when the user declines their own `Proceed with
> installation? [Y/n]` prompt, which makes the whole of §5 reachable with two
> ordinary runs:
>
> - **5.1** — run `nog update`, answer **n** at pacman's prompt. Expect nog to
>   cancel and never reach yay.
> - **5.2/5.3/5.5** — run again, answer **Y** at pacman's prompt and **n** at
>   yay's. Expect nog's own continue-prompt, defaulting to no.
> - **5.6/5.7** — same, then answer **y** to the continue-prompt.
> - **5.4** — `nog update </dev/null` once a helper failure is in play.
>
> Do **not** simulate by interrupting a real pacman transaction mid-write.

---

## §6 · Fence and holds still intact

| # | Check | Expected | Result |
|---|---|---|---|
| 6.1 | Fence message reworded | `held back as a dependency`, not `shielded from the handoff` | ✅ |
| 6.2 | Fence still populated | count matches uncleared foreign packages | ✅ 22 foreign − 2 with updates = **20**, matches the message exactly |
| 6.3 | Held AUR package cannot move | absent from the helper's argument list | ✅ `snapd` absent |
| 6.4 | AUR deactivated | `nog deactivate aur` → pacman step only, helper never invoked | ⚪ not run (writes `/etc/nog/sources.toml`, needs sudo) |
| 6.5 | Reactivate | `nog activate aur` restores step 2 | ⚪ not run |

---

## §7 · Report, log, and docs

| # | Check | Expected | Result |
|---|---|---|---|
| 7.1 | Per-source counts match execution | reported counts describe what actually ran | ✅ 169+2+0+0 reported; 8 official + 1 AUR installed |
| 7.2 | CSV written | dated file under `~/.local/share/nog/logs/` | ✅ `20260826 nog-update.csv` |
| 7.3 | Outcome column accurate | one of the documented values | ✅ `installed` on all 171 rows |
| 7.4 | Man page matches behaviour | `man nog` four-step description is what happened | ✅ |
| 7.5 | README sample matches | the example output resembles the real run | ✅ |
| 7.6 | Version sync | binary, man page, README badge, PKGBUILD all `1.3.0` | ✅ binary/man/badge `1.3.0`; PKGBUILD `1.2.1`, bumps at release |

---

## §8 · Regression slice from v1.2.0

| # | Check | Expected | Result |
|---|---|---|---|
| 8.1 | Unknown prompt still works | per-package `[y/N]` | ⚪ not run — UNKNOWN was empty |
| 8.2 | Kernel/headers desync warning | unchanged | ⚪ not run — kernel and headers were in step |
| 8.3 | `--realign` | unchanged | ⚪ not run |
| 8.4 | `nog search` tier colours | unchanged | ✅ |
| 8.5 | `nog install` | routes through the helper, unchanged | ⚪ not run |
| 8.6 | All-held early exit | `Nothing to install — every pending update is held.` | ⚪ not run — carried to Run 2 |

---

## Field observations (not checks)

**F-1 · v1.2.1 died on a transient mirror timeout; v1.3.0 walked through it.**
The 07:00 PM run in the same log file is the *installed* v1.2.1 (`/usr/bin/nog`),
run minutes before the test build. Its handoff — `yay -Syu` — reached yay's own
`pacman -S -y` (pacman.log 19:01:20), which could not fetch `chaotic-aur.db`
from `geo-mirror.chaotic.cx` (10 s timeout). yay aborted, and nog logged
`handoff failed (status 1)` against all 171 rows: **nothing was installed.**

Forty-two seconds later v1.3.0's `pacman -Syu` hit the *same* timeout, treated a
single unreachable third-party DB as non-fatal, and completed all 8 upgrades.

This was not planned, and it is the strongest evidence for #10 in the file: the
split handoff removed a failure mode nobody had filed — one flaky third-party
mirror could take down the entire update, including packages from `core` and
`extra` that had nothing to do with it.

**F-2 · pacman.log misrepresents nog's ignore list.** nog passes one
comma-joined `--ignore`; pacman splits that string **in place inside `argv`**
and only then writes its `Running '...'` line, so the log records:

```
[PACMAN] Running 'pacman -Syu --ignore archlinux-appstream-data'
```

— one name, where 162 were passed. Verified by `--print`: a genuine single-name
ignore plans 160 packages, and the run installed 8. Anyone auditing pacman.log
after an incident will conclude nog held exactly one package. Worth a
troubleshooting note; it is not a nog defect.

**F-3 · "failed" is the wrong word for a user cancel.** If the user answers
**n** at pacman's own `Proceed with installation?`, pacman exits 1 and nog will
log `pacman handoff failed (status N)`. Stopping is correct; calling a
deliberate decline a *failure* is not, and the run log is the permanent record.
`cancelled` vocabulary already exists in §5.5. Confirm the wording during 5.1
and decide before tagging.

---

## Not run, and why

| Check | Why not run |
|---|---|
| §2.5 chaotic-aur | No chaotic-aur package was Ready; all 8 were `extra/`. The chaotic DB *fetch* did happen in the pacman step, which is partial evidence only. |
| §3.6 helper skipped when empty | An AUR package *was* cleared, so the empty case never arose. |
| §5.1–5.7 failure handling | Requires a deliberate non-zero exit from a handoff step. Reachable via the declined-prompt plan above; **§5.1 blocks the tag.** |
| §6.4–6.5 source kill switch | `nog activate` / `deactivate` write `/etc/nog/sources.toml` via sudo — hand-run, not agent-run. |
| §8.1 Unknown prompt | UNKNOWN bucket was empty. |
| §8.2 kernel/headers desync | `linux-zen` and `linux-zen-headers` were both held at matching versions — no desync to warn about. |
| §8.3 `--realign`, §8.5 `nog install` | Not exercised by an update run. |
| §8.6 all-held early exit | Every Ready package was consumed by this run; the next all-held run will show it. |
