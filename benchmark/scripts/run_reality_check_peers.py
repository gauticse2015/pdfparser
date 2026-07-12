#!/usr/bin/env python3
"""Peer bakeoff vs real_structure T3 gold (same metrics as run_real_structure).

Compares pdfparser (auto + engine-v2) to camelot lattice/stream and pdfplumber
on the G1 real_structure corpus. Never promotes gold.

Usage:
  source .venv/bin/activate
  cargo build --release -p pdfparser-cli
  python3 benchmark/scripts/run_reality_check_peers.py
  python3 benchmark/scripts/run_reality_check_peers.py --limit 5
"""
from __future__ import annotations

import argparse
import json
import sys
import time
import traceback
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

SCRIPTS = Path(__file__).resolve().parent
BENCH = SCRIPTS.parent
REPO = BENCH.parent
sys.path.insert(0, str(SCRIPTS))

from metrics import table_accuracy  # noqa: E402
from run_real_structure import (  # noqa: E402
    MANIFEST_DEFAULT,
    RESULTS_DIR,
    extract_tables_payload,
    filter_preds_to_gold_pages,
    find_binary,
    resolve_gold,
    resolve_pdf,
    score_doc,
    tables_to_grids,
    try_build_release,
)

OUT_JSON = RESULTS_DIR / "reality_check_peers_g1.json"
OUT_MD = RESULTS_DIR / "REALITY_CHECK_PEERS.md"


def load_manifest(path: Path) -> dict:
    return json.loads(path.read_text())


def camelot_grids(pdf: Path, pages: str, flavor: str) -> list[list[list[str]]]:
    import camelot

    tables = camelot.read_pdf(str(pdf), pages=pages, flavor=flavor)
    out = []
    for t in tables:
        df = t.df
        grid = [[str(c) if c is not None else "" for c in row] for row in df.values.tolist()]
        # camelot first row is often header already in df
        out.append(grid)
    return out


def plumber_grids(pdf: Path, page_indices: list[int]) -> list[list[list[str]]]:
    import pdfplumber

    out = []
    with pdfplumber.open(str(pdf)) as doc:
        for pi in page_indices:
            if pi < 0 or pi >= len(doc.pages):
                continue
            page = doc.pages[pi]
            found = page.extract_tables() or []
            for tab in found:
                grid = []
                for row in tab or []:
                    grid.append([("" if c is None else str(c)) for c in row])
                if grid:
                    out.append(grid)
    return out


def gold_pages(gold: dict) -> list[int]:
    pages = set()
    for t in gold.get("expected_tables") or []:
        if isinstance(t, dict) and "page" in t:
            pages.add(int(t["page"]))
    return sorted(pages) if pages else [0]


def pages_arg_1based(pages0: list[int]) -> str:
    return ",".join(str(p + 1) for p in pages0)


def score_grids(gold: dict, grids: list[list[list[str]]]) -> dict[str, Any]:
    # Build fake table dicts for page filter compatibility (already page-filtered)
    acc = table_accuracy(grids, gold)
    return {
        "detection_count": acc.get("detection"),
        "structure": acc.get("structure"),
        "cell": acc.get("cell"),
        "score_0_100": acc.get("score_0_100"),
        "n_pred": len(grids),
        "n_exp": gold.get("expected_table_count")
        if gold.get("expected_table_count") is not None
        else len(gold.get("expected_tables") or []),
    }


def cell_f1(m: Optional[dict]) -> Optional[float]:
    if not m:
        return None
    c = m.get("cell") or {}
    return c.get("f1")


def det_f1(m: Optional[dict]) -> Optional[float]:
    if not m:
        return None
    d = m.get("detection_count") or {}
    return d.get("f1")


def shape_rate(m: Optional[dict]) -> Optional[float]:
    if not m:
        return None
    s = m.get("structure") or {}
    return s.get("shape_exact_rate")


def run_pdfparser(binary: Path, pdf: Path, gold: dict, preset: str) -> tuple[Optional[dict], Optional[str], float]:
    payload, err, elapsed = extract_tables_payload(binary, pdf, preset)
    if err or not payload:
        return None, err or "empty", elapsed
    from run_real_structure import collect_root_tables

    tabs = collect_root_tables(payload)
    return score_doc(gold, tabs), None, elapsed


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--manifest", type=Path, default=MANIFEST_DEFAULT)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--build", action="store_true")
    ap.add_argument("--skip-peers", action="store_true")
    args = ap.parse_args()

    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    binary = find_binary()
    if binary is None or args.build:
        ok, msg = try_build_release()
        print(msg, flush=True)
        if not ok:
            return 1
        binary = find_binary()
    if binary is None:
        print("missing pdfparser binary", file=sys.stderr)
        return 1

    man = load_manifest(args.manifest)
    docs = man.get("documents") or man.get("docs") or []
    if args.limit:
        docs = docs[: args.limit]

    libs = ["pdfparser_auto", "pdfparser_engine_v2"]
    if not args.skip_peers:
        libs += ["camelot_lattice", "camelot_stream", "pdfplumber"]

    per_doc: list[dict] = []
    for doc in docs:
        did = doc.get("id") or doc.get("doc_id")
        pdf = resolve_pdf(doc.get("pdf") or "")
        gold_path = resolve_gold(doc)
        if not pdf.is_file() or not gold_path.is_file():
            per_doc.append({"id": did, "error": "missing pdf/gold", "pdf": str(pdf)})
            continue
        gold = json.loads(gold_path.read_text())
        pages = gold_pages(gold)
        row: dict[str, Any] = {
            "id": did,
            "struggle": bool(doc.get("struggle") or gold.get("struggle_tags")),
            "failure_classes": gold.get("failure_classes") or doc.get("failure_classes") or [],
            "pdf": str(pdf),
            "gold_pages": pages,
            "metrics": {},
            "errors": {},
            "elapsed_s": {},
        }
        # pdfparser presets
        for preset, key in [("auto", "pdfparser_auto"), ("engine-v2", "pdfparser_engine_v2")]:
            try:
                m, err, el = run_pdfparser(binary, pdf, gold, preset)
                row["elapsed_s"][key] = round(el, 3)
                if err:
                    row["errors"][key] = err
                else:
                    row["metrics"][key] = m
            except Exception as e:
                row["errors"][key] = f"{type(e).__name__}: {e}"

        if not args.skip_peers:
            pstr = pages_arg_1based(pages)
            # camelot lattice
            for flavor, key in [("lattice", "camelot_lattice"), ("stream", "camelot_stream")]:
                t0 = time.perf_counter()
                try:
                    grids = camelot_grids(pdf, pstr, flavor)
                    row["metrics"][key] = score_grids(gold, grids)
                except Exception as e:
                    row["errors"][key] = f"{type(e).__name__}: {e}"
                row["elapsed_s"][key] = round(time.perf_counter() - t0, 3)
            # pdfplumber
            t0 = time.perf_counter()
            try:
                grids = plumber_grids(pdf, pages)
                row["metrics"]["pdfplumber"] = score_grids(gold, grids)
            except Exception as e:
                row["errors"]["pdfplumber"] = f"{type(e).__name__}: {e}"
            row["elapsed_s"]["pdfplumber"] = round(time.perf_counter() - t0, 3)

        per_doc.append(row)
        cf = cell_f1(row["metrics"].get("pdfparser_auto"))
        print(
            f"  {did}: auto_cell={cf} peers_ok={len(row['metrics'])} err={list(row['errors'])}",
            flush=True,
        )

    # Aggregate equal-weight means
    def mean_for(lib: str, getter) -> Optional[float]:
        vals = []
        for d in per_doc:
            m = (d.get("metrics") or {}).get(lib)
            if not m:
                continue
            v = getter(m)
            if v is not None:
                vals.append(float(v))
        if not vals:
            return None
        return sum(vals) / len(vals)

    summary = {}
    for lib in libs:
        summary[lib] = {
            "n_scored": sum(1 for d in per_doc if lib in (d.get("metrics") or {})),
            "n_error": sum(1 for d in per_doc if lib in (d.get("errors") or {})),
            "mean_cell_f1": mean_for(lib, cell_f1),
            "mean_det_f1": mean_for(lib, det_f1),
            "mean_shape": mean_for(lib, shape_rate),
            "mean_score_0_100": mean_for(lib, lambda m: m.get("score_0_100")),
        }

    # Wins: best cell F1 among libs that scored
    wins = {lib: 0 for lib in libs}
    for d in per_doc:
        best_lib = None
        best_v = -1.0
        for lib in libs:
            m = (d.get("metrics") or {}).get(lib)
            v = cell_f1(m)
            if v is None:
                continue
            if v > best_v + 1e-9:
                best_v = v
                best_lib = lib
        if best_lib:
            wins[best_lib] += 1
            d["best_cell_lib"] = best_lib
            d["best_cell_f1"] = best_v

    for lib in libs:
        summary[lib]["wins_best_cell"] = wins.get(lib, 0)

    payload = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "manifest": str(args.manifest),
        "binary": str(binary),
        "policy": "same gold + metrics as run_real_structure; stitch off; gold pages only",
        "libraries": libs,
        "summary": summary,
        "documents": per_doc,
    }
    OUT_JSON.write_text(json.dumps(payload, indent=2))
    print(f"wrote {OUT_JSON}", flush=True)

    # Markdown
    lines = [
        "# Reality check — G1 real_structure vs peers",
        "",
        f"Generated: `{payload['generated_at']}`",
        "",
        "Shared **human/vision T3 gold**. Same metrics as `run_real_structure` (grids).",
        "",
        "## Mean scores (equal-weight per doc)",
        "",
        "| library | mean cell F1 | mean det F1 | mean shape | mean score | wins (best cell F1) | n_err |",
        "|---------|-------------:|------------:|-----------:|-----------:|--------------------:|------:|",
    ]
    for lib in libs:
        s = summary[lib]
        def fmt(x):
            return f"{x:.4f}" if isinstance(x, float) else "—"
        lines.append(
            f"| **{lib}** | {fmt(s['mean_cell_f1'])} | {fmt(s['mean_det_f1'])} | "
            f"{fmt(s['mean_shape'])} | {fmt(s['mean_score_0_100'])} | {s['wins_best_cell']} | {s['n_error']} |"
        )
    lines += [
        "",
        "## Per-doc cell F1",
        "",
        "| id | S | " + " | ".join(libs) + " | best |",
        "|----|---|" + "|".join(["----------:" for _ in libs]) + "|------|",
    ]
    for d in per_doc:
        cells = []
        for lib in libs:
            m = (d.get("metrics") or {}).get(lib)
            if lib in (d.get("errors") or {}):
                cells.append("ERR")
            else:
                v = cell_f1(m)
                cells.append(f"{v:.3f}" if v is not None else "—")
        lines.append(
            f"| `{d['id']}` | {d.get('struggle')} | "
            + " | ".join(cells)
            + f" | {d.get('best_cell_lib', '—')} |"
        )
    lines += [
        "",
        f"JSON: `{OUT_JSON.relative_to(REPO)}`",
        "",
    ]
    OUT_MD.write_text("\n".join(lines))
    print(f"wrote {OUT_MD}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
