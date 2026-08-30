#!/usr/bin/env python3
"""Recompute a nog test matrix's roll-up from its own tables.

Never hand-tally the roll-up. It was hand-tallied twice during the v1.4.0
release and was wrong both times -- 44 and then 52 against an actual 63, and it
under-reported the failure count, which is the direction that matters.

Refuses to count a verdict it was not taught rather than binning it under a
guess: an unrecognised verdict is a defect in the matrix or in this script, and
either way silently folding it into PASS is the worst available answer.

    python3 testing/tally-matrix.py "testing/<matrix>.md"
"""
import collections
import re
import sys

VERDICTS = ["PASS", "FAIL", "N/A", "DEFERRED", "CANNOT TEST"]


def tally(path):
    section = None
    rows = []
    for line in open(path, encoding="utf-8"):
        m = re.match(r"^## (§\d+)", line)
        if m:
            section = m.group(1)
        m = re.match(r"^\|\s*(\d+\.\d+)\s*\|", line)
        if not m:
            continue
        check = m.group(1)
        # Split on unescaped pipes only: a check's text may contain a literal \|
        cells = [c.strip() for c in re.split(r"(?<!\\)\|", line)]
        verdict = None
        for cell in cells:
            text = cell.replace("*", "").strip().upper()
            # Classify on the LEADING token: result cells carry prose that
            # contains the words "pass" and "fail" further along.
            for v in VERDICTS:
                if text.startswith(v):
                    verdict = v
                    break
            if verdict:
                break
        if verdict is None:
            raise SystemExit(
                f"{path}: check {check} has no verdict this script was taught.\n"
                f"  row: {line.strip()[:140]}\n"
                f"  known verdicts: {', '.join(VERDICTS)}\n"
                "Refusing to guess. Add the verdict here or fix the row."
            )
        rows.append((section, check, verdict))
    return rows


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    rows = tally(sys.argv[1])
    if not rows:
        raise SystemExit("no numbered check rows found -- is that a matrix?")

    by_section = collections.Counter(s for s, _, _ in rows)
    counts = collections.Counter(v for _, _, v in rows)

    print("Per section:")
    for s in sorted(by_section, key=lambda x: int(x.strip("§"))):
        print(f"  {s:<5} {by_section[s]:>3}")
    print()
    print(f"{len(rows)} checks")
    for v in VERDICTS:
        if counts[v]:
            print(f"  {counts[v]:>3}  {v}")

    unfinished = counts["DEFERRED"] + counts["CANNOT TEST"]
    if unfinished:
        print(f"\n{unfinished} check(s) reached no verdict. "
              "Complete is not the same as proven.")


if __name__ == "__main__":
    main()
