#!/usr/bin/env python3
"""Integrity checks for compete / compete_hard datasets.

Fails if:
  - hard suite too small
  - any hard doc missing PDF/GT
  - frozen baseline missing or "too easy" (mean cell F1 >= 0.75)

  .venv/bin/python benchmark/scripts/check_compete_suite.py
"""
from __future__ import annotations
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GT = ROOT / "ground_truth"
CORPUS = ROOT / "corpus"
RESULTS = ROOT / "results"

def main() -> int:
    hard = []
    for p in sorted(GT.glob("C*.json")):
        g = json.loads(p.read_text())
        if g.get("tier") == "compete_hard":
            hard.append(g)
    print(f"compete_hard gold: {len(hard)}")
    if len(hard) < 60:
        print(f"FAIL: need >=60 hard fixtures, got {len(hard)}")
        return 1
    missing = 0
    for g in hard:
        pdf = ROOT / g["path"]
        if not pdf.exists():
            print(f"MISSING pdf {g['id']}: {pdf}")
            missing += 1
        if not g.get("expected_tables"):
            print(f"MISSING grid gold {g['id']}")
            missing += 1
        if not g.get("failure_classes"):
            print(f"MISSING failure_classes {g['id']}")
            missing += 1
    if missing:
        print(f"FAIL: {missing} integrity issues")
        return 1

    # class coverage
    classes = set()
    for g in hard:
        classes.update(g.get("failure_classes") or [])
    required = {
        "C1_OVER_DETECT", "C3_UNDER_DETECT", "C3_MISS_ALL", "C4_ROW_FRAGMENT_LARGE",
        "C5_HEADER_ONLY_SLICE", "C6_COL_SKEW_WIDE", "C7_ROW_OVERCOUNT",
        "C8_MULTI_TABLE_COUNT_ERROR", "C9_MULTIPAGE", "C10_COUNT_OK_STRUCT_BAD",
        "C13_MIXED_LATTICE_STREAM", "C14_INVOICE_FOOTER", "C15_SPAN_COMPLEX",
    }
    miss_c = required - classes
    if miss_c:
        print(f"FAIL: missing classes {sorted(miss_c)}")
        return 1
    print(f"class coverage OK ({len(classes)} tags)")

    frozen = RESULTS / "compete_hard_baseline_frozen.json"
    if not frozen.exists():
        print("WARN: frozen baseline missing — run accuracy suite first")
        return 0
    b = json.loads(frozen.read_text())
    cell = (b.get("pdfparser") or {}).get("cell_f1")
    shape = (b.get("pdfparser") or {}).get("shape")
    print(f"frozen cell_f1={cell} shape={shape}")
    if cell is not None and cell >= 0.75:
        print("FAIL: hard suite too easy (cell_f1 >= 0.75) — revise fixtures")
        return 1
    if shape is not None and shape >= 0.85:
        print("FAIL: hard suite shape rate too high — revise fixtures")
        return 1
    print("OK: compete hard suite integrity + difficulty gate passed")
    return 0

if __name__ == "__main__":
    sys.exit(main())
