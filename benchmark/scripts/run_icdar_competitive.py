#!/usr/bin/env python3
"""ICDAR-2013 competitive head-to-head (EXTERNAL data only).

NOT part of the regression corpus. Uses Camelot-shipped ICDAR PDFs + *-str.xml
and Camelot bench/_metrics.score (detection F1, difflib TEDS proxy, row/col).

Usage:
  source .venv/bin/activate
  cargo build --release -p pdfparser-cli
  python benchmark/scripts/run_icdar_competitive.py \\
    --data-dir /tmp/camelot-upstream/tests/files/tabula/icdar2013-dataset

Outputs:
  benchmark/results/camelot_icdar_headtohead.json
  benchmark/results/icdar_failure_analysis.json
  docs/icdar-competitive-report.md
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
import traceback
import xml.etree.ElementTree as ET
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parent
RESULTS = ROOT / "results"
RESULTS.mkdir(parents=True, exist_ok=True)

# Import Camelot metrics if available
def load_score_fn(camelot_root: Path | None):
    if camelot_root and (camelot_root / "bench" / "_metrics.py").exists():
        sys.path.insert(0, str(camelot_root / "bench"))
        from _metrics import score  # type: ignore

        return score
    # Fallback: local copy of Camelot score semantics
    import difflib

    def _norm(x):
        return " ".join(str(x or "").split()).lower()

    def simple_teds(pred, gt):
        p = [_norm(c) for row in pred for c in row]
        g = [_norm(c) for row in gt for c in row]
        if not p and not g:
            return 1.0
        content = difflib.SequenceMatcher(None, p, g).ratio()
        pr, pc = len(pred), max((len(r) for r in pred), default=0)
        gr, gc = len(gt), max((len(r) for r in gt), default=0)
        shape = 1.0 - (abs(pr - gr) + abs(pc - gc)) / (pr + pc + gr + gc + 1)
        return content * shape

    def score(pred_per_doc, gt_per_doc):
        tp = fp = fn = 0
        teds, rows_ok, cols_ok, matched = [], 0, 0, 0
        for key, gt_pages in gt_per_doc.items():
            pred_pages = pred_per_doc.get(key, {})
            for pg in set(gt_pages) | set(pred_pages):
                gts = gt_pages.get(pg, [])
                preds = pred_pages.get(pg, [])
                tp += min(len(gts), len(preds))
                fp += max(0, len(preds) - len(gts))
                fn += max(0, len(gts) - len(preds))
                for gtab, ptab in zip(gts, preds):
                    matched += 1
                    teds.append(simple_teds(ptab, gtab))
                    rows_ok += len(ptab) == len(gtab)
                    cols_ok += max((len(r) for r in ptab), default=0) == max(
                        (len(r) for r in gtab), default=0
                    )
        prec = tp / (tp + fp) if (tp + fp) else 0.0
        rec = tp / (tp + fn) if (tp + fn) else 0.0
        f1 = 2 * prec * rec / (prec + rec) if (prec + rec) else 0.0
        return {
            "f1": f1,
            "teds": sum(teds) / len(teds) if teds else 0.0,
            "row": rows_ok / matched if matched else 0.0,
            "col": cols_ok / matched if matched else 0.0,
        }

    return score


def parse_icdar_str_xml(xml_path: Path) -> dict[int, list[list[list[str]]]]:
    out: dict[int, list[list[list[str]]]] = {}
    root = ET.parse(xml_path).getroot()
    for region in root.iter("region"):
        page = int(region.get("page", "1"))
        placed = []
        max_r = max_c = 0
        for cell in region.findall("cell"):
            r = int(cell.get("start-row", "0"))
            c = int(cell.get("start-col", "0"))
            content = cell.find("content")
            text = (content.text or "") if content is not None else ""
            placed.append((r, c, " ".join(text.split())))
            max_r, max_c = max(max_r, r), max(max_c, c)
        grid = [["" for _ in range(max_c + 1)] for _ in range(max_r + 1)]
        for r, c, text in placed:
            grid[r][c] = text
        out.setdefault(page, []).append(grid)
    return out


def load_icdar(data_dir: Path, limit: int | None = None):
    n = 0
    for xml in sorted(data_dir.rglob("*-str.xml")):
        pdf = xml.with_name(xml.name.replace("-str.xml", ".pdf"))
        if not pdf.exists():
            continue
        yield pdf, parse_icdar_str_xml(xml)
        n += 1
        if limit and n >= limit:
            return


def pdfparser_bin() -> Path:
    for p in (
        REPO / "target" / "release" / "pdfparser",
        REPO / "target" / "debug" / "pdfparser",
    ):
        if p.exists():
            return p
    raise SystemExit("pdfparser binary not found; cargo build --release -p pdfparser-cli")


def extract_pdfparser(pdf: Path) -> dict[int, list]:
    bin_path = pdfparser_bin()
    proc = subprocess.run(
        [str(bin_path), "extract", "--format", "json", "--tables", str(pdf)],
        capture_output=True,
        text=True,
        timeout=180,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or f"exit {proc.returncode}")
    data = json.loads(proc.stdout or "{}")
    per_page: dict[int, list] = {}

    def tab_to_grid(tab):
        rows = int(tab.get("rows") or 0)
        cols = int(tab.get("cols") or 0)
        grid = [["" for _ in range(cols)] for _ in range(rows)]
        for cell in tab.get("cells") or []:
            r = int(cell.get("row") or 0)
            c = int(cell.get("col") or 0)
            if 0 <= r < rows and 0 <= c < cols:
                grid[r][c] = str(cell.get("text") or "")
        if rows == 0 and tab.get("cells"):
            # rebuild from max indices
            cells = tab["cells"]
            rows = max(int(c.get("row") or 0) for c in cells) + 1
            cols = max(int(c.get("col") or 0) for c in cells) + 1
            grid = [["" for _ in range(cols)] for _ in range(rows)]
            for cell in cells:
                r, c = int(cell.get("row") or 0), int(cell.get("col") or 0)
                if 0 <= r < rows and 0 <= c < cols:
                    grid[r][c] = str(cell.get("text") or "")
        return grid

    # Prefer page-local tables for ICDAR page-keyed scoring when present
    pages = data.get("pages") or []
    root_tables = data.get("tables")
    if pages:
        for i, page in enumerate(pages):
            # CLI uses 0-based "index"; ICDAR pages are 1-based
            page_no = int(page.get("index", i)) + 1
            tabs = page.get("tables") or []
            if tabs:
                per_page.setdefault(page_no, [])
                for tab in tabs:
                    per_page[page_no].append(tab_to_grid(tab))
    # Fallback: document-level tables keyed by tab.page (0-based)
    has_any = sum(len(v) for v in per_page.values())
    if has_any == 0 and isinstance(root_tables, list):
        for tab in root_tables:
            page_no = int(tab.get("page") or 0) + 1
            per_page.setdefault(page_no, []).append(tab_to_grid(tab))
    return per_page


def extract_camelot(pdf: Path, flavor: str, engine: str | None = None) -> dict[int, list]:
    import camelot
    import warnings

    kwargs = {"pages": "all", "flavor": flavor, "suppress_stdout": True}
    if flavor in ("lattice", "hybrid") and engine:
        kwargs["engine"] = engine
    per_page: dict[int, list] = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for t in camelot.read_pdf(str(pdf), **kwargs):
            per_page.setdefault(int(t.page), []).append(t.df.values.tolist())
    return per_page


def extract_pdfplumber(pdf: Path) -> dict[int, list]:
    import pdfplumber

    per_page: dict[int, list] = {}
    with pdfplumber.open(str(pdf)) as doc:
        for i, page in enumerate(doc.pages):
            page_no = i + 1
            tabs = page.extract_tables() or []
            if tabs:
                per_page[page_no] = [
                    [[("" if c is None else str(c)) for c in row] for row in t] for t in tabs
                ]
    return per_page


def extract_pymupdf(pdf: Path) -> dict[int, list]:
    import fitz

    per_page: dict[int, list] = {}
    doc = fitz.open(str(pdf))
    for i in range(doc.page_count):
        page = doc.load_page(i)
        page_no = i + 1
        try:
            tf = page.find_tables()
            if tf and tf.tables:
                per_page[page_no] = [
                    [[("" if c is None else str(c)) for c in row] for row in t.extract()]
                    for t in tf.tables
                ]
        except Exception:
            pass
    doc.close()
    return per_page


EXTRACTORS = {
    "pdfparser": lambda p: extract_pdfparser(p),
    "camelot_lattice_vector": lambda p: extract_camelot(p, "lattice", "vector"),
    "camelot_stream": lambda p: extract_camelot(p, "stream"),
    "camelot_auto": lambda p: extract_camelot(p, "auto"),
    "camelot_hybrid_vector": lambda p: extract_camelot(p, "hybrid", "vector"),
    "pdfplumber": extract_pdfplumber,
    "pymupdf": extract_pymupdf,
}


def shape_of(grid):
    rows = len(grid)
    cols = max((len(r) for r in grid), default=0)
    return rows, cols


def analyze_failures(gt_per_doc, pred_us, pred_c, score_fn):
    """Per-doc failure modes for pdfparser vs camelot_lattice_vector."""
    import difflib

    def simple_teds(pred, gt):
        def _norm(x):
            return " ".join(str(x or "").split()).lower()

        p = [_norm(c) for row in pred for c in row]
        g = [_norm(c) for row in gt for c in row]
        if not p and not g:
            return 1.0
        content = difflib.SequenceMatcher(None, p, g).ratio()
        pr, pc = len(pred), max((len(r) for r in pred), default=0)
        gr, gc = len(gt), max((len(r) for r in gt), default=0)
        shape = 1.0 - (abs(pr - gr) + abs(pc - gc)) / (pr + pc + gr + gc + 1)
        return content * shape

    mode_counts = Counter()
    per_doc = []
    for key, gt_pages in sorted(gt_per_doc.items(), key=lambda x: Path(x[0]).name):
        us_pages = pred_us.get(key, {})
        c_pages = pred_c.get(key, {})
        n_gt = sum(len(v) for v in gt_pages.values())
        n_us = sum(len(v) for v in us_pages.values())
        n_c = sum(len(v) for v in c_pages.values())

        # per-doc score
        sub_gt = {key: gt_pages}
        s_us = score_fn({key: us_pages}, sub_gt)
        s_c = score_fn({key: c_pages}, sub_gt)

        modes = []
        if n_us == 0 and n_gt > 0:
            modes.append("MISS_ALL")
        if n_us < n_gt:
            modes.append("UNDER_DETECT")
        if n_us > n_gt:
            modes.append("OVER_DETECT")
        multi_table_page = any(len(v) > 1 for v in gt_pages.values())
        if multi_table_page:
            modes.append("MULTI_TABLE_PAGE")
        if len(gt_pages) > 1:
            modes.append("MULTI_PAGE_DOC")

        # shape/structure on order-matched pairs
        row_mis = col_mis = wrong_shape = bad_struct = 0
        matched = 0
        for pg, gts in gt_pages.items():
            preds = us_pages.get(pg, [])
            for gtab, ptab in zip(gts, preds):
                matched += 1
                gr, gc = shape_of(gtab)
                pr, pc = shape_of(ptab)
                if pr != gr:
                    row_mis += 1
                if pc != gc:
                    col_mis += 1
                if (pr, pc) != (gr, gc):
                    wrong_shape += 1
                if simple_teds(ptab, gtab) < 0.5:
                    bad_struct += 1
        if row_mis:
            modes.append("ROW_MISCOUNT")
        if col_mis:
            modes.append("COL_MISCOUNT")
        if wrong_shape:
            modes.append("WRONG_SHAPE")
        if bad_struct:
            modes.append("BAD_STRUCTURE")

        for m in modes:
            mode_counts[m] += 1

        def page_shapes(pages):
            return {str(pg): [list(shape_of(g)) for g in grids] for pg, grids in sorted(pages.items())}

        # shapes as list-of-pages list-of-shapes for compactness
        def shapes_list(pages):
            out = []
            for pg in sorted(pages.keys()):
                out.append([list(shape_of(g)) for g in pages[pg]])
            return out

        per_doc.append(
            {
                "doc": Path(key).name,
                "n_gt": n_gt,
                "n_us": n_us,
                "n_camelot": n_c,
                "f1_us": s_us["f1"],
                "f1_c": s_c["f1"],
                "teds_us": s_us["teds"],
                "teds_c": s_c["teds"],
                "row_us": s_us["row"],
                "row_c": s_c["row"],
                "col_us": s_us["col"],
                "col_c": s_c["col"],
                "delta_f1": s_us["f1"] - s_c["f1"],
                "delta_teds": s_us["teds"] - s_c["teds"],
                "modes": modes,
                "gt_pages": {str(k): len(v) for k, v in gt_pages.items()},
                "us_pages": {str(k): len(v) for k, v in us_pages.items()},
                "c_pages": {str(k): len(v) for k, v in c_pages.items()},
                "us_shapes": shapes_list(us_pages),
                "gt_shapes": shapes_list(gt_pages),
                "c_shapes": shapes_list(c_pages),
            }
        )

    # buckets
    miss_all = sum(1 for d in per_doc if "MISS_ALL" in d["modes"])
    under = sum(1 for d in per_doc if "UNDER_DETECT" in d["modes"] and "MISS_ALL" not in d["modes"])
    over = sum(1 for d in per_doc if "OVER_DETECT" in d["modes"])
    good = sum(1 for d in per_doc if d["f1_us"] >= 0.95 and d["teds_us"] >= 0.7)
    bad_struct_ok_count = sum(
        1
        for d in per_doc
        if d["f1_us"] >= 0.9 and d["teds_us"] < 0.5 and "MISS_ALL" not in d["modes"]
    )
    return {
        "mode_counts": dict(mode_counts),
        "bucket_sizes": {
            "miss_all": miss_all,
            "under": under,
            "over": over,
            "bad_struct_ok_count": bad_struct_ok_count,
            "good": good,
        },
        "per_doc": per_doc,
    }


def render_report(results: dict, analysis: dict, prev: dict | None) -> str:
    lines = []
    lines.append("# ICDAR-2013 Competitive Analysis (external)\n\n")
    lines.append(
        "**Policy:** ICDAR is **not** part of the regression corpus. "
        "This report is for competitive measurement only.\n\n"
    )
    lines.append(f"**Docs:** {results['n_docs']} · **Gold:** ICDAR `*-str.xml` · "
                 f"**Metrics:** Camelot `bench/_metrics.score` (F1, TEDS proxy, row/col)\n\n")

    lines.append("## Leaderboard\n\n")
    lines.append("| Rank | Tool | F1 | TEDS | row | col | time (s) |\n")
    lines.append("|-----:|------|---:|-----:|----:|----:|---------:|\n")
    for i, r in enumerate(results["ranking"], 1):
        mark = " ← **ours**" if r["tool"] == "pdfparser" else ""
        lines.append(
            f"| {i} | **{r['tool']}**{mark} | {r['f1']:.3f} | {r['teds']:.3f} | "
            f"{r['row']:.3f} | {r['col']:.3f} | {r['time_s']:.2f} |\n"
        )

    us = results["results"]["pdfparser"]
    cl = results["results"].get("camelot_lattice_vector", {})
    ca = results["results"].get("camelot_auto", {})
    lines.append("\n## pdfparser vs Camelot (headline)\n\n")
    lines.append("| Metric | pdfparser | camelot lattice/vector | camelot auto | Δ vs lattice |\n")
    lines.append("|--------|----------:|-----------------------:|-------------:|-------------:|\n")
    for k in ("f1", "teds", "row", "col"):
        u, c = us.get(k), cl.get(k)
        a = ca.get(k)
        delta = (u - c) if u is not None and c is not None else None
        a_s = f"{a:.3f}" if a is not None else "—"
        d_s = f"{delta:+.3f}" if delta is not None else "—"
        lines.append(f"| {k} | {u:.3f} | {c:.3f} | {a_s} | {d_s} |\n")

    if prev and "results" in prev and "pdfparser" in prev["results"]:
        p = prev["results"]["pdfparser"]
        lines.append("\n## Improvement vs previous ICDAR run\n\n")
        lines.append("| Metric | Previous | Now | Δ |\n")
        lines.append("|--------|---------:|----:|--:|\n")
        for k in ("f1", "teds", "row", "col"):
            lines.append(
                f"| {k} | {p[k]:.3f} | {us[k]:.3f} | {us[k] - p[k]:+.3f} |\n"
            )

    lines.append("\n## Failure mode histogram (pdfparser)\n\n")
    lines.append("| Mode | Docs |\n|------|-----:|\n")
    for m, c in sorted(analysis["mode_counts"].items(), key=lambda x: -x[1]):
        lines.append(f"| {m} | {c} |\n")
    lines.append("\n### Buckets\n\n")
    for k, v in analysis["bucket_sizes"].items():
        lines.append(f"- **{k}:** {v}\n")

    # slices
    docs = analysis["per_doc"]
    multi = [d for d in docs if "MULTI_TABLE_PAGE" in d["modes"] or d["n_gt"] > 1]
    single = [d for d in docs if d["n_gt"] == 1]
    lines.append("\n## Multi-table vs single-table\n\n")
    if multi:
        lines.append(
            f"- Multi-table docs (n={len(multi)}): mean F1 us={sum(d['f1_us'] for d in multi)/len(multi):.3f}, "
            f"camelot={sum(d['f1_c'] for d in multi)/len(multi):.3f}; "
            f"TEDS us={sum(d['teds_us'] for d in multi)/len(multi):.3f}, "
            f"camelot={sum(d['teds_c'] for d in multi)/len(multi):.3f}\n"
        )
    if single:
        lines.append(
            f"- Single-table docs (n={len(single)}): mean F1 us={sum(d['f1_us'] for d in single)/len(single):.3f}, "
            f"camelot={sum(d['f1_c'] for d in single)/len(single):.3f}; "
            f"TEDS us={sum(d['teds_us'] for d in single)/len(single):.3f}, "
            f"camelot={sum(d['teds_c'] for d in single)/len(single):.3f}\n"
        )

    worst = sorted(docs, key=lambda d: d["delta_teds"])[:15]
    lines.append("\n## Worst TEDS gap vs Camelot lattice (top 15)\n\n")
    lines.append("| Doc | ΔTEDS | F1 us/c | TEDS us/c | n_gt/us/c | modes |\n")
    lines.append("|------|------:|--------:|----------:|----------:|-------|\n")
    for d in worst:
        lines.append(
            f"| `{d['doc']}` | {d['delta_teds']:+.3f} | {d['f1_us']:.2f}/{d['f1_c']:.2f} | "
            f"{d['teds_us']:.3f}/{d['teds_c']:.3f} | {d['n_gt']}/{d['n_us']}/{d['n_camelot']} | "
            f"{', '.join(d['modes'][:4])} |\n"
        )

    high_f1_low_teds = [d for d in docs if d["f1_us"] >= 0.9 and d["teds_us"] < 0.35]
    lines.append(f"\n## Detect OK, structure bad (F1≥0.9, TEDS&lt;0.35) — n={len(high_f1_low_teds)}\n\n")
    for d in high_f1_low_teds[:12]:
        lines.append(
            f"- `{d['doc']}`: shapes us={d['us_shapes']} gt={d['gt_shapes']} "
            f"TEDS={d['teds_us']:.3f}\n"
        )

    miss = [d for d in docs if "MISS_ALL" in d["modes"]]
    lines.append(f"\n## MISS_ALL (n={len(miss)})\n\n")
    for d in miss:
        lines.append(
            f"- `{d['doc']}`: gt={d['n_gt']} camelot={d['n_camelot']} "
            f"camelot TEDS={d['teds_c']:.3f}\n"
        )

    lines.append("\n## Gap analysis (where we still lack)\n\n")
    lines.append(results.get("gap_analysis_md", "_see JSON_\n"))

    lines.append(
        "\n---\n*Generated by `benchmark/scripts/run_icdar_competitive.py`. "
        "ICDAR files remain external; never copied into `benchmark/corpus/`.*\n"
    )
    return "".join(lines)


def build_gap_analysis(analysis: dict, results: dict) -> str:
    us = results["results"]["pdfparser"]
    cl = results["results"].get("camelot_lattice_vector", {})
    lines = []
    lines.append(
        f"pdfparser F1={us['f1']:.3f} TEDS={us['teds']:.3f} row={us['row']:.3f} col={us['col']:.3f} "
        f"vs camelot lattice F1={cl.get('f1', 0):.3f} TEDS={cl.get('teds', 0):.3f}.\n\n"
    )
    mc = analysis["mode_counts"]
    lines.append("### Primary remaining gaps\n\n")
    lines.append(
        "1. **Structure quality (TEDS / row / col)** — Detection has improved more than content alignment. "
        f"ROW_MISCOUNT={mc.get('ROW_MISCOUNT', 0)}, COL_MISCOUNT={mc.get('COL_MISCOUNT', 0)}, "
        f"WRONG_SHAPE={mc.get('WRONG_SHAPE', 0)}, BAD_STRUCTURE={mc.get('BAD_STRUCTURE', 0)}.\n"
    )
    lines.append(
        "2. **MISS_ALL / UNDER_DETECT** — "
        f"MISS_ALL={mc.get('MISS_ALL', 0)}, UNDER_DETECT={mc.get('UNDER_DETECT', 0)}. "
        "Often stream-only or faint/incomplete rules where lattice CC has too few joints; "
        "Camelot raster/auto recovers some of these.\n"
    )
    lines.append(
        "3. **MULTI_TABLE_PAGE** — "
        f"{mc.get('MULTI_TABLE_PAGE', 0)} docs. Multi-region CC helps; residual fusion or "
        "order-mismatch vs gold still hurts F1/TEDS (order-based matching).\n"
    )
    lines.append(
        "4. **Spans & partial rules** — High F1 / low TEDS cases usually have wrong row/col "
        "counts from extra decorative lines or missing span merge on real competition layouts.\n"
    )
    lines.append(
        "5. **Metric sensitivity** — ICDAR matching is **page order**, not IoU. Correct tables "
        "in wrong order look like structure failures. TEDS is a difflib proxy, not tree-edit TEDS.\n"
    )
    lines.append(
        "6. **No raster line engine** — Camelot `auto`/`combined` can find painted/faint rules; "
        "we are vector-only.\n"
    )
    return "".join(lines)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--data-dir",
        type=Path,
        default=Path("/tmp/camelot-upstream/tests/files/tabula/icdar2013-dataset"),
    )
    ap.add_argument(
        "--camelot-root",
        type=Path,
        default=Path("/tmp/camelot-upstream"),
    )
    ap.add_argument("--limit", type=int, default=None)
    ap.add_argument(
        "--libs",
        default="pdfparser,camelot_lattice_vector,camelot_auto,pdfplumber,pymupdf",
        help="Comma-separated extractors",
    )
    args = ap.parse_args()

    if not args.data_dir.exists():
        raise SystemExit(
            f"ICDAR data not found at {args.data_dir}. "
            "Clone camelot: git clone --depth 1 https://github.com/camelot-dev/camelot.git /tmp/camelot-upstream"
        )

    score_fn = load_score_fn(args.camelot_root if args.camelot_root.exists() else None)
    libs = [x.strip() for x in args.libs.split(",") if x.strip()]
    for lib in libs:
        if lib not in EXTRACTORS:
            raise SystemExit(f"Unknown lib {lib}; choose from {list(EXTRACTORS)}")

    # Load GT once
    docs = list(load_icdar(args.data_dir, args.limit))
    print(f"ICDAR docs: {len(docs)}")
    gt_per_doc = {str(pdf): gt for pdf, gt in docs}

    prev_path = RESULTS / "camelot_icdar_headtohead.json"
    prev = None
    if prev_path.exists():
        try:
            prev = json.loads(prev_path.read_text())
        except Exception:
            prev = None

    results_map = {}
    preds_store = {}

    for lib in libs:
        print(f"\n=== {lib} ===")
        pred_per_doc = {}
        errors = 0
        t0 = time.perf_counter()
        for pdf, gt in docs:
            key = str(pdf)
            try:
                pred_per_doc[key] = EXTRACTORS[lib](pdf)
            except Exception as e:
                errors += 1
                pred_per_doc[key] = {}
                print(f"  ERR {pdf.name}: {type(e).__name__}: {e}")
            n_tabs = sum(len(v) for v in pred_per_doc[key].values())
            n_gt = sum(len(v) for v in gt.values())
            print(f"  {pdf.name}: pred_tables={n_tabs} gt={n_gt}")
        elapsed = time.perf_counter() - t0
        metrics = score_fn(pred_per_doc, gt_per_doc)
        metrics.update(
            {
                "time_s": elapsed,
                "errors": errors,
                "docs": len(docs),
            }
        )
        results_map[lib] = metrics
        preds_store[lib] = pred_per_doc
        print(
            f"  → F1={metrics['f1']:.3f} TEDS={metrics['teds']:.3f} "
            f"row={metrics['row']:.3f} col={metrics['col']:.3f} time={elapsed:.1f}s"
        )

    ranking = sorted(
        [{"tool": k, **v} for k, v in results_map.items()],
        key=lambda x: (x["f1"], x["teds"]),
        reverse=True,
    )
    for i, r in enumerate(ranking, 1):
        r["rank"] = i

    out = {
        "dataset": "ICDAR-2013 (camelot-dev/camelot shipped)",
        "metrics": "camelot bench/_metrics.score (detection F1, difflib TEDS proxy, row/col exact)",
        "gold": "ICDAR *-str.xml via parse_icdar_str_xml",
        "n_docs": len(docs),
        "policy": "competitive_only_not_regression",
        "results": results_map,
        "ranking": ranking,
    }

    # Failure analysis vs camelot lattice
    c_key = "camelot_lattice_vector" if "camelot_lattice_vector" in preds_store else ranking[0]["tool"]
    us_pred = preds_store.get("pdfparser", {})
    c_pred = preds_store.get(c_key, {})
    analysis = analyze_failures(gt_per_doc, us_pred, c_pred, score_fn)
    gap_md = build_gap_analysis(analysis, out)
    out["gap_analysis_md"] = gap_md
    analysis["gap_analysis_md"] = gap_md

    (RESULTS / "camelot_icdar_headtohead.json").write_text(json.dumps(out, indent=2) + "\n")
    (RESULTS / "icdar_failure_analysis.json").write_text(json.dumps(analysis, indent=2) + "\n")

    report = render_report(out, analysis, prev)
    (RESULTS / "icdar_competitive_report.md").write_text(report)
    docs_path = REPO / "docs" / "icdar-competitive-report.md"
    docs_path.write_text(report)

    print("\n==== RANKING ====")
    for r in ranking:
        print(
            f"  #{r['rank']} {r['tool']:28} F1={r['f1']:.3f} TEDS={r['teds']:.3f} "
            f"row={r['row']:.3f} col={r['col']:.3f} t={r['time_s']:.1f}s"
        )
    print(f"\nWrote {RESULTS / 'camelot_icdar_headtohead.json'}")
    print(f"Wrote {RESULTS / 'icdar_failure_analysis.json'}")
    print(f"Wrote {docs_path}")


if __name__ == "__main__":
    main()
