#!/usr/bin/env python3
"""Validate real_structure gold files (T3 schema). Exit 0 if suite empty or all valid."""
from __future__ import annotations
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MAN = ROOT / "real_track" / "manifests" / "real_structure_v0.json"
GOLD = ROOT / "real_track" / "gold"


def main() -> int:
    if not MAN.exists():
        print("missing real_structure_v0.json", file=sys.stderr)
        return 1
    man = json.loads(MAN.read_text())
    docs = man.get("documents") or []
    if not docs:
        print("validate_structure_gold: OK (empty suite — G1 not ready, n=0)")
        return 0
    errors = []
    g1_min = int(man.get("g1_min_docs") or 15)
    g1_ready = bool(man.get("g1_ready"))
    n = int(man.get("n_docs") or len(docs))
    if g1_ready and n < g1_min:
        errors.append(f"g1_ready true but n_docs={n} < g1_min_docs={g1_min}")
    if n != len(docs):
        errors.append(f"n_docs={n} != len(documents)={len(docs)}")
    for d in docs:
        gid = d.get("gold") or d.get("id")
        path = ROOT / "real_track" / (d.get("gold_path") or f"gold/{gid}.json")
        if not path.exists():
            # try flat gold id
            path = GOLD / f"{d.get('id', gid)}.json"
        if not path.exists():
            # gold field may already be basename
            path = GOLD / Path(str(gid)).name
        if not path.exists():
            errors.append(f"missing gold {path}")
            continue
        g = json.loads(path.read_text())
        if g.get("track") not in (None, "real_structure"):
            errors.append(f"{path.name}: track must be real_structure (got {g.get('track')})")
        if g.get("status") in ("needs_human_review", "draft", "auto"):
            errors.append(f"{path.name}: status={g.get('status')} not reviewed for core")
        et = g.get("expected_tables") or []
        if not et:
            errors.append(f"{path.name}: no expected_tables")
            continue
        for i, t in enumerate(et):
            if not isinstance(t, dict):
                errors.append(f"{path.name} table {i}: not object")
                continue
            cells = t.get("cells")
            if not cells:
                errors.append(f"{path.name} table {i}: missing cells (need T3)")
            if "bbox" not in t and "page" not in t:
                errors.append(f"{path.name} table {i}: prefer bbox+page for IoU")
    if errors:
        for e in errors:
            print("ERR", e, file=sys.stderr)
        return 1
    struggle_n = sum(1 for d in docs if d.get("struggle"))
    print(
        f"validate_structure_gold: OK ({len(docs)} docs, struggle={struggle_n}, "
        f"g1_ready={g1_ready})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
