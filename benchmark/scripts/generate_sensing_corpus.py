#!/usr/bin/env python3
"""Generate *struggle* synthetic fixtures for table-engine development.

Suite: regression_sensing  (tier=hard_sensing)
Path:  corpus/hard_sensing/

═══════════════════════════════════════════════════════════════════════════
POLICY — this is NOT a “beat Camelot mean score” suite
═══════════════════════════════════════════════════════════════════════════

Include a document ONLY if:
  1. pdfparser fails hard vs *gold* (shape / cell F1 / detect), AND
  2. preferably at least one strong peer *or* a clear algorithm path succeeds.

Do NOT pad the suite with free wins (those dilute the mean and create the
illusion we are “ahead”). Free-win chrome cases belong in
regression_precision (70–82), not here.

ICDAR PDFs/XML are never copied. Modes only, original tokens.

Current struggle set
--------------------
  90  Painted thin-rect rules + jittered text
      → us: stream mis-grid (cell F1 ≪ 1); camelot lattice / plumber often 1.0
  91  Two large stacked stroke grids (12×5 each)
      → us: 0 tables (lattice silent); camelot lattice finds both
  92  Large borderless 28×8 with mid-table prose gap
      → us: splits into 2×14-row tables; gold wants 1×28

Regenerate
----------
  python benchmark/scripts/generate_sensing_corpus.py
  python benchmark/scripts/run_accuracy_benchmark.py --suite regression_sensing \\
    --libs pdfparser,pdfplumber,camelot_lattice,camelot_auto --tag sensing

Judge progress on **per-doc** gap-to-gold (and gap-to-best-peer on that doc),
not on suite-level rank.
"""
from __future__ import annotations

import json
from pathlib import Path

from reportlab.lib import colors
from reportlab.lib.pagesizes import letter
from reportlab.pdfgen import canvas

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "corpus"
SENS = CORPUS / "hard_sensing"
GT = ROOT / "ground_truth"
SUITES = CORPUS / "suites.json"
for d in (CORPUS, SENS, GT):
    d.mkdir(parents=True, exist_ok=True)

PAGE_W, PAGE_H = letter

# Struggle-only IDs (no free wins)
SENSING_IDS = [
    "90_painted_thin_rect_rules",
    "91_two_large_stacked_grids",
    "92_large_borderless_prose_gap",
]


def write_gt(doc_id: str, data: dict) -> None:
    payload = {
        "id": doc_id,
        "tier": "hard_sensing",
        "suite": "regression_sensing",
        "source": "synthetic",
        "icdar_derived": False,
        "path": f"corpus/hard_sensing/{doc_id}.pdf",
        "weight_text": 0.10,
        "weight_tables": 0.90,
        "weight_objects": 0.0,
        "expected_images": 0,
        "development_role": "struggle",  # not a free-win pad
        **data,
    }
    if "expected_tables" in payload:
        tables = []
        for t in payload["expected_tables"]:
            cells = t["cells"]
            tables.append(
                {
                    **{k: v for k, v in t.items() if k != "cells"},
                    "rows": len(cells),
                    "cols": len(cells[0]) if cells else 0,
                    "cells": cells,
                }
            )
        payload["expected_tables"] = tables
        payload.setdefault("expected_table_count", len(tables))
    (GT / f"{doc_id}.json").write_text(
        json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )


def draw_grid_thin_rects(c, xs, ys, thickness=1.1):
    c.setFillColor(colors.black)
    x_left, x_right = xs[0], xs[-1]
    y_bot, y_top = ys[-1], ys[0]
    for y in ys:
        c.rect(x_left, y - thickness / 2, x_right - x_left, thickness, fill=1, stroke=0)
    for x in xs:
        c.rect(x - thickness / 2, y_bot, thickness, y_top - y_bot, fill=1, stroke=0)


def draw_grid_stroke(c, xs, ys, lw=0.65):
    c.setStrokeColor(colors.black)
    c.setLineWidth(lw)
    for y in ys:
        c.line(xs[0], y, xs[-1], y)
    for x in xs:
        c.line(x, ys[-1], x, ys[0])


# ─── 90 painted thin-rect rules (peer lattice succeeds; we stream-fail) ──────
def gen_90():
    doc_id = "90_painted_thin_rect_rules"
    path = SENS / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)

    c.setFont("Helvetica-Bold", 12)
    c.drawString(48, PAGE_H - 40, "Painted Rules Grid TOKEN_S90_DOC")
    c.setFont("Helvetica", 8)
    c.drawString(
        48,
        PAGE_H - 56,
        "Struggle: thin filled-rect rules + row jitter. Peers lattice≈1.0; we stream-misgrid.",
    )

    cells = [
        ["Metric", "Q1", "Q2", "Q3", "Note"],
        ["Revenue TOKEN_S90_REV", "120", "", "140", "up"],
        ["", "48", "52", "", "raw"],
        ["Gross TOKEN_S90_GP", "", "83", "90", ""],
        ["OpEx", "30", "", "33", "ops"],
        ["Net TOKEN_S90_NI", "42", "52", "57", "ni"],
    ]
    col_w = [150, 48, 72, 40, 80]
    row_h = [18, 22, 14, 20, 16, 18]
    x0, y_top = 90, PAGE_H - 100
    xs = [x0]
    for w in col_w:
        xs.append(xs[-1] + w)
    ys = [y_top]
    for h in row_h:
        ys.append(ys[-1] - h)

    draw_grid_thin_rects(c, xs, ys, thickness=1.1)
    jitter = [0, 10, -8, 14, -6, 12]
    for r, row in enumerate(cells):
        for col, txt in enumerate(row):
            if not txt:
                continue
            x_lo, x_hi = xs[col], xs[col + 1]
            y_hi, y_lo = ys[r], ys[r + 1]
            c.setFont("Helvetica-Bold" if r == 0 else "Helvetica", 7)
            j = jitter[r % len(jitter)]
            x = min(max(x_lo + 3 + j, x_lo + 2), x_hi - 20)
            c.drawString(x, (y_lo + y_hi) / 2 - 2.5, txt)

    c.setFont("Helvetica", 7)
    c.drawString(48, 48, "TOKEN_S90_FOOT · target lattice 6×5 cellF1=1 (not stream 6×4 mess)")
    c.showPage()
    c.save()

    write_gt(
        doc_id,
        {
            "category": "painted_thin_rect_rules",
            "description": "STRUGGLE: painted thin-rect rules; stream mis-grids; lattice peers OK",
            "challenges": [
                "painted_rules",
                "thin_rect_lines",
                "lattice_sensing",
                "sparse_cells",
                "stream_fallback_trap",
            ],
            "failure_modes_covered": ["WRONG_SHAPE", "STREAM_FALLBACK", "COL_MISCOUNT"],
            "must_contain": [
                "TOKEN_S90_DOC",
                "TOKEN_S90_REV",
                "TOKEN_S90_GP",
                "TOKEN_S90_NI",
            ],
            "table_cells_must_include": [
                "Metric",
                "Revenue TOKEN_S90_REV",
                "Gross TOKEN_S90_GP",
                "Net TOKEN_S90_NI",
                "120",
                "57",
            ],
            "expected_tables": [{"cells": cells}],
            "page_count": 1,
            "preferred_method": "lattice",
            "struggle_baseline": {
                "note": "pdfparser stream mis-grid; camelot_lattice/pdfplumber often perfect",
                "target_shape": [6, 5],
            },
            "quality_notes": "WIN = n=1 shape 6×5 cellF1≈1 via lattice on painted rules.",
        },
    )
    print(" ", doc_id)


# ─── 91 two large stacked stroke grids (we find 0; camelot finds 2) ──────────
def gen_91():
    """Two large stroke grids with geometry that currently silences our lattice.

    Empirically: col pitch 55pt + row pitch 14pt + dense cell text → us n=0,
    camelot lattice n=2. Slightly different pitch (48/13) makes us pass — so
    this is a real sensing/CC/joint regression, not an accident.
    """
    doc_id = "91_two_large_stacked_grids"
    path = SENS / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)

    c.setFont("Helvetica-Bold", 11)
    c.drawString(40, PAGE_H - 32, "Two Large Stacked Grids TOKEN_S91_DOC")
    c.setFont("Helvetica", 7.5)
    c.drawString(
        40,
        PAGE_H - 46,
        "STRUGGLE: baseline pdfparser n=0 tables; camelot lattice finds both 12×5.",
    )

    def make_cells(tag: str, corner_token: str) -> list[list[str]]:
        headers = ["K", "V1", "V2", "V3", "Note"]
        rows = [headers]
        for i in range(1, 12):
            # Dense short tokens (probe geometry) — still unique for gold match
            row = [
                f"{tag}{i:02d}",
                f"{tag}{i}a",
                f"{tag}{i}b",
                f"{tag}{i}c",
                f"{tag}{i}n",
            ]
            if i == 1:
                row[4] = corner_token
            if i == 11:
                row[4] = f"{tag}_LAST"
            rows.append(row)
        return rows

    cells_a = make_cells("A", "TOKEN_S91_A")
    cells_b = make_cells("B", "TOKEN_S91_B")

    def paint_grid(cells, x0, y_top):
        # FAILING pitch (do not "fix" by changing to 48/13 without intentional work)
        col_w = 55.0
        row_h = 14.0
        nrows, ncols = len(cells), len(cells[0])
        xs = [x0 + i * col_w for i in range(ncols + 1)]
        ys = [y_top - i * row_h for i in range(nrows + 1)]
        draw_grid_stroke(c, xs, ys, lw=0.6)
        for r in range(nrows):
            for col in range(ncols):
                txt = cells[r][col]
                c.setFont("Helvetica-Bold" if r == 0 else "Helvetica", 6)
                c.drawString(xs[col] + 2, (ys[r] + ys[r + 1]) / 2 - 2.0, txt)

    paint_grid(cells_a, 80, 720)
    paint_grid(cells_b, 80, 480)

    c.setFont("Helvetica", 7)
    c.drawString(40, 36, "TOKEN_S91_FOOT · WIN = n=2 shapes 12×5 each (lattice multi-region)")
    c.showPage()
    c.save()

    write_gt(
        doc_id,
        {
            "category": "two_large_stacked_grids",
            "description": "STRUGGLE: two 12×5 stroke lattices; baseline us n=0, camelot n=2",
            "challenges": [
                "multi_table_page",
                "large_lattice",
                "multi_region",
                "under_detect",
                "lattice_cc",
                "joint_density",
            ],
            "failure_modes_covered": ["UNDER_DETECT", "MISS_ALL", "WRONG_SHAPE"],
            "must_contain": ["TOKEN_S91_DOC", "TOKEN_S91_A", "TOKEN_S91_B"],
            "table_cells_must_include": [
                "TOKEN_S91_A",
                "TOKEN_S91_B",
                "A01",
                "B01",
                "A_LAST",
                "B_LAST",
            ],
            "expected_tables": [{"cells": cells_a}, {"cells": cells_b}],
            "page_count": 1,
            "preferred_method": "lattice",
            "struggle_baseline": {
                "pdfparser": "n=0",
                "camelot_lattice": "n=2 shapes 12×5",
                "target_shapes": [[12, 5], [12, 5]],
            },
            "quality_notes": "WIN only when n=2 and both 12×5 with A/B tokens. Do not weaken geometry to greenwash.",
        },
    )
    print(" ", doc_id)


def gen_92():
    doc_id = "92_large_borderless_prose_gap"
    path = SENS / f"{doc_id}.pdf"
    # remove old filename if present
    old = SENS / "92_large_borderless_grid.pdf"
    if old.exists():
        old.unlink()
    old_gt = GT / "92_large_borderless_grid.json"
    if old_gt.exists():
        old_gt.unlink()

    c = canvas.Canvas(str(path), pagesize=letter)
    c.setFont("Helvetica-Bold", 11)
    c.drawString(36, PAGE_H - 36, "Borderless Prose Gap TOKEN_S92_DOC")
    c.setFont("Helvetica", 7)
    c.drawString(
        36,
        PAGE_H - 50,
        "Struggle: 28×8 borderless; mid prose must NOT split into two tables.",
    )

    headers = ["Code", "Name", "A", "B", "C", "D", "E", "Flag"]
    cells = [list(headers)]
    for i in range(1, 28):
        row = [
            f"R{i:02d}",
            f"Item{i}",
            str(10 + i),
            str(20 + i),
            str(30 + i),
            str(40 + i),
            str(50 + i),
            "Y" if i % 3 else "N",
        ]
        if i == 1:
            row[1] = "Item1 TOKEN_S92_R1"
        if i == 14:
            row[1] = "Item14 TOKEN_S92_MID"
        if i == 27:
            row[1] = "Item27 TOKEN_S92_LAST"
        cells.append(row)

    col_w = [36, 130, 32, 32, 32, 32, 32, 28]
    row_h = 10.5
    x0 = 36
    y_top = PAGE_H - 68
    xs = [x0]
    for w in col_w:
        xs.append(xs[-1] + w)

    c.setFont("Helvetica-Bold", 6.5)
    for col, h in enumerate(headers):
        c.drawString(xs[col] + 1, y_top - 8, h)

    c.setFont("Helvetica", 6.5)
    for r in range(1, 14):
        y = y_top - 8 - r * row_h
        for col in range(8):
            c.drawString(xs[col] + 1, y, cells[r][col])

    prose_y = y_top - 8 - 13 * row_h - 14
    c.setFont("Helvetica-Oblique", 7)
    c.drawString(
        36,
        prose_y,
        "Interstitial note TOKEN_S92_GAP — same logical table continues below.",
    )

    y_resume = prose_y - 16
    c.setFont("Helvetica", 6.5)
    for r in range(14, 28):
        y = y_resume - (r - 13) * row_h
        for col in range(8):
            c.drawString(xs[col] + 1, y, cells[r][col])

    c.setFont("Helvetica", 6)
    c.drawString(36, 28, "TOKEN_S92_FOOT · target: n=1 shape≈28×8 (not 2×14)")
    c.showPage()
    c.save()

    write_gt(
        doc_id,
        {
            "category": "large_borderless_prose_gap",
            "description": "STRUGGLE: 28×8 borderless split by mid prose; gold is one table",
            "challenges": [
                "borderless",
                "large_row_count",
                "prose_gap_mid_table",
                "anti_split",
                "network_class",
            ],
            "failure_modes_covered": [
                "OVER_DETECT",
                "ROW_MISCOUNT",
                "WRONG_SHAPE",
                "BAD_STRUCTURE",
            ],
            "must_contain": [
                "TOKEN_S92_DOC",
                "TOKEN_S92_R1",
                "TOKEN_S92_MID",
                "TOKEN_S92_LAST",
                "TOKEN_S92_GAP",
            ],
            "table_cells_must_include": [
                "Code",
                "Item1 TOKEN_S92_R1",
                "Item14 TOKEN_S92_MID",
                "Item27 TOKEN_S92_LAST",
                "R01",
                "R27",
            ],
            "expected_tables": [{"cells": cells}],
            "expected_table_count": 1,
            "page_count": 1,
            "preferred_method": "stream",
            "struggle_baseline": {
                "note": "pdfparser often emits 2×(14×8); peers also weak — gap is to gold",
                "target_shape": [28, 8],
            },
            "quality_notes": (
                "WIN = n=1 ≈28×8 with R1 and LAST in same table. "
                "Peers may also fail; still a valid us-vs-gold struggle."
            ),
        },
    )
    print(" ", doc_id)


def update_suites_json() -> None:
    if SUITES.exists():
        data = json.loads(SUITES.read_text(encoding="utf-8"))
    else:
        data = {"version": 1, "policy": {}, "suites": {}}

    data.setdefault("policy", {})
    data["policy"]["regression_never_includes_icdar"] = True
    data["policy"]["sensing_suite_purpose"] = (
        "STRUGGLE-only capability modes. Include docs only when pdfparser fails "
        "gold (and preferably a peer/algorithm path succeeds). Do not pad with "
        "free wins — those belong in hard_precision. Original synthetic only."
    )

    data.setdefault("suites", {})
    data["suites"]["regression_sensing"] = {
        "description": (
            "Struggle suite (90+): painted rules, multi large lattice under-detect, "
            "borderless split. Not a peer leaderboard — track per-doc gap-to-gold."
        ),
        "tiers": ["hard_sensing"],
        "document_ids": list(SENSING_IDS),
        "icdar_derived": False,
        "scoring_guidance": (
            "Ignore suite mean rank. Report per-doc cell F1 / detect / shape "
            "vs gold and vs best peer on that doc."
        ),
    }

    data.setdefault("hard_failure_mode_coverage", {})
    data["hard_failure_mode_coverage"]["PAINTED_RULES"] = ["90_painted_thin_rect_rules"]
    data["hard_failure_mode_coverage"]["MULTI_LARGE_LATTICE_UNDERDETECT"] = [
        "91_two_large_stacked_grids"
    ]
    data["hard_failure_mode_coverage"]["BORDERLESS_PROSE_SPLIT"] = [
        "92_large_borderless_prose_gap"
    ]

    SUITES.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    print("  updated suites.json")


def cleanup_retired() -> None:
    """Remove free-win / renamed fixtures from earlier draft."""
    for name in [
        "93_ruled_plus_page_chrome",
        "92_large_borderless_grid",
        "_probe_two_large",
    ]:
        p = SENS / f"{name}.pdf"
        g = GT / f"{name}.json"
        if p.exists():
            p.unlink()
            print("  removed", p.name)
        if g.exists():
            g.unlink()
            print("  removed", g.name)


def main() -> None:
    print("Generating STRUGGLE-only hard_sensing corpus…")
    cleanup_retired()
    gen_90()
    gen_91()
    gen_92()
    update_suites_json()
    print("Done:", ", ".join(SENSING_IDS))
    print(
        "\nRemember: judge per-doc gap-to-gold, not suite mean rank.\n"
        "  python benchmark/scripts/run_accuracy_benchmark.py "
        "--suite regression_sensing --libs pdfparser,camelot_lattice,pdfplumber --tag sensing"
    )


if __name__ == "__main__":
    main()
