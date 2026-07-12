#!/usr/bin/env python3
"""Refuse ICDAR-2013 competition PDFs/gold in benchmark corpus paths.

Policy: ICDAR is external competitive-only. Never copy into corpus, real_track,
or ground_truth used by CI/regression. See docs/design-table-engine-v2.md.

Exit 0 if clean; exit 1 with diagnostics if forbidden basenames/patterns found.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCAN_DIRS = [
    ROOT / "corpus",
    ROOT / "real_track",
    ROOT / "ground_truth",
    ROOT / "downloads",
]

# Known ICDAR-2013 competition package basename patterns (eu-### / us-### family).
# These names are public knowledge; we do NOT store the PDFs.
FORBIDDEN_BASENAME_RE = re.compile(
    r"^(eu-\d{3}[a-z]?|us-\d{3}[a-z]?)\.(pdf|xml)$",
    re.IGNORECASE,
)
# Directory / path markers that must not appear under benchmark regression trees.
FORBIDDEN_PATH_FRAGMENTS = (
    "icdar2013",
    "icdar-2013",
    "icdar_2013",
    "tabula/icdar",
)


def main() -> int:
    hits: list[str] = []
    for d in SCAN_DIRS:
        if not d.exists():
            continue
        for p in d.rglob("*"):
            if not p.is_file():
                # still check dir names
                rel = str(p.relative_to(ROOT)).lower()
                for frag in FORBIDDEN_PATH_FRAGMENTS:
                    if frag in rel:
                        hits.append(f"path_fragment:{p.relative_to(ROOT)}")
                continue
            name = p.name
            if FORBIDDEN_BASENAME_RE.match(name):
                hits.append(f"basename:{p.relative_to(ROOT)}")
            rel = str(p.relative_to(ROOT)).lower()
            for frag in FORBIDDEN_PATH_FRAGMENTS:
                if frag in rel:
                    hits.append(f"path_fragment:{p.relative_to(ROOT)}")

    # Allow docs and scripts that *mention* ICDAR; only binary/data trees scanned.
    if hits:
        print("ERROR: ICDAR competition material must not live under benchmark regression paths.", file=sys.stderr)
        for h in sorted(set(hits))[:50]:
            print(f"  {h}", file=sys.stderr)
        print(f"  total hits: {len(set(hits))}", file=sys.stderr)
        return 1
    print("assert_no_icdar: OK (no forbidden ICDAR basenames/paths under benchmark data trees)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
