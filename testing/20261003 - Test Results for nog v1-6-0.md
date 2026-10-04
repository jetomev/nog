# nog v1.6.0: Test Results (2026-10-03)

nog 1.6.0 was built for nogForge and tested through it, on the desktop (`tphome-linux`), from 1.6.0-rc.1 to rc.5.

## Automated
| Check | Result |
|---|---|
| `cargo test --release --locked` | **234 passed**, 5 ignored (224 → 234) |
| Warnings | 6 → 6 |

## On the desktop, against its real state
| ID | Check | Result |
|---|---|---|
| 1 | `nog update --json` gives one JSON document; nothing else on stdout | PASS |
| 2 | `--keep ldb`: libwbclient and smbclient stay back with it (same Samba build) | PASS |
| 3 | `--promote linux-zen`: "promoted by you"; linux-zen-headers "promoted with linux-zen"; keep wins over promote | PASS |
| 4 | `nog install calc` / `nog install <file>` with print-only stand-in sudo and yay: exactly `yay -S calc` / `pacman -U -- <file>`, nothing else (F-8, #41) | PASS |
| 5 | A recorded run (scratch home): the whole screen in `YYYYMMDD-HHMMSS <command>.log`; the closing note names it (#42) | PASS |
| 6 | A failing recorded run (`nog install no-such-package-xyz`) ends with status 1; its log has nog's whole message | PASS |
| 7 | A run without a terminal is not recorded and still names its CSV | PASS |
| 8 | `.log` files older than 30 days are deleted; CSVs never (unit test) | PASS |

## Javier's runs
| When | What | Result |
|---|---|---|
| 3 Oct 12:15 | rc.1 installed; Promote on archiso from nogForge | done (Promote then installed at once: became `--promote`, F-5 of nogForge) |
| 3 Oct 21:16 | rc.2 installed with `nog install <file>` | **F-8 found**: nog updated 20 packages first (#41) |
| 3 Oct 21:27 | rc.3 installed | PASS: only nog |
| 3 Oct 21:58 | rc.4 installed | PASS: only nog (no `.log`: rc.3 ran that install) |
| 3 Oct 22:18 | `nog install python-forgekit` (AUR) | PASS: 0.5.2 built and installed; the run kept whole as a `.log` (11 KB) |
| 3 Oct 22:20 | rc.5 installed | PASS: only nog |
| 4 Oct | the stable 1.6.0 from the AUR; nogForge Update with keep/promote | pending |
