#!/usr/bin/env python3
"""Offline structure-error taxonomy for table extractors (generic labels).

Compares predicted vs gold shapes on real_structure and optional ICDAR failure
analysis JSON. **Does not feed document ids into the engine** — measurement only.

Usage:
  python3 benchmark/scripts/structure_error_taxonomy.py
  python3 benchmark/scripts/structure_error_taxonomy.py --icdar-analysis benchmark/results/icdar_failure_analysis.json

Writes: benchmark/real_track/results/structure_error_taxonomy_latest.json
"""
from __future__ import annotations

import argparse
import json
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

SCRIPTS = Path(__file__).resolve().parent
BENCH = SCRIPTS.parent
RT = BENCH / "real_track"
OUT = RT / "results" / "structure_error_taxonomy_latest.json"


def classify_shape(pr: int, pc: int, gr: int, gc: int) -> list[str]:
    """Generic shape-error labels (no corpus names)."""
    tags: list[str] = []
    if pr == gr and pc == gc:
        return ["shape_exact"]
    if pr > gr:
        tags.append("over_row")
    elif pr < gr:
        tags.append("under_row")
    if pc > gc:
        tags.append("over_col")
    elif pc < gc:
        tags.append("under_col")
    dr, dc = abs(pr - gr), abs(pc - gc)
    if dr + dc >= 1:
        tags.append("wrong_shape")
    if dr >= 3 or (gr > 0 and pr / max(gr, 1) >= 2.0):
        tags.append("row_explode")
    if dc >= 3 or (gc > 0 and pc / max(gc, 1) >= 2.0):
        tags.append("col_explode")
    return tags


def from_real_structure(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    run = data["runs"][0]
    mode_counts: Counter[str] = Counter()
    per: list[dict[str, Any]] = []
    for doc in run.get("documents") or []:
        if doc.get("error"):
            continue
        # Prefer first table vs gold shape if available
        gold = doc.get("gold") or {}
        # gold may be embedded shapes via metrics
        m = doc.get("metrics") or {}
        shape = m.get("shape") or {}
        # tables list
        preds = doc.get("tables") or doc.get("pred_tables") or []
        # try rows/cols from summary
        pr = pc = gr = gc = None
        if preds and isinstance(preds[0], dict):
            pr = int(preds[0].get("rows") or 0)
            pc = int(preds[0].get("cols") or 0)
        # gold shape from metrics if present
        if "gold_rows" in m:
            gr = int(m["gold_rows"])
            gc = int(m.get("gold_cols") or 0)
        elif isinstance(gold, dict) and gold.get("tables"):
            gt0 = gold["tables"][0]
            gr = int(gt0.get("rows") or 0)
            gc = int(gt0.get("cols") or 0)
        # shape exact rate fields
        exact = shape.get("exact")
        tags: list[str] = []
        if exact is True:
            tags = ["shape_exact"]
        elif pr is not None and gr is not None and gr > 0:
            tags = classify_shape(pr, pc or 0, gr, gc or 0)
        elif exact is False:
            tags = ["wrong_shape"]
        else:
            tags = ["unknown"]
        for t in tags:
            mode_counts[t] += 1
        per.append(
            {
                "id": doc.get("id"),
                "tags": tags,
                "pred": [pr, pc],
                "gold": [gr, gc],
                "cell_f1": (m.get("cell") or {}).get("f1"),
            }
        )
    return {
        "source": "real_structure",
        "path": str(path),
        "n_docs": len(per),
        "mode_counts": dict(mode_counts),
        "documents": per,
    }


def from_icdar_analysis(path: Path) -> dict[str, Any]:
    """Map ICDAR failure modes → generic taxonomy (measurement only)."""
    data = json.loads(path.read_text(encoding="utf-8"))
    mode_map = {
        "ROW_MISCOUNT": "row_miscount",
        "COL_MISCOUNT": "col_miscount",
        "WRONG_SHAPE": "wrong_shape",
        "BAD_STRUCTURE": "content_structure",
        "OVER_DETECT": "over_detect",
        "UNDER_DETECT": "under_detect",
        "MISS_ALL": "miss_all",
        "MULTI_TABLE_PAGE": "multi_table_page",
        "MULTI_PAGE_DOC": "multi_page_doc",
    }
    counts: Counter[str] = Counter()
    shape_bins: Counter[str] = Counter()
    for d in data.get("per_doc") or []:
        for m in d.get("modes") or []:
            counts[mode_map.get(m, m.lower())] += 1
        # derive over/under row/col from first page first table if shapes present
        us = d.get("us_shapes") or []
        gt = d.get("gt_shapes") or []
        if us and gt and us[0] and gt[0]:
            pr, pc = us[0][0]
            gr, gc = gt[0][0]
            for t in classify_shape(pr, pc, gr, gc):
                shape_bins[t] += 1
    return {
        "source": "icdar_failure_analysis",
        "path": str(path),
        "mode_counts": dict(counts),
        "shape_bins_first_table": dict(shape_bins),
        "n_docs": len(data.get("per_doc") or []),
        "note": "ICDAR ids used for measurement only; not for engine control",
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--real-structure",
        type=Path,
        default=RT / "results" / "real_structure_latest.json",
    )
    ap.add_argument(
        "--icdar-analysis",
        type=Path,
        default=BENCH / "results" / "icdar_failure_analysis.json",
    )
    ap.add_argument("--out", type=Path, default=OUT)
    args = ap.parse_args()

    report: dict[str, Any] = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "policy": "generic shape taxonomy; no engine doc-id coupling",
    }
    if args.real_structure.is_file():
        report["real_structure"] = from_real_structure(args.real_structure)
    else:
        report["real_structure"] = {"error": f"missing {args.real_structure}"}
    if args.icdar_analysis.is_file():
        report["icdar"] = from_icdar_analysis(args.icdar_analysis)
    else:
        report["icdar"] = {"error": f"missing {args.icdar_analysis}"}

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {args.out}")
    rs = report.get("real_structure") or {}
    print("real mode_counts:", rs.get("mode_counts"))
    ic = report.get("icdar") or {}
    print("icdar mode_counts:", ic.get("mode_counts"))
    print("icdar shape_bins:", ic.get("shape_bins_first_table"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
