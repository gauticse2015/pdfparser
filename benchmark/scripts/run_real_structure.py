#!/usr/bin/env python3
"""real_structure harness — cell/shape/det metrics vs T3 gold (K27 stitch off).

Reads ``benchmark/real_track/manifests/real_structure_v0.json``.

Live mode:

  pdfparser extract --tables --table-preset <preset> --no-stitch --page-tables \\
    --format json <pdf>

Scores with ``metrics.table_accuracy`` (+ IoU detection when bboxes present).
Writes ``benchmark/real_track/results/real_structure_latest.json``.

Policy
------
* Never claims G1 / market SOTA.
* stitch_multipage=false (CLI --no-stitch).
* No ICDAR paths.
* Does not auto-promote gold.

Usage
-----
  python3 benchmark/scripts/run_real_structure.py --dry-run
  python3 benchmark/scripts/run_real_structure.py --build
  python3 benchmark/scripts/run_real_structure.py --preset auto
  python3 benchmark/scripts/run_real_structure.py --preset engine-v2 --compare
  python3 benchmark/scripts/run_real_structure.py --strict
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

SCRIPTS = Path(__file__).resolve().parent
BENCH = SCRIPTS.parent
REPO = BENCH.parent
sys.path.insert(0, str(SCRIPTS))

from metrics import (  # noqa: E402
    table_accuracy,
    table_detection_metrics_iou,
)

MANIFEST_DEFAULT = BENCH / "real_track" / "manifests" / "real_structure_v0.json"
RESULTS_DIR = BENCH / "real_track" / "results"
OUT_DEFAULT = RESULTS_DIR / "real_structure_latest.json"
RELEASE_BIN = REPO / "target" / "release" / "pdfparser"

STITCH_POLICY = False
STITCH_NOTE = (
    "real_structure forces stitch_multipage=false via CLI --no-stitch (design K27). "
    "Root tables = page-local fragments (--page-tables)."
)

def _rel(path: Path, base: Path) -> str:
    try:
        return str(path.resolve().relative_to(base.resolve()))
    except Exception:
        return str(path)

PRESET_CLI = {
    "auto": "auto",
    "engine-v2": "engine-v2",
    "high-quality": "high-quality",
    "fast": "fast",
    "full": "full",
    "lattice-only": "lattice-only",
}


def find_binary() -> Optional[Path]:
    for cand in (RELEASE_BIN, REPO / "target" / "debug" / "pdfparser"):
        if cand.is_file():
            return cand
    which = shutil.which("pdfparser")
    return Path(which) if which else None


def try_build_release(timeout: float = 600.0) -> tuple[bool, str]:
    cargo = shutil.which("cargo")
    if not cargo:
        return False, "cargo not found"
    cmd = [cargo, "build", "--release", "-p", "pdfparser-cli"]
    print(f"building: {' '.join(cmd)}", flush=True)
    try:
        proc = subprocess.run(
            cmd, cwd=str(REPO), capture_output=True, text=True, timeout=timeout
        )
    except subprocess.TimeoutExpired:
        return False, "cargo build timeout"
    if proc.returncode != 0:
        tail = (proc.stderr or proc.stdout or "").strip().splitlines()[-25:]
        return False, "build failed:\n" + "\n".join(tail)
    if not RELEASE_BIN.is_file():
        return False, f"binary missing: {RELEASE_BIN}"
    return True, f"built {RELEASE_BIN}"


def resolve_pdf(pdf_rel: str) -> Path:
    for base in (BENCH, REPO, REPO / "benchmark"):
        p = base / pdf_rel
        if p.is_file():
            return p
    return BENCH / pdf_rel


def resolve_gold(doc: dict) -> Path:
    gp = doc.get("gold_path") or f"gold/{doc.get('gold') or doc.get('id')}.json"
    # strip leading real_track/
    gp = gp.replace("real_track/", "") if gp.startswith("real_track/") else gp
    for p in (
        BENCH / "real_track" / gp,
        BENCH / "real_track" / "gold" / Path(gp).name,
        BENCH / gp,
    ):
        if p.is_file():
            return p
    return BENCH / "real_track" / "gold" / f"{doc.get('id')}.json"


def extract_tables_payload(
    binary: Path, pdf: Path, preset: str, timeout: float = 120.0
) -> tuple[Optional[dict], Optional[str], float]:
    cli_preset = PRESET_CLI.get(preset, preset)
    cmd = [
        str(binary),
        "extract",
        "--format",
        "json",
        "--tables",
        "--table-preset",
        cli_preset,
        "--no-stitch",
        "--page-tables",
        str(pdf),
    ]
    t0 = time.perf_counter()
    try:
        proc = subprocess.run(
            cmd, capture_output=True, text=True, timeout=timeout, cwd=str(REPO)
        )
    except subprocess.TimeoutExpired:
        return None, "timeout", time.perf_counter() - t0
    elapsed = time.perf_counter() - t0
    if proc.returncode != 0:
        return None, (proc.stderr or proc.stdout or "extract failed")[:500], elapsed
    try:
        return json.loads(proc.stdout), None, elapsed
    except json.JSONDecodeError as e:
        return None, f"json decode: {e}", elapsed


def tables_to_grids(tables: list) -> list[list[list[str]]]:
    """Convert CLI table objects to row grids for metrics.table_accuracy."""
    grids = []
    for t in tables or []:
        if not isinstance(t, dict):
            continue
        # already a grid?
        if isinstance(t.get("cells"), list) and t["cells"] and isinstance(t["cells"][0], list):
            grids.append(t["cells"])
            continue
        rows = int(t.get("rows") or 0)
        cols = int(t.get("cols") or 0)
        if rows <= 0 or cols <= 0:
            # try infer from cell list
            cells_in = t.get("cells") or []
            if not cells_in:
                continue
            max_r = max(int(c.get("row", 0)) for c in cells_in if isinstance(c, dict))
            max_c = max(int(c.get("col", 0)) for c in cells_in if isinstance(c, dict))
            rows, cols = max_r + 1, max_c + 1
        grid = [["" for _ in range(cols)] for _ in range(rows)]
        for c in t.get("cells") or []:
            if not isinstance(c, dict):
                continue
            r, col = int(c.get("row", 0)), int(c.get("col", 0))
            if 0 <= r < rows and 0 <= col < cols:
                grid[r][col] = c.get("text") or ""
        grids.append(grid)
    return grids


def tables_with_bbox(tables: list) -> list[dict]:
    out = []
    for t in tables or []:
        if not isinstance(t, dict):
            continue
        bb = t.get("bbox") or {}
        out.append(
            {
                "page": t.get("page", 0),
                "bbox": {
                    "x0": float(bb.get("x0", 0)),
                    "y0": float(bb.get("y0", 0)),
                    "x1": float(bb.get("x1", 0)),
                    "y1": float(bb.get("y1", 0)),
                },
                "rows": t.get("rows"),
                "cols": t.get("cols"),
                "method": t.get("method"),
            }
        )
    return out


def collect_root_tables(payload: dict) -> list:
    tabs = payload.get("tables")
    if isinstance(tabs, list) and tabs:
        return tabs
    out = []
    for p in payload.get("pages") or []:
        for t in p.get("tables") or []:
            if isinstance(t, dict) and "page" not in t:
                t = {**t, "page": p.get("index", 0)}
            out.append(t)
    return out


def filter_preds_to_gold_pages(pred_tables: list, gold: dict) -> tuple[list, dict]:
    """Keep predictions only on pages that appear in gold (page-local T3).

    Multipage PDFs (e.g. CA WARN) often have one gold table on page 0 while the
    extract returns every page — counting those as FP is unfair for structure eval.
    """
    gold_tabs = gold.get("expected_tables") or []
    pages = set()
    for t in gold_tabs:
        if isinstance(t, dict) and "page" in t:
            pages.add(int(t["page"]))
    if not pages:
        # default: only page 0 when gold is page-local soft structure
        pages = {0}
    filtered = []
    for t in pred_tables:
        if not isinstance(t, dict):
            continue
        p = t.get("page")
        if p is None:
            filtered.append(t)  # keep unpaged
            continue
        if int(p) in pages:
            filtered.append(t)
    meta = {
        "gold_pages": sorted(pages),
        "n_pred_all_pages": len(pred_tables),
        "n_pred_gold_pages": len(filtered),
        "filtered": len(pred_tables) != len(filtered),
    }
    return filtered, meta


def score_doc(gold: dict, pred_tables: list) -> dict[str, Any]:
    pred_tables, page_filter = filter_preds_to_gold_pages(pred_tables, gold)
    grids = tables_to_grids(pred_tables)
    acc = table_accuracy(grids, gold)
    # IoU detection when gold has bboxes
    gold_tabs = gold.get("expected_tables") or []
    gold_for_iou = []
    for t in gold_tabs:
        if isinstance(t, dict) and t.get("bbox"):
            gold_for_iou.append(
                {
                    "page": t.get("page", 0),
                    "bbox": t["bbox"],
                }
            )
    pred_for_iou = tables_with_bbox(pred_tables)
    iou_det = None
    if gold_for_iou and any(p.get("bbox") for p in pred_for_iou):
        try:
            iou_det = table_detection_metrics_iou(pred_for_iou, gold_for_iou, iou_thresh=0.5)
        except Exception as e:
            iou_det = {"error": str(e)}
    return {
        "detection_count": acc.get("detection"),
        "detection_iou": iou_det,
        "structure": acc.get("structure"),
        "cell": acc.get("cell"),
        "per_table": acc.get("per_table"),
        "score_0_100": acc.get("score_0_100"),
        "n_pred": len(pred_tables),
        "n_exp": gold.get("expected_table_count")
        if gold.get("expected_table_count") is not None
        else len(gold_tabs),
        "page_filter": page_filter,
    }


def micro_average(docs: list[dict], key_path: tuple[str, ...]) -> Optional[float]:
    vals = []
    for d in docs:
        cur: Any = d
        for k in key_path:
            if not isinstance(cur, dict):
                cur = None
                break
            cur = cur.get(k)
        if isinstance(cur, (int, float)) and cur is not None:
            vals.append(float(cur))
    if not vals:
        return None
    return sum(vals) / len(vals)


def run_preset(
    binary: Path,
    man: dict,
    preset: str,
    limit: Optional[int],
) -> dict:
    docs_out = []
    for i, doc in enumerate(man.get("documents") or []):
        if limit is not None and i >= limit:
            break
        doc_id = doc.get("id")
        pdf = resolve_pdf(doc.get("pdf") or f"corpus/real/{doc_id}.pdf")
        gold_path = resolve_gold(doc)
        entry: dict[str, Any] = {
            "id": doc_id,
            "pdf": str(pdf.relative_to(REPO)) if pdf.is_file() and pdf.is_relative_to(REPO) else str(pdf),
            "gold": str(gold_path.relative_to(BENCH / "real_track")) if gold_path.is_file() else str(gold_path),
            "struggle": bool(doc.get("struggle")),
            "failure_classes": doc.get("failure_classes") or [],
        }
        if not pdf.is_file():
            entry["status"] = "missing_pdf"
            entry["error"] = str(pdf)
            docs_out.append(entry)
            continue
        if not gold_path.is_file():
            entry["status"] = "missing_gold"
            entry["error"] = str(gold_path)
            docs_out.append(entry)
            continue
        gold = json.loads(gold_path.read_text(encoding="utf-8"))
        payload, err, elapsed = extract_tables_payload(binary, pdf, preset)
        entry["elapsed_s"] = round(elapsed, 3)
        if err or payload is None:
            entry["status"] = "extract_error"
            entry["error"] = err
            docs_out.append(entry)
            continue
        pred = collect_root_tables(payload)
        entry["cli_preset"] = payload.get("table_preset")
        entry["cli_stitch_multipage"] = payload.get("stitch_multipage")
        scores = score_doc(gold, pred)
        entry["status"] = "ok"
        entry["metrics"] = scores
        entry["method_mix"] = [
            {
                "page": t.get("page"),
                "method": t.get("method"),
                "rows": t.get("rows"),
                "cols": t.get("cols"),
            }
            for t in pred
            if isinstance(t, dict)
        ]
        docs_out.append(entry)

    ok_docs = [d for d in docs_out if d.get("status") == "ok"]
    summary = {
        "n_docs": len(docs_out),
        "n_ok": len(ok_docs),
        "n_fail": len(docs_out) - len(ok_docs),
        "micro_cell_f1": micro_average(ok_docs, ("metrics", "cell", "f1")),
        "micro_shape_exact_rate": micro_average(
            ok_docs, ("metrics", "structure", "shape_exact_rate")
        ),
        "micro_det_count_f1": micro_average(
            ok_docs, ("metrics", "detection_count", "f1")
        ),
        "micro_det_iou_f1": micro_average(
            ok_docs, ("metrics", "detection_iou", "f1")
        ),
        "micro_score_0_100": micro_average(ok_docs, ("metrics", "score_0_100")),
        "note": "soft progressive scores; G1 not claimed",
    }
    return {
        "preset": preset,
        "stitch_multipage": STITCH_POLICY,
        "stitch_note": STITCH_NOTE,
        "summary": summary,
        "documents": docs_out,
    }


def main() -> int:
    ap = argparse.ArgumentParser(description="real_structure T3 metrics harness")
    ap.add_argument("--manifest", type=Path, default=MANIFEST_DEFAULT)
    ap.add_argument("--out", type=Path, default=OUT_DEFAULT)
    ap.add_argument(
        "--preset",
        default="auto",
        choices=list(PRESET_CLI.keys()),
        help="table preset (default auto = legacy product)",
    )
    ap.add_argument(
        "--compare",
        action="store_true",
        help="also run engine-v2 and include both in output",
    )
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--build", action="store_true")
    ap.add_argument("--limit", type=int, default=None)
    ap.add_argument(
        "--strict",
        action="store_true",
        help="exit 1 if any doc extract fails or suite empty",
    )
    args = ap.parse_args()

    if not args.manifest.is_file():
        print(f"missing manifest {args.manifest}", file=sys.stderr)
        return 2
    man = json.loads(args.manifest.read_text(encoding="utf-8"))
    docs = man.get("documents") or []
    print(
        f"real_structure: suite={man.get('suite')} n_docs={man.get('n_docs')} "
        f"g1_ready={man.get('g1_ready')} listed={len(docs)}"
    )

    if args.dry_run:
        for d in docs[: args.limit or len(docs)]:
            pdf = resolve_pdf(d.get("pdf") or "")
            gold = resolve_gold(d)
            print(
                f"  {d.get('id')}: pdf={'OK' if pdf.is_file() else 'MISS'} "
                f"gold={'OK' if gold.is_file() else 'MISS'} struggle={d.get('struggle')}"
            )
        print("dry-run OK")
        return 0

    if not docs:
        print("empty structure suite — nothing to score (G1 not ready)")
        RESULTS_DIR.mkdir(parents=True, exist_ok=True)
        empty = {
            "suite": "real_structure",
            "status": "empty",
            "generated_at": datetime.now(timezone.utc).isoformat(),
            "g1_ready": man.get("g1_ready"),
            "summary": {"n_docs": 0},
        }
        args.out.write_text(json.dumps(empty, indent=2), encoding="utf-8")
        return 1 if args.strict else 0

    binary = find_binary()
    if binary is None or args.build:
        ok, msg = try_build_release()
        print(msg)
        if not ok:
            return 2
        binary = find_binary()
    if binary is None:
        print("missing pdfparser binary; pass --build", file=sys.stderr)
        return 2

    runs = [run_preset(binary, man, args.preset, args.limit)]
    if args.compare and args.preset != "engine-v2":
        runs.append(run_preset(binary, man, "engine-v2", args.limit))

    out = {
        "suite": "real_structure",
        "version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "binary": str(binary),
        "manifest": _rel(args.manifest, REPO),
        "g1_ready": bool(man.get("g1_ready")),
        "g1_min_docs": man.get("g1_min_docs"),
        "policy": {
            "stitch_multipage": STITCH_POLICY,
            "stitch_note": STITCH_NOTE,
            "no_auto_gold_promote": True,
            "not_g1_claim": True,
        },
        "runs": runs,
        # convenience: primary run at top level too
        "primary_preset": args.preset,
        "summary": runs[0]["summary"],
        "documents": runs[0]["documents"],
    }
    if len(runs) > 1:
        out["compare"] = {
            r["preset"]: r["summary"] for r in runs
        }

    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(out, indent=2), encoding="utf-8")
    s = runs[0]["summary"]
    print(
        f"wrote {args.out} preset={args.preset} n_ok={s['n_ok']}/{s['n_docs']} "
        f"cell_f1={s.get('micro_cell_f1')} shape={s.get('micro_shape_exact_rate')} "
        f"det_f1={s.get('micro_det_count_f1')}"
    )
    if args.compare and len(runs) > 1:
        s2 = runs[1]["summary"]
        print(
            f"  compare engine-v2 cell_f1={s2.get('micro_cell_f1')} "
            f"shape={s2.get('micro_shape_exact_rate')} det_f1={s2.get('micro_det_count_f1')}"
        )

    if args.strict:
        if s["n_fail"] or s["n_ok"] == 0:
            return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
