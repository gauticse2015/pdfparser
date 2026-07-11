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
from typing import Callable

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
        for tier in (
            "basic",
            "stress",
            "hard",
            "hard_precision",
            "hard_sensing",
            "compete",
            "compete_hard",
            "compete_real",
            "real",
        ):
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
                "table_shape_exact_rate_mean": _r(avg([r["table_shape_exact_rate"] for r in tr])),
                "table_row_accuracy_mean": _r(avg([r["table_row_accuracy"] for r in tr])),
                "table_col_accuracy_mean": _r(avg([r["table_col_accuracy"] for r in tr])),
                "objects_score_mean": _r(avg([r["objects_score"] for r in tr])),
            }
        # docs with grid gold only
        grid = [r for r in ok if r.get("table_has_grid_gold")]
        summary[lib]["grid_gold_subset"] = {
            "n": len(grid),
            "table_cell_f1_mean": _r(avg([r["table_cell_f1"] for r in grid])),
            "table_shape_exact_rate_mean": _r(avg([r["table_shape_exact_rate"] for r in grid])),
            "table_detect_f1_mean": _r(avg([r["table_detect_f1"] for r in grid])),
            "table_row_accuracy_mean": _r(avg([r["table_row_accuracy"] for r in grid])),
            "table_col_accuracy_mean": _r(avg([r["table_col_accuracy"] for r in grid])),
            "table_score_mean": _r(avg([r["table_score"] for r in grid])),
        }
        # top-level row/col for compete-style reporting
        summary[lib]["table_row_accuracy_mean"] = _r(avg([r["table_row_accuracy"] for r in ok]))
        summary[lib]["table_col_accuracy_mean"] = _r(avg([r["table_col_accuracy"] for r in ok]))
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
    for tier in ("basic", "stress", "hard", "real"):
        lines.append(f"\n### {tier}\n\n")
        lines.append("| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |\n")
        lines.append("|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|\n")
        for lib in ranked:
            t = summary[lib]["by_tier"].get(tier)
            if not t:
                continue
            lines.append(
                f"| {lib} | {t['n']} | {_fmt(t['overall_score_mean'])} | {_fmt(t['text_score_mean'])} | "
                f"{_fmt(t['table_score_mean'])} | {_fmt(t['table_detect_f1_mean'])} | "
                f"{_fmt(t['table_cell_f1_mean'])} | {_fmt(t.get('table_shape_exact_rate_mean'))} | "
                f"{_fmt(t['objects_score_mean'])} |\n"
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


def load_suite_filter(suite: str | None, tier: str | None):
    """Filter gold records by suite policy and/or tier.

    Suites are defined in corpus/suites.json. ICDAR is never part of regression.
    """
    suites_path = ROOT / "corpus" / "suites.json"
    suites = {}
    if suites_path.exists():
        suites = json.loads(suites_path.read_text(encoding="utf-8")).get("suites", {})

    def ok(gold: dict) -> bool:
        g_tier = gold.get("tier") or "basic"
        if gold.get("icdar_derived") is True:
            return False  # hard ban
        if tier and g_tier != tier:
            return False
        if not suite:
            return True
        if suite == "all":
            return True
        if suite == "regression":
            # synthetic-focused regression; include hard/basic/stress; skip real by default
            return g_tier in ("basic", "stress", "hard") and gold.get("source", "synthetic") != "icdar"
        if suite == "regression_with_real":
            return g_tier in ("basic", "stress", "hard", "real")
        if suite == "regression_hard":
            return g_tier == "hard" or gold.get("suite") == "regression_hard"
        if suite == "regression_precision":
            return (
                g_tier == "hard_precision"
                or gold.get("suite") == "regression_precision"
            )
        if suite == "regression_sensing":
            return (
                g_tier == "hard_sensing"
                or gold.get("suite") == "regression_sensing"
            )
        if suite == "regression_compete":
            return (
                g_tier in ("compete", "compete_hard", "compete_real")
                or gold.get("suite") == "regression_compete"
            )
        if suite == "regression_compete_hard":
            return g_tier == "compete_hard"
        if suite == "regression_grid_gold":
            return bool(gold.get("expected_tables")) and g_tier in (
                "basic",
                "stress",
                "hard",
                "hard_precision",
            )
        if suite == "regression_basic_stress":
            return g_tier in ("basic", "stress")
        if suite in suites:
            spec = suites[suite]
            if spec.get("external"):
                print(f"Suite {suite} is external-only; not loaded from corpus.")
                return False
            ids = spec.get("document_ids")
            if ids is not None:
                return gold.get("id") in ids
            tiers = spec.get("tiers")
            if tiers:
                return g_tier in tiers
        # unknown suite name: treat as tier name
        return g_tier == suite

    return ok


def probe_available_adapters(requested: list[str] | None = None) -> dict[str, Callable]:
    """Return adapters that import/run at least once; drop tabula without JRE."""
    names = requested or list(rb.ADAPTERS.keys())
    out = {}
    for name in names:
        if name not in rb.ADAPTERS:
            print(f"WARN unknown library {name}; skip")
            continue
        if name == "tabula":
            # Tabula needs a real JRE; macOS stub java fails
            import subprocess

            try:
                p = subprocess.run(
                    ["java", "-version"], capture_output=True, text=True, timeout=10
                )
                err = (p.stderr or "") + (p.stdout or "")
                if p.returncode != 0 or "Unable to locate a Java Runtime" in err:
                    print("SKIP tabula (no working JRE)")
                    continue
            except Exception as e:
                print(f"SKIP tabula ({e})")
                continue
        out[name] = rb.ADAPTERS[name]
    return out


def render_scoreboard_md(
    summary: dict,
    rows: list[dict],
    *,
    suite: str,
    libs: list[str],
    n_docs: int,
) -> str:
    """Richer scoreboard: overall + table quality + hard tier + per-doc matrix."""
    # Executive summary first (what we care about for table engine improvement)
    ranked_table = sorted(
        libs,
        key=lambda L: (
            summary.get(L, {}).get("table_cell_f1_mean") is not None,
            summary.get(L, {}).get("table_cell_f1_mean") or -1,
            summary.get(L, {}).get("table_detect_f1_mean") or -1,
        ),
        reverse=True,
    )
    ranked_overall = sorted(
        libs,
        key=lambda L: summary.get(L, {}).get("overall_score_mean") or -1,
        reverse=True,
    )
    exec_lines = []
    exec_lines.append("# PDF Parser Accuracy Scoreboard\n\n")
    exec_lines.append(
        f"**Suite:** `{suite}` · **Docs:** {n_docs} · **ICDAR excluded** (competitive-only)\n\n"
    )
    exec_lines.append("## Executive summary — table quality\n\n")
    exec_lines.append(
        "Primary ranking for table-engine work (mean over docs that produce table metrics):\n\n"
    )
    exec_lines.append(
        "| Rank | Library | cell F1 | detect F1 | shape exact | overall | ms |\n"
    )
    exec_lines.append(
        "|-----:|---------|--------:|----------:|------------:|--------:|---:|\n"
    )
    for i, lib in enumerate(ranked_table, 1):
        s = summary.get(lib) or {}
        mark = " ← **ours**" if lib == "pdfparser" else ""
        exec_lines.append(
            f"| {i} | **{lib}**{mark} | {_fmt(s.get('table_cell_f1_mean'))} | "
            f"{_fmt(s.get('table_detect_f1_mean'))} | {_fmt(s.get('table_shape_exact_rate_mean'))} | "
            f"{_fmt(s.get('overall_score_mean'))} | {_fmt(s.get('wall_ms_mean'), 1)} |\n"
        )
    hard_any = any((summary.get(L) or {}).get("by_tier", {}).get("hard") for L in libs)
    if hard_any:
        exec_lines.append("\n### Hard tier only (structure stress)\n\n")
        exec_lines.append(
            "| Rank | Library | cell F1 | detect F1 | shape exact | overall |\n"
        )
        exec_lines.append(
            "|-----:|---------|--------:|----------:|------------:|--------:|\n"
        )
        hard_rank = sorted(
            [L for L in libs if (summary.get(L) or {}).get("by_tier", {}).get("hard")],
            key=lambda L: (summary[L]["by_tier"]["hard"].get("table_cell_f1_mean") or -1),
            reverse=True,
        )
        for i, lib in enumerate(hard_rank, 1):
            t = summary[lib]["by_tier"]["hard"]
            mark = " ← **ours**" if lib == "pdfparser" else ""
            exec_lines.append(
                f"| {i} | **{lib}**{mark} | {_fmt(t.get('table_cell_f1_mean'))} | "
                f"{_fmt(t.get('table_detect_f1_mean'))} | {_fmt(t.get('table_shape_exact_rate_mean'))} | "
                f"{_fmt(t.get('overall_score_mean'))} |\n"
            )
    exec_lines.append("\n### Overall product score (text+tables+objects)\n\n")
    exec_lines.append("| Rank | Library | overall | text F1 | table cell F1 | ms |\n")
    exec_lines.append("|-----:|---------|--------:|--------:|--------------:|---:|\n")
    for i, lib in enumerate(ranked_overall, 1):
        s = summary.get(lib) or {}
        mark = " ← **ours**" if lib == "pdfparser" else ""
        exec_lines.append(
            f"| {i} | **{lib}**{mark} | {_fmt(s.get('overall_score_mean'))} | "
            f"{_fmt(s.get('text_token_f1_mean'))} | {_fmt(s.get('table_cell_f1_mean'))} | "
            f"{_fmt(s.get('wall_ms_mean'), 1)} |\n"
        )
    exec_lines.append(
        "\n> Table-primary tools (`camelot_*`, `img2table`) get lower **overall** when "
        "outside-table tokens are missing — judge them on **cell F1 / detect F1**.\n"
    )
    exec_lines.append("\n---\n\n")

    base = render_md(summary, rows)
    # Drop duplicate H1 from base
    if base.startswith("# "):
        base = base.split("\n", 1)[1] if "\n" in base else base

    lines = []
    lines.append(f"\n## Run metadata\n\n")
    lines.append(f"- **Suite:** `{suite}` (ICDAR never included in regression)\n")
    lines.append(f"- **Documents scored:** {n_docs}\n")
    lines.append(f"- **Libraries:** {', '.join(libs)}\n")
    lines.append(
        "- **Note:** `camelot_*` / `img2table` / `tabula` are table-primary; "
        "text scores use cell-concatenated text only (outside-table tokens may miss).\n"
    )
    lines.append("- **Tabula:** skipped when no working JRE is installed.\n")

    # Table-quality leaderboard (primary for product table work)
    lines.append("\n## Table-quality leaderboard (mean over docs with table gold)\n\n")
    lines.append(
        "| Rank | Library | detect F1 | shape exact | cell F1 | table score | ms |\n"
    )
    lines.append("|-----:|---------|----------:|------------:|--------:|------------:|---:|\n")
    ranked = sorted(
        libs,
        key=lambda L: (
            summary.get(L, {}).get("table_cell_f1_mean") is not None,
            summary.get(L, {}).get("table_cell_f1_mean") or -1,
            summary.get(L, {}).get("table_detect_f1_mean") or -1,
        ),
        reverse=True,
    )
    for i, lib in enumerate(ranked, 1):
        s = summary.get(lib) or {}
        lines.append(
            f"| {i} | **{lib}** | {_fmt(s.get('table_detect_f1_mean'))} | "
            f"{_fmt(s.get('table_shape_exact_rate_mean'))} | {_fmt(s.get('table_cell_f1_mean'))} | "
            f"{_fmt(s.get('table_score_mean'))} | {_fmt(s.get('wall_ms_mean'), 1)} |\n"
        )

    # Hard tier focus
    lines.append("\n## Hard tier (structure stress 50–62)\n\n")
    lines.append(
        "| Library | n | detect F1 | shape exact | cell F1 | table score | overall |\n"
    )
    lines.append(
        "|---------|--:|----------:|------------:|--------:|------------:|--------:|\n"
    )
    for lib in ranked:
        t = (summary.get(lib) or {}).get("by_tier", {}).get("hard")
        if not t:
            continue
        lines.append(
            f"| {lib} | {t['n']} | {_fmt(t.get('table_detect_f1_mean'))} | "
            f"{_fmt(t.get('table_shape_exact_rate_mean'))} | {_fmt(t.get('table_cell_f1_mean'))} | "
            f"{_fmt(t.get('table_score_mean'))} | {_fmt(t.get('overall_score_mean'))} |\n"
        )

    # Per-doc detect F1 matrix for hard docs
    hard_docs = sorted({r["doc_id"] for r in rows if r.get("tier") == "hard"})
    if hard_docs:
        lines.append("\n## Hard suite — per-doc table detect F1\n\n")
        lines.append("| Doc | " + " | ".join(libs) + " |\n")
        lines.append("|-----|" + "|".join(["-----:"] * len(libs)) + "|\n")
        idx = {(r["library"], r["doc_id"]): r for r in rows}
        for d in hard_docs:
            cells = []
            for lib in libs:
                r = idx.get((lib, d))
                if not r or not r.get("success"):
                    cells.append("FAIL")
                else:
                    cells.append(_fmt(r.get("table_detect_f1"), 2))
            lines.append(f"| `{d}` | " + " | ".join(cells) + " |\n")

        lines.append("\n## Hard suite — per-doc table cell F1\n\n")
        lines.append("| Doc | " + " | ".join(libs) + " |\n")
        lines.append("|-----|" + "|".join(["-----:"] * len(libs)) + "|\n")
        for d in hard_docs:
            cells = []
            for lib in libs:
                r = idx.get((lib, d))
                if not r or not r.get("success"):
                    cells.append("FAIL")
                else:
                    v = r.get("table_cell_f1")
                    cells.append(_fmt(v, 3) if v is not None else "—")
            lines.append(f"| `{d}` | " + " | ".join(cells) + " |\n")

    lines.append(
        "\n## How to re-run\n\n"
        "```bash\n"
        "cargo build --release -p pdfparser-cli\n"
        "source .venv/bin/activate\n"
        "python benchmark/scripts/run_accuracy_benchmark.py --suite regression\n"
        "python benchmark/scripts/run_accuracy_benchmark.py --suite regression_hard\n"
        "```\n"
        "\nCompetitive ICDAR (external, not this scoreboard): "
        "see `docs/camelot-comparison-replication.md`.\n"
    )
    return "".join(exec_lines) + base + "".join(lines)


def main() -> None:
    import argparse

    ap = argparse.ArgumentParser(description="Accuracy scoreboard (regression corpus; no ICDAR)")
    ap.add_argument(
        "--suite",
        default="regression",
        help=(
            "Suite filter: regression (default, basic+stress+hard synthetic), "
            "regression_hard, regression_grid_gold, regression_basic_stress, "
            "regression_with_real, all"
        ),
    )
    ap.add_argument("--tier", default=None, help="Optional single tier override (basic|stress|hard|real)")
    ap.add_argument(
        "--libs",
        default=None,
        help="Comma-separated library names (default: all available adapters)",
    )
    ap.add_argument(
        "--tag",
        default="",
        help="Optional tag suffix for output files (e.g. hard → accuracy_scoreboard_hard.md)",
    )
    args = ap.parse_args()

    keep = load_suite_filter(args.suite, args.tier)
    gts = []
    for p in sorted(GT_DIR.glob("*.json")):
        gold = json.loads(p.read_text(encoding="utf-8"))
        if keep(gold):
            gts.append(gold)
    print(f"Scoring {len(gts)} docs (suite={args.suite}, tier={args.tier})")

    requested = [x.strip() for x in args.libs.split(",")] if args.libs else None
    adapters = probe_available_adapters(requested)
    if not adapters:
        raise SystemExit("No adapters available")
    print(f"Libraries: {list(adapters.keys())}")

    if not gts:
        hint = ""
        if args.suite == "regression_precision":
            hint = (
                "\n  The precision suite is empty until you generate fixtures:\n"
                "    python benchmark/scripts/generate_precision_corpus.py\n"
            )
        elif args.suite == "regression_sensing":
            hint = (
                "\n  The sensing suite is empty until you generate fixtures:\n"
                "    python benchmark/scripts/generate_sensing_corpus.py\n"
            )
        elif args.suite == "regression_compete":
            hint = (
                "\n  The compete suite is empty until you generate fixtures:\n"
                "    python benchmark/scripts/generate_compete_corpus.py\n"
                "    python benchmark/scripts/fetch_compete_real.py\n"
            )
        raise SystemExit(
            f"No documents matched suite={args.suite!r} tier={args.tier!r}.{hint}"
        )

    detailed = []
    flat = []

    for gold in gts:
        doc_id = gold["id"]
        pdf_path = rb.resolve_pdf(gold)
        if not pdf_path:
            print(f"MISSING {doc_id}")
            continue
        print(f"=== {doc_id} ===")
        for lib, fn in adapters.items():
            # encrypted: score the with-password path for accuracy (capability with secrets)
            password = gold.get("password") if gold.get("category") == "encrypted_password" else None
            page_hint = int(gold.get("page_count") or gold.get("expected_pages_min") or 1)
            exp_tables = gold.get("expected_table_count")
            if exp_tables is None and gold.get("expected_tables") is not None:
                exp_tables = len(gold.get("expected_tables") or [])
            # Skip multi-minute table scans on huge no-table docs
            table_primary = lib in getattr(rb, "TABLE_PRIMARY_LIBS", set()) or lib.startswith("camelot")
            if table_primary and page_hint >= 40 and (exp_tables == 0 or exp_tables is None) and not gold.get("expected_tables"):
                res = rb.ExtractResult(
                    library=lib,
                    doc_id=doc_id,
                    success=True,
                    text="",
                    tables=[],
                    notes=["skipped_heavy_table_scan_no_table_gold"],
                    supports={"tables": True},
                )
                wall = 0.0
                acc = score_extraction(res, gold)
                row = flat_row(lib, doc_id, gold, res, acc, wall)
                flat.append(row)
                detailed.append({**row, "accuracy_detail": acc, "supports": res.supports, "notes": res.notes})
                print(f"  {lib:16} SKIP heavy no-table doc")
                continue

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
                    "error": res.error,
                }
            )
            print(
                f"  {lib:16} overall={row['overall_score']} textF1={row['text_token_f1']} "
                f"detF1={row['table_detect_f1']} cellF1={row['table_cell_f1']} "
                f"shape={row['table_shape_exact_rate']} {wall:.0f}ms"
                + (f" ERR={row['error']}" if row.get("error") else "")
            )

    summary = aggregate(flat)
    lib_names = list(adapters.keys())
    out = {
        "benchmark": "pdfparser-accuracy-v2",
        "suite": args.suite,
        "tier": args.tier,
        "libraries": lib_names,
        "n_docs": len(gts),
        "metric_definitions": {
            "text_token_f1": "F1 over must_contain substrings",
            "text_cer": "levenshtein(ref,hyp)/len(ref) on reference_text",
            "table_detect_f1": "count-based TP/FP/FN on number of tables",
            "table_cell_f1": "aligned cell text micro-F1 vs gold grids",
            "table_shape_exact_rate": "fraction of gold tables with exact RxC",
            "overall_score": "weighted 0-100 per GT weights",
        },
        "policy": {
            "icdar_in_regression": False,
            "note": "ICDAR is competitive-only; this scoreboard uses synthetic regression corpus",
        },
        "summary": summary,
        "rows": flat,
        "detailed": detailed,
    }

    tag = f"_{args.tag}" if args.tag else ""
    if args.suite == "regression_hard" and not args.tag:
        tag = "_hard"
    elif args.suite == "regression_precision" and not args.tag:
        tag = "_regression_precision"
    elif args.suite == "regression_sensing" and not args.tag:
        tag = "_sensing"
    elif args.suite == "regression_compete" and not args.tag:
        tag = "_compete"
    elif args.suite != "regression" and not args.tag:
        tag = f"_{args.suite}"

    json_path = RESULTS / f"accuracy_results{tag}.json"
    tsv_path = RESULTS / f"accuracy_scoreboard{tag}.tsv"
    md_path = RESULTS / f"accuracy_scoreboard{tag}.md"

    json_path.write_text(json.dumps(out, indent=2), encoding="utf-8")

    cols = list(flat[0].keys()) if flat else []
    tsv_lines = ["\t".join(cols)]
    for r in flat:
        tsv_lines.append("\t".join("" if r[c] is None else str(r[c]) for c in cols))
    tsv_path.write_text("\n".join(tsv_lines) + "\n", encoding="utf-8")

    md = render_scoreboard_md(summary, flat, suite=args.suite, libs=lib_names, n_docs=len(gts))
    md_path.write_text(md, encoding="utf-8")

    # Always refresh primary docs scoreboard for default regression run
    if args.suite == "regression" and not args.tag:
        (RESULTS / "accuracy_results.json").write_text(json.dumps(out, indent=2), encoding="utf-8")
        (RESULTS / "accuracy_scoreboard.md").write_text(md, encoding="utf-8")
        (RESULTS / "accuracy_scoreboard.tsv").write_text("\n".join(tsv_lines) + "\n", encoding="utf-8")
        docs = ROOT.parent / "docs" / "accuracy-scoreboard.md"
        docs.write_text(md, encoding="utf-8")
        print(f"Wrote {docs}")
    else:
        # copy suite-specific docs scoreboard without clobbering unrelated boards
        suite = args.suite or ""
        tag = args.tag or ""
        if suite == "regression_hard" or tag == "hard":
            docs = ROOT.parent / "docs" / "accuracy-scoreboard-hard.md"
            docs.write_text(md, encoding="utf-8")
            print(f"Wrote {docs}")
        elif suite == "regression_compete_hard" or tag == "compete_hard":
            docs = ROOT.parent / "docs" / "accuracy-scoreboard-compete-hard.md"
            docs.write_text(md, encoding="utf-8")
            print(f"Wrote {docs}")
        elif suite == "regression_compete" or tag == "compete":
            docs = ROOT.parent / "docs" / "accuracy-scoreboard-compete.md"
            docs.write_text(md, encoding="utf-8")
            print(f"Wrote {docs}")
        elif suite == "regression_sensing" or tag == "sensing":
            docs = ROOT.parent / "docs" / "accuracy-scoreboard-sensing.md"
            docs.write_text(md, encoding="utf-8")
            print(f"Wrote {docs}")
        elif suite == "regression_precision" or "precision" in tag:
            docs = ROOT.parent / "docs" / "accuracy-scoreboard-precision.md"
            docs.write_text(md, encoding="utf-8")
            print(f"Wrote {docs}")

    print("\n==== SUMMARY (table quality rank) ====")
    ranked = sorted(
        lib_names,
        key=lambda L: (summary.get(L, {}).get("table_cell_f1_mean") or -1),
        reverse=True,
    )
    for lib in ranked:
        s = summary[lib]
        print(
            f"  {lib:16} cellF1={s.get('table_cell_f1_mean')} detF1={s.get('table_detect_f1_mean')} "
            f"shape={s.get('table_shape_exact_rate_mean')} overall={s.get('overall_score_mean')} "
            f"ms={s.get('wall_ms_mean')}"
        )
    print(f"\nWrote {json_path}")
    print(f"Wrote {md_path}")


if __name__ == "__main__":
    main()
