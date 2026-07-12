#!/usr/bin/env python3
"""Export structure drafts from pdfparser JSON — SHADOW / comparison ONLY.

**Do NOT use this for real_structure gold.** Self-extract gold is circular and
has produced wrong grids. Use instead:

  python3 benchmark/scripts/build_structure_gold_peers.py --pdf ...

(peer Camelot/pdfplumber/vision consensus). See schema/gold_peer_build.md.

Usage (shadow only):
  cargo build --release -p pdfparser-cli
  python3 benchmark/scripts/export_structure_draft.py \\
    --pdf benchmark/corpus/06_table_lattice.pdf \\
    --out benchmark/real_track/gold/drafts/06_table_lattice.draft.json
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def tables_to_gold(tables: list, doc_id: str) -> dict:
    expected = []
    for t in tables:
        rows = int(t.get("rows") or 0)
        cols = int(t.get("cols") or 0)
        cells_in = t.get("cells") or []
        grid = [["" for _ in range(cols)] for _ in range(rows)]
        for c in cells_in:
            r = int(c.get("row", 0))
            col = int(c.get("col", 0))
            if 0 <= r < rows and 0 <= col < cols:
                grid[r][col] = c.get("text") or ""
        bb = t.get("bbox") or {}
        expected.append(
            {
                "page": int(t.get("page", 0)),
                "bbox": {
                    "x0": float(bb.get("x0", 0)),
                    "y0": float(bb.get("y0", 0)),
                    "x1": float(bb.get("x1", 0)),
                    "y1": float(bb.get("y1", 0)),
                },
                "rows": rows,
                "cols": cols,
                "cells": grid,
                "method_hint": t.get("method"),
            }
        )
    return {
        "id": doc_id,
        "schema_version": 1,
        "track": "structure_draft",
        "status": "needs_human_review",
        "policy": "Never auto-accept into real_structure G1 core without human edit.",
        "expected_table_count": len(expected),
        "expected_tables": expected,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--pdf", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument(
        "--binary",
        default=str(ROOT / "target/release/pdfparser"),
    )
    args = ap.parse_args()
    pdf = Path(args.pdf)
    if not pdf.is_file():
        print(f"missing pdf {pdf}", file=sys.stderr)
        return 1
    bin_p = Path(args.binary)
    if not bin_p.is_file():
        print(f"missing binary {bin_p}; run cargo build --release -p pdfparser-cli", file=sys.stderr)
        return 2
    r = subprocess.run(
        [str(bin_p), "extract", "--tables", "--format", "json", str(pdf)],
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        print(r.stderr, file=sys.stderr)
        return 1
    data = json.loads(r.stdout)
    tables = data.get("tables") or []
    # also pages[].tables
    if not tables:
        for p in data.get("pages") or []:
            tables.extend(p.get("tables") or [])
    doc_id = pdf.stem
    gold = tables_to_gold(tables, doc_id)
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(gold, indent=2), encoding="utf-8")
    print(f"wrote draft {out} tables={len(tables)} status=needs_human_review")
    return 0


if __name__ == "__main__":
    sys.exit(main())
