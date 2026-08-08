#!/usr/bin/env python3
"""Offline structure-error taxonomy for table extractors (generic labels).

Compares predicted vs gold shapes on real_structure (`metrics.per_table[]`).
**Does not feed document ids into the engine** — measurement only.

Default invocation is ICDAR-free (real_structure only). `--icdar-analysis` is
optional/external and must not be wired into CI.

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
REPO = BENCH.parent
RT = BENCH / "real_track"
OUT = RT / "results" / "structure_error_taxonomy_latest.json"

# Shadow metric only (H17): record census cell/shape; do not assert 10-col.
CENSUS_ID_SUBSTR = "census"


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


def _shape_pair(val: Any) -> tuple[int, int] | None:
    if isinstance(val, (list, tuple)) and len(val) >= 2:
        try:
            return int(val[0]), int(val[1])
        except (TypeError, ValueError):
            return None
    return None


def _is_census_id(doc_id: Any) -> bool:
    return CENSUS_ID_SUBSTR in str(doc_id or "").lower()


def _repo_rel(path: Path) -> str:
    """Stable artifact path: repo-relative when possible (no machine worktrees)."""
    try:
        return path.resolve().relative_to(REPO.resolve()).as_posix()
    except ValueError:
        return Path(path).as_posix()


def _classify_per_table(pt: dict[str, Any]) -> dict[str, Any]:
    pred = _shape_pair(pt.get("pred_shape"))
    gold = _shape_pair(pt.get("gold_shape"))
    unmatched = bool(pt.get("unmatched_gold")) or (
        isinstance(pt.get("pred_index"), int) and pt["pred_index"] < 0
    )
    # Sentinel pred_shape [0, 0] + pred_index < 0 is a detection miss, not a 0x0 extract.
    if unmatched:
        tags = ["unmatched_gold"]
    elif pred is not None and gold is not None:
        tags = classify_shape(pred[0], pred[1], gold[0], gold[1])
    elif pt.get("shape_exact") is True:
        tags = ["shape_exact"]
    elif pt.get("shape_exact") is False:
        tags = ["wrong_shape"]
    else:
        tags = ["unknown"]
    return {
        "gold_index": pt.get("gold_index"),
        "pred_index": pt.get("pred_index"),
        "pred": [pred[0], pred[1]] if pred is not None else [None, None],
        "gold": [gold[0], gold[1]] if gold is not None else [None, None],
        "tags": tags,
        "cell_f1": pt.get("cell_f1"),
        "shape_exact": pt.get("shape_exact"),
        "unmatched_gold": unmatched or None,
    }


def from_real_structure(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    run = data["runs"][0]
    mode_counts: Counter[str] = Counter()
    per: list[dict[str, Any]] = []
    census_shadow: list[dict[str, Any]] = []
    n_tables = 0
    for doc in run.get("documents") or []:
        if doc.get("error"):
            continue
        m = doc.get("metrics") or {}
        per_table = m.get("per_table") or []
        table_rows: list[dict[str, Any]] = []
        doc_tags: list[str] = []
        seen: set[str] = set()

        if per_table:
            for pt in per_table:
                if not isinstance(pt, dict):
                    continue
                row = _classify_per_table(pt)
                table_rows.append(row)
                n_tables += 1
                for t in row["tags"]:
                    mode_counts[t] += 1
                    if t not in seen:
                        seen.add(t)
                        doc_tags.append(t)
        else:
            # Fallback for older JSON without per_table shapes.
            gold = doc.get("gold") or {}
            shape = m.get("shape") or {}
            preds = doc.get("tables") or doc.get("pred_tables") or []
            pr = pc = gr = gc = None
            if preds and isinstance(preds[0], dict):
                pr = int(preds[0].get("rows") or 0)
                pc = int(preds[0].get("cols") or 0)
            if "gold_rows" in m:
                gr = int(m["gold_rows"])
                gc = int(m.get("gold_cols") or 0)
            elif isinstance(gold, dict) and gold.get("tables"):
                gt0 = gold["tables"][0]
                gr = int(gt0.get("rows") or 0)
                gc = int(gt0.get("cols") or 0)
            exact = shape.get("exact")
            if exact is True:
                doc_tags = ["shape_exact"]
            elif pr is not None and gr is not None and gr > 0:
                doc_tags = classify_shape(pr, pc or 0, gr, gc or 0)
            elif exact is False:
                doc_tags = ["wrong_shape"]
            else:
                doc_tags = ["unknown"]
            for t in doc_tags:
                mode_counts[t] += 1
            n_tables += 1
            table_rows.append(
                {
                    "gold_index": 0,
                    "pred_index": 0 if pr is not None else -1,
                    "pred": [pr, pc],
                    "gold": [gr, gc],
                    "tags": doc_tags,
                    "cell_f1": (m.get("cell") or {}).get("f1"),
                    "shape_exact": exact,
                    "unmatched_gold": None,
                }
            )

        first = table_rows[0] if table_rows else {
            "pred": [None, None],
            "gold": [None, None],
        }
        per.append(
            {
                "id": doc.get("id"),
                "tags": doc_tags or ["unknown"],
                "pred": first.get("pred"),
                "gold": first.get("gold"),
                "cell_f1": (m.get("cell") or {}).get("f1"),
                "tables": table_rows,
            }
        )
        if _is_census_id(doc.get("id")):
            census_shadow.append(
                {
                    "id": doc.get("id"),
                    "doc_cell_f1": (m.get("cell") or {}).get("f1"),
                    "n_pred": m.get("n_pred"),
                    "n_exp": m.get("n_exp"),
                    "tables": [
                        {
                            "gold_index": r.get("gold_index"),
                            "pred_index": r.get("pred_index"),
                            "pred_shape": r.get("pred"),
                            "gold_shape": r.get("gold"),
                            "cell_f1": r.get("cell_f1"),
                            "shape_exact": r.get("shape_exact"),
                            "tags": r.get("tags"),
                            "unmatched_gold": r.get("unmatched_gold"),
                        }
                        for r in table_rows
                    ],
                }
            )
    return {
        "source": "real_structure",
        "path": _repo_rel(path),
        "n_docs": len(per),
        "n_tables": n_tables,
        "mode_counts": dict(mode_counts),
        "note": "shapes from metrics.per_table[].pred_shape/gold_shape; mode_counts are per-table",
        "census_shadow": {
            "note": "record-only cell/shape for census docs; no 10-col e2e assert (H17)",
            "documents": census_shadow,
        },
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
        "path": _repo_rel(path),
        "mode_counts": dict(counts),
        "shape_bins_first_table": dict(shape_bins),
        "n_docs": len(data.get("per_doc") or []),
        "note": "ICDAR ids used for measurement only; not for engine control; not CI",
    }


def _print_census_shadow(shadow: dict[str, Any]) -> None:
    docs = shadow.get("documents") or []
    print("census shadow (record only; no 10-col assert):")
    if not docs:
        print("  (none)")
        return
    for d in docs:
        print(
            f"  {d.get('id')} n_pred={d.get('n_pred')} n_exp={d.get('n_exp')} "
            f"doc_cell_f1={d.get('doc_cell_f1')}"
        )
        for t in d.get("tables") or []:
            print(
                f"    gold_i={t.get('gold_index')} pred_i={t.get('pred_index')} "
                f"pred={t.get('pred_shape')} gold={t.get('gold_shape')} "
                f"cell_f1={t.get('cell_f1')} tags={t.get('tags')}"
            )


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
        default=None,
        help="Optional external ICDAR failure analysis JSON. Not default; not CI.",
    )
    ap.add_argument("--out", type=Path, default=OUT)
    args = ap.parse_args()

    report: dict[str, Any] = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "policy": (
            "generic shape taxonomy; no engine doc-id coupling; "
            "default=real_structure only; --icdar-analysis external optional"
        ),
    }
    if args.real_structure.is_file():
        report["real_structure"] = from_real_structure(args.real_structure)
    else:
        report["real_structure"] = {"error": f"missing {args.real_structure}"}

    if args.icdar_analysis is not None:
        if args.icdar_analysis.is_file():
            report["icdar"] = from_icdar_analysis(args.icdar_analysis)
        else:
            report["icdar"] = {"error": f"missing {args.icdar_analysis}"}

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {args.out}")
    rs = report.get("real_structure") or {}
    print("real n_docs:", rs.get("n_docs"), "n_tables:", rs.get("n_tables"))
    print("real mode_counts:", rs.get("mode_counts"))
    unknown = (rs.get("mode_counts") or {}).get("unknown", 0)
    n_tables = rs.get("n_tables")
    print(f"unknown tables: {unknown}/{n_tables}")
    if isinstance(rs.get("census_shadow"), dict):
        _print_census_shadow(rs["census_shadow"])
    if "icdar" in report:
        ic = report.get("icdar") or {}
        print("icdar mode_counts:", ic.get("mode_counts"))
        print("icdar shape_bins:", ic.get("shape_bins_first_table"))
    else:
        print("icdar: skipped (pass --icdar-analysis PATH; external only, not CI)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
