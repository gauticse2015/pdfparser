#!/usr/bin/env python3
"""Compare two pdfparser_table_dump_v1 dumps (P0.6 mechanical no-regress).

Must-match: table count, method, rows/cols, bbox snap (0.5pt), cell indices/spans/text,
header_rows, weak_edges, typed flags, strategy_provenance set, document engine_path.

Allowed to differ: notes text, confidence <=1e-4, elapsed, diagnostics pretty-print.

  python3 benchmark/scripts/compare_table_dumps.py --before before.json --after after.json
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

SCHEMA = "pdfparser_table_dump_v1"
BBOX_TOL = 0.5
CONF_TOL = 1e-4
TYPED_FLAGS = (
    "weak_edges",
    "text_row_recovery",
    "text_col_recovery",
    "multitable_stream_recovery",
    "stream_vs_overwide_hybrid",
    "continued_from_previous_page",
    "continued_to_next_page",
)


def table_sort_key(t: dict) -> tuple:
    bb = t.get("bbox") or {}
    y1 = float(bb.get("y1", 0.0))
    x0 = float(bb.get("x0", 0.0))
    page = int(t.get("page", 0))
    return (-y1, x0, page)


def _bbox(t: dict) -> dict:
    bb = t.get("bbox") or {}
    return {
        "x0": float(bb.get("x0", 0.0)),
        "y0": float(bb.get("y0", 0.0)),
        "x1": float(bb.get("x1", 0.0)),
        "y1": float(bb.get("y1", 0.0)),
    }


def _num_close(a: float, b: float, tol: float) -> bool:
    return abs(a - b) <= tol


def _provenance_set(t: dict) -> set[str]:
    vals = t.get("strategy_provenance") or []
    return {str(v) for v in vals}


def _cells_sorted(t: dict) -> list[dict]:
    cells = [c for c in (t.get("cells") or []) if isinstance(c, dict)]
    cells.sort(key=lambda c: (int(c.get("row", 0)), int(c.get("col", 0))))
    return cells


def flatten_tables(doc: dict) -> list[dict]:
    tabs: list[dict] = []
    for page in doc.get("pages") or []:
        idx = int(page.get("index", 0))
        for t in page.get("tables") or []:
            if not isinstance(t, dict):
                continue
            tt = dict(t)
            tt.setdefault("page", idx)
            tabs.append(tt)
    tabs.sort(key=table_sort_key)
    return tabs


def compare_table(prefix: str, a: dict, b: dict) -> list[str]:
    diffs: list[str] = []
    if a.get("method") != b.get("method"):
        diffs.append(f"{prefix} method {a.get('method')!r} != {b.get('method')!r}")
    if int(a.get("rows") or 0) != int(b.get("rows") or 0):
        diffs.append(f"{prefix} rows {a.get('rows')} != {b.get('rows')}")
    if int(a.get("cols") or 0) != int(b.get("cols") or 0):
        diffs.append(f"{prefix} cols {a.get('cols')} != {b.get('cols')}")
    if int(a.get("page") or 0) != int(b.get("page") or 0):
        diffs.append(f"{prefix} page {a.get('page')} != {b.get('page')}")
    if int(a.get("header_rows") or 0) != int(b.get("header_rows") or 0):
        diffs.append(f"{prefix} header_rows {a.get('header_rows')} != {b.get('header_rows')}")
    ba, bb = _bbox(a), _bbox(b)
    for k in ("x0", "y0", "x1", "y1"):
        if not _num_close(ba[k], bb[k], BBOX_TOL):
            diffs.append(f"{prefix} bbox.{k} {ba[k]} != {bb[k]} (tol={BBOX_TOL})")
    for flag in TYPED_FLAGS:
        if bool(a.get(flag)) != bool(b.get(flag)):
            diffs.append(f"{prefix} {flag} {a.get(flag)!r} != {b.get(flag)!r}")
    pa, pb = _provenance_set(a), _provenance_set(b)
    if pa != pb:
        diffs.append(f"{prefix} strategy_provenance {sorted(pa)} != {sorted(pb)}")
    ca, cb = _cells_sorted(a), _cells_sorted(b)
    if len(ca) != len(cb):
        diffs.append(f"{prefix} cell_count {len(ca)} != {len(cb)}")
    else:
        for i, (xa, xb) in enumerate(zip(ca, cb)):
            for key in ("row", "col", "rowspan", "colspan"):
                if int(xa.get(key, 0)) != int(xb.get(key, 0)):
                    diffs.append(f"{prefix} cell[{i}].{key} {xa.get(key)} != {xb.get(key)}")
            if (xa.get("text") or "") != (xb.get("text") or ""):
                diffs.append(f"{prefix} cell[{i}].text {xa.get('text')!r} != {xb.get('text')!r}")
    # confidence may drift by 1e-4; only fail beyond that
    if a.get("confidence") is not None and b.get("confidence") is not None:
        if not _num_close(float(a["confidence"]), float(b["confidence"]), CONF_TOL):
            diffs.append(
                f"{prefix} confidence {a.get('confidence')} != {b.get('confidence')} (tol={CONF_TOL})"
            )
    return diffs


def compare_dumps(before: dict, after: dict) -> list[str]:
    diffs: list[str] = []
    if before.get("schema") != SCHEMA:
        diffs.append(f"before schema {before.get('schema')!r} != {SCHEMA!r}")
    if after.get("schema") != SCHEMA:
        diffs.append(f"after schema {after.get('schema')!r} != {SCHEMA!r}")
    if before.get("engine_path") != after.get("engine_path"):
        diffs.append(
            f"engine_path {before.get('engine_path')!r} != {after.get('engine_path')!r}"
        )
    if bool(before.get("stitch_multipage")) != bool(after.get("stitch_multipage")):
        diffs.append(
            f"stitch_multipage {before.get('stitch_multipage')!r} != {after.get('stitch_multipage')!r}"
        )

    bdocs = {d.get("id"): d for d in (before.get("documents") or []) if d.get("id")}
    adocs = {d.get("id"): d for d in (after.get("documents") or []) if d.get("id")}
    if set(bdocs) != set(adocs):
        diffs.append(f"document ids {sorted(bdocs)} != {sorted(adocs)}")
        return diffs

    for did in bdocs:
        bt = flatten_tables(bdocs[did])
        at = flatten_tables(adocs[did])
        if len(bt) != len(at):
            diffs.append(f"{did}: table count {len(bt)} != {len(at)}")
            continue
        for i, (ta, tb) in enumerate(zip(bt, at)):
            diffs.extend(compare_table(f"{did}[{i}]", ta, tb))
    return diffs


def load_dump(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise SystemExit(f"{path} is not a JSON object")
    return data


def main() -> int:
    ap = argparse.ArgumentParser(description="Compare pdfparser_table_dump_v1 dumps.")
    ap.add_argument("--before", type=Path, required=True)
    ap.add_argument("--after", type=Path, required=True)
    args = ap.parse_args()
    if not args.before.is_file():
        print(f"missing --before {args.before}", file=sys.stderr)
        return 2
    if not args.after.is_file():
        print(f"missing --after {args.after}", file=sys.stderr)
        return 2
    before = load_dump(args.before)
    after = load_dump(args.after)
    diffs = compare_dumps(before, after)
    if diffs:
        print(f"DIFF: {len(diffs)} mismatch(es)")
        for d in diffs:
            print(f"  {d}")
        return 1
    print("IDENTICAL (count/method/bbox/cells/provenance/engine_path)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
