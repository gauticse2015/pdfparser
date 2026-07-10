#!/usr/bin/env python3
"""Run extractors and compute quantitative accuracy scoreboard.

Outputs:
  results/accuracy_results.json   — per library × doc accuracy blocks
  results/accuracy_scoreboard.tsv — flat table
  results/accuracy_scoreboard.md  — human-readable scoreboard
  docs/accuracy-scoreboard.md     — copy for product docs
"""
from __future__ import annotations

import importlib.util
import json
import sys
import time
import traceback
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = Path(__file__).resolve().parent
RESULTS = ROOT / "results"
GT_DIR = ROOT / "ground_truth"
RESULTS.mkdir(parents=True, exist_ok=True)

# load sibling modules
sys.path.insert(0, str(SCRIPTS))
import metrics as M  # noqa: E402

# load run_benchmark adapters without executing main
spec = importlib.util.spec_from_file_location("run_benchmark", SCRIPTS / "run_benchmark.py")
rb = importlib.util.module_from_spec(spec)
sys.modules["run_benchmark"] = rb  # required for dataclasses on Python 3.14
spec.loader.exec_module(rb)


def score_extraction(res: rb.ExtractResult, gold: dict) -> dict:
    text_m = M.text_accuracy(res.text or "", gold)
    table_m = M.table_accuracy(res.tables or [], gold)
    obj_m = M.objects_accuracy(
        image_count=res.image_count,
        links=res.links or [],
        form_fields=res.form_fields or [],
        outline=res.outline or [],
        gold=gold,
    )
    overall = M.overall_accuracy(text_m, table_m, obj_m, gold)
    return {
        "text": text_m,
        "tables": table_m,
        "objects": obj_m,
        "overall": overall,
    }


def flat_row(lib: str, doc_id: str, gold: dict, res: rb.ExtractResult, acc: dict, wall_ms: float) -> dict:
    t = acc["text"]
    tb = acc["tables"]
    o = acc["objects"]
    det = tb.get("detection") or {}
    st = tb.get("structure") or {}
    cell = tb.get("cell") or {}
    tok = t.get("token") or {}
    return {
        "library": lib,
        "doc_id": doc_id,
        "tier": gold.get("tier"),
        "category": gold.get("category"),
        "success": res.success,
        "wall_ms": round(wall_ms, 2),
        "overall_score": _r(acc["overall"].get("score_0_100")),
        "text_score": _r(t.get("score_0_100")),
        "text_token_f1": _r(tok.get("f1")),
        "text_token_recall": _r(tok.get("recall")),
        "text_cer": _r(t.get("cer")),
        "text_wer": _r(t.get("wer")),
        "text_similarity": _r(t.get("normalized_similarity")),
        "table_score": _r(tb.get("score_0_100")),
        "table_detect_f1": _r(det.get("f1")),
        "table_detect_precision": _r(det.get("precision")),
        "table_detect_recall": _r(det.get("recall")),
        "table_expected": det.get("expected"),
        "table_predicted": det.get("predicted") if not det.get("skipped") else tb.get("predicted_table_count"),
        "table_shape_exact_rate": _r(st.get("shape_exact_rate")),
        "table_row_accuracy": _r(st.get("row_accuracy")),
        "table_col_accuracy": _r(st.get("col_accuracy")),
        "table_cell_f1": _r(cell.get("f1")),
        "table_cell_precision": _r(cell.get("precision")),
        "table_cell_recall": _r(cell.get("recall")),
        "table_content_token_recall": _r((tb.get("content_tokens") or {}).get("recall")),
        "table_has_grid_gold": tb.get("has_grid_gold"),
        "objects_score": _r(o.get("score_0_100")),
        "images_score": _r((o.get("images") or {}).get("score")),
        "images_exact": (o.get("images") or {}).get("exact"),
        "links_f1": _r((o.get("links") or {}).get("f1")),
        "forms_f1": _r((o.get("forms") or {}).get("f1")),
        "outline_f1": _r((o.get("outline") or {}).get("f1")),
        "error": res.error,
    }


def _r(x, nd=4):
    if x is None:
        return None
    try:
        return round(float(x), nd)
    except Exception:
        return None


def aggregate(rows: list[dict]) -> dict:
    by_lib = {}
    for r in rows:
        by_lib.setdefault(r["library"], []).append(r)

    def avg(vals):
        vals = [v for v in vals if v is not None]
        return sum(vals) / len(vals) if vals else None

    summary = {}
    for lib, rs in by_lib.items():
        ok = [r for r in rs if r["success"]]
        summary[lib] = {
            "n_docs": len(rs),
            "n_success": len(ok),
            "overall_score_mean": _r(avg([r["overall_score"] for r in ok])),
            "text_score_mean": _r(avg([r["text_score"] for r in ok])),
            "text_token_f1_mean": _r(avg([r["text_token_f1"] for r in ok])),
            "text_cer_mean": _r(avg([r["text_cer"] for r in ok])),
            "table_score_mean": _r(avg([r["table_score"] for r in ok])),
            "table_detect_f1_mean": _r(avg([r["table_detect_f1"] for r in ok])),
            "table_cell_f1_mean": _r(avg([r["table_cell_f1"] for r in ok])),
            "table_shape_exact_rate_mean": _r(avg([r["table_shape_exact_rate"] for r in ok])),
            "objects_score_mean": _r(avg([r["objects_score"] for r in ok])),
            "wall_ms_mean": _r(avg([r["wall_ms"] for r in ok]), 2),
            # tier splits
            "by_tier": {},
        }
        for tier in ("basic", "stress", "real"):
            tr = [r for r in ok if r.get("tier") == tier]
            if not tr:
                continue
            summary[lib]["by_tier"][tier] = {
                "n": len(tr),
                "overall_score_mean": _r(avg([r["overall_score"] for r in tr])),
                "text_score_mean": _r(avg([r["text_score"] for r in tr])),
                "table_score_mean": _r(avg([r["table_score"] for r in tr])),
                "table_detect_f1_mean": _r(avg([r["table_detect_f1"] for r in tr])),
                "table_cell_f1_mean": _r(avg([r["table_cell_f1"] for r in tr])),
                "objects_score_mean": _r(avg([r["objects_score"] for r in tr])),
            }
        # docs with grid gold only
        grid = [r for r in ok if r.get("table_has_grid_gold")]
        summary[lib]["grid_gold_subset"] = {
            "n": len(grid),
            "table_cell_f1_mean": _r(avg([r["table_cell_f1"] for r in grid])),
            "table_shape_exact_rate_mean": _r(avg([r["table_shape_exact_rate"] for r in grid])),
            "table_detect_f1_mean": _r(avg([r["table_detect_f1"] for r in grid])),
            "table_score_mean": _r(avg([r["table_score"] for r in grid])),
        }
    return summary


def render_md(summary: dict, rows: list[dict]) -> str:
    libs = sorted(summary.keys())
    lines = []
    lines.append("# PDF Parser Accuracy Scoreboard\n")
    lines.append(f"**Generated:** auto · **Docs scored:** {len({r['doc_id'] for r in rows})} · **Libraries:** {', '.join(libs)}\n")
    lines.append("\n## Metric definitions\n")
    lines.append("| Metric | Meaning | Range |\n|--------|---------|-------|\n")
    lines.append("| **overall_score** | Weighted blend of text / tables / objects (per-doc weights in GT) | 0–100 |\n")
    lines.append("| **text_token_f1** | F1 on required substrings (`must_contain`) | 0–1 |\n")
    lines.append("| **text_cer** | Character Error Rate vs `reference_text` (lower better) | 0–1+ |\n")
    lines.append("| **table_detect_f1** | F1 on table *count* vs expected | 0–1 |\n")
    lines.append("| **table_row/col_accuracy** | Fraction of gold tables with exact row/col counts | 0–1 |\n")
    lines.append("| **table_cell_f1** | Micro F1 of normalized cell text after table alignment | 0–1 |\n")
    lines.append("| **images/forms/links/outline** | Count or set accuracy vs gold | 0–1 |\n")
    lines.append("\nScores are only computed when gold exists for that component. ")
    lines.append("Synthetic docs have full table grids; real docs often have count-only gold.\n")

    lines.append("\n## Overall leaderboard (mean over successful runs)\n")
    lines.append("| Library | overall | text | text F1 | CER↓ | table | detect F1 | cell F1 | objects | ms |\n")
    lines.append("|---------|--------:|-----:|--------:|-----:|------:|----------:|--------:|--------:|---:|\n")
    # sort by overall
    ranked = sorted(libs, key=lambda L: summary[L].get("overall_score_mean") or -1, reverse=True)
    for lib in ranked:
        s = summary[lib]
        lines.append(
            f"| {lib} | {_fmt(s['overall_score_mean'])} | {_fmt(s['text_score_mean'])} | "
            f"{_fmt(s['text_token_f1_mean'])} | {_fmt(s['text_cer_mean'])} | "
            f"{_fmt(s['table_score_mean'])} | {_fmt(s['table_detect_f1_mean'])} | "
            f"{_fmt(s['table_cell_f1_mean'])} | {_fmt(s['objects_score_mean'])} | "
            f"{_fmt(s['wall_ms_mean'],1)} |\n"
        )

    lines.append("\n## Grid-gold subset (synthetic tables with full cell grids)\n")
    lines.append("Best apples-to-apples table quality comparison.\n\n")
    lines.append("| Library | n | detect F1 | shape exact | cell F1 | table score |\n")
    lines.append("|---------|--:|----------:|------------:|--------:|------------:|\n")
    for lib in ranked:
        g = summary[lib]["grid_gold_subset"]
        lines.append(
            f"| {lib} | {g['n']} | {_fmt(g['table_detect_f1_mean'])} | "
            f"{_fmt(g['table_shape_exact_rate_mean'])} | {_fmt(g['table_cell_f1_mean'])} | "
            f"{_fmt(g['table_score_mean'])} |\n"
        )

    lines.append("\n## By tier\n")
    for tier in ("basic", "stress", "real"):
        lines.append(f"\n### {tier}\n\n")
        lines.append("| Library | n | overall | text | table | detect F1 | cell F1 | objects |\n")
        lines.append("|---------|--:|--------:|-----:|------:|----------:|--------:|--------:|\n")
        for lib in ranked:
            t = summary[lib]["by_tier"].get(tier)
            if not t:
                continue
            lines.append(
                f"| {lib} | {t['n']} | {_fmt(t['overall_score_mean'])} | {_fmt(t['text_score_mean'])} | "
                f"{_fmt(t['table_score_mean'])} | {_fmt(t['table_detect_f1_mean'])} | "
                f"{_fmt(t['table_cell_f1_mean'])} | {_fmt(t['objects_score_mean'])} |\n"
            )

    lines.append("\n## Per-document matrix (overall score)\n\n")
    docs = sorted({r["doc_id"] for r in rows if "#" not in r["doc_id"]})
    lines.append("| Doc | " + " | ".join(libs) + " |\n")
    lines.append("|-----|" + "|".join(["-----:"] * len(libs)) + "|\n")
    idx = {(r["library"], r["doc_id"]): r for r in rows}
    for d in docs:
        cells = []
        for lib in libs:
            r = idx.get((lib, d))
            if not r or not r["success"]:
                cells.append("FAIL" if r and not r["success"] else "—")
            else:
                cells.append(_fmt(r["overall_score"], 1) if r["overall_score"] is not None else "—")
        lines.append(f"| `{d}` | " + " | ".join(cells) + " |\n")

    lines.append("\n## Per-document table cell F1 (grid gold only)\n\n")
    lines.append("| Doc | " + " | ".join(libs) + " |\n")
    lines.append("|-----|" + "|".join(["-----:"] * len(libs)) + "|\n")
    for d in docs:
        # only if any has grid
        any_grid = any((idx.get((lib, d)) or {}).get("table_has_grid_gold") for lib in libs)
        if not any_grid:
            continue
        cells = []
        for lib in libs:
            r = idx.get((lib, d))
            if not r or not r["success"]:
                cells.append("—")
            else:
                cells.append(_fmt(r["table_cell_f1"], 3) if r["table_cell_f1"] is not None else "—")
        lines.append(f"| `{d}` | " + " | ".join(cells) + " |\n")

    lines.append("\n## How to compare `pdfparser` later\n")
    lines.append("1. Add an adapter returning the same extract fields.\n")
    lines.append("2. Re-run `python benchmark/scripts/run_accuracy_benchmark.py`.\n")
    lines.append("3. Read **overall_score**, **table_cell_f1** (grid subset), **table_detect_f1**, **text_cer**.\n")
    lines.append("4. Target bars (from competitors on this harness):\n")
    lines.append("   - text_token_f1 ≥ best competitor on basic/stress\n")
    lines.append("   - table_cell_f1 ≥ pdfplumber on grid-gold subset\n")
    lines.append("   - table_detect_f1 high on statements with multi-page tolerance\n")
    lines.append("   - low false positives on IRS/NIST (detect F1 with expected 0)\n")

    lines.append("\n---\n*Machine-generated scoreboard. Re-run after corpus/gold changes.*\n")
    return "".join(lines)


def _fmt(x, nd=3):
    if x is None:
        return "—"
    if isinstance(x, float):
        return f"{x:.{nd}f}"
    return str(x)


def main() -> None:
    gts = []
    for p in sorted(GT_DIR.glob("*.json")):
        gts.append(json.loads(p.read_text(encoding="utf-8")))

    detailed = []
    flat = []

    for gold in gts:
        doc_id = gold["id"]
        pdf_path = rb.resolve_pdf(gold)
        if not pdf_path:
            print(f"MISSING {doc_id}")
            continue
        print(f"=== {doc_id} ===")
        for lib, fn in rb.ADAPTERS.items():
            # encrypted: score the with-password path for accuracy (capability with secrets)
            password = gold.get("password") if gold.get("category") == "encrypted_password" else None
            t0 = time.perf_counter()
            try:
                res = fn(pdf_path, gold, password=password)
                res.success = True
            except Exception as e:
                res = rb.ExtractResult(
                    library=lib,
                    doc_id=doc_id,
                    success=False,
                    error=f"{type(e).__name__}: {e}",
                    notes=[traceback.format_exc(limit=2)],
                )
            wall = (time.perf_counter() - t0) * 1000
            res.wall_ms = wall
            if res.success:
                acc = score_extraction(res, gold)
            else:
                acc = {
                    "text": {"score_0_100": 0.0},
                    "tables": {"score_0_100": 0.0},
                    "objects": {"score_0_100": 0.0},
                    "overall": {"score_0_100": 0.0},
                }
            row = flat_row(lib, doc_id, gold, res, acc, wall)
            flat.append(row)
            detailed.append(
                {
                    **row,
                    "accuracy_detail": acc,
                    "supports": res.supports,
                    "notes": res.notes,
                }
            )
            print(
                f"  {lib:12} overall={row['overall_score']} textF1={row['text_token_f1']} "
                f"detF1={row['table_detect_f1']} cellF1={row['table_cell_f1']} "
                f"obj={row['objects_score']} {wall:.0f}ms"
            )

    summary = aggregate(flat)
    out = {
        "benchmark": "pdfparser-accuracy-v1",
        "metric_definitions": {
            "text_token_f1": "F1 over must_contain substrings",
            "text_cer": "levenshtein(ref,hyp)/len(ref) on reference_text",
            "table_detect_f1": "count-based TP/FP/FN on number of tables",
            "table_cell_f1": "aligned cell text micro-F1 vs gold grids",
            "table_shape_exact_rate": "fraction of gold tables with exact RxC",
            "overall_score": "weighted 0-100 per GT weights",
        },
        "summary": summary,
        "rows": flat,
        "detailed": detailed,
    }
    (RESULTS / "accuracy_results.json").write_text(json.dumps(out, indent=2), encoding="utf-8")

    # TSV
    cols = list(flat[0].keys()) if flat else []
    tsv_lines = ["\t".join(cols)]
    for r in flat:
        tsv_lines.append("\t".join("" if r[c] is None else str(r[c]) for c in cols))
    (RESULTS / "accuracy_scoreboard.tsv").write_text("\n".join(tsv_lines) + "\n", encoding="utf-8")

    md = render_md(summary, flat)
    (RESULTS / "accuracy_scoreboard.md").write_text(md, encoding="utf-8")
    docs = ROOT.parent / "docs" / "accuracy-scoreboard.md"
    docs.write_text(md, encoding="utf-8")

    print("\n==== SUMMARY ====")
    print(json.dumps(summary, indent=2))
    print(f"\nWrote {RESULTS / 'accuracy_results.json'}")
    print(f"Wrote {RESULTS / 'accuracy_scoreboard.md'}")
    print(f"Wrote {docs}")


if __name__ == "__main__":
    main()
