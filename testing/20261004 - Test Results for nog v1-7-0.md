# nog v1.7.0 — Test Results (4 Oct 2026)

The full run is nogForge's matrix: `nogforge/testing/20261004 - Test Matrix for nogForge v1-1-0.md`
(nog 1.7.0 is the nog half of nogForge 1.1's "nog inside the app").

| ID | What | Result |
|---|---|---|
| 1 | `NOG_EVENTS` on the desktop's real plan (answered n) | PASS — check start/done ("3 to install, 67 on hold"), steps list, nog's question, end 0 (steps list now sent before "check done") |
| 2 | Through nogForge in the KognogOS VM, tty3 | PASS — update with steps (check → pacman), repo install, AUR install via yay, cancelled password (`sudo: no password was provided` → stopped, nothing changed), removal |
| 3 | F-11: AUR neofetch → chaotic-aur unifetch | FOUND in the VM; fixed (`aur/x`, `repo/x`); then `install extra/cowsay` and `install extra/sl` by Javier on the desktop |
| 4 | F-12: banner 330 columns with four files | FOUND by Javier (install-rc.sh); fixed, capped at 80 |
| 5 | Javier, desktop: update leancrypto lib32-pcre2 pcre2 through nogForge | PASS (exit 0) |
| 6 | `cargo test --release --locked` | 240 passed, 5 ignored; warnings 6 |

Still to come: Javier's install of 1.7.0 from the AUR.
