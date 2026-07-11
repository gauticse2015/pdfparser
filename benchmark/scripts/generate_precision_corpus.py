#!/usr/bin/env python3
"""Generate precision/structure synthetic fixtures (ICDAR-like modes, original content).

Suite: regression_precision  (tier=hard_precision)
Path:  corpus/hard_precision/70_….pdf … 82_….pdf

These fixtures stress OVER_DETECT, phantom lines, stream FPs, and structure —
not the multi-table modes already covered by hard/50–62.

  python benchmark/scripts/generate_precision_corpus.py
  python benchmark/scripts/run_accuracy_benchmark.py --suite regression_precision \\
    --libs pdfparser,pdfplumber,camelot_lattice,camelot_auto
"""
from __future__ import annotations

import json
from pathlib import Path

from reportlab.lib import colors
from reportlab.lib.pagesizes import letter
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.lib.enums import TA_CENTER
from reportlab.platypus import Paragraph, SimpleDocTemplate, Spacer, Table, TableStyle
from reportlab.pdfgen import canvas

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "corpus"
PREC = CORPUS / "hard_precision"
GT = ROOT / "ground_truth"
for d in (CORPUS, PREC, GT):
    d.mkdir(parents=True, exist_ok=True)

PAGE_W, PAGE_H = letter
styles = getSampleStyleSheet()
styles.add(ParagraphStyle(name="PTitle", fontName="Helvetica-Bold", fontSize=12, leading=14))
styles.add(ParagraphStyle(name="PMeta", fontName="Helvetica", fontSize=8, leading=10))
styles.add(ParagraphStyle(name="PCell", fontName="Helvetica", fontSize=8, leading=10))
styles.add(
    ParagraphStyle(name="PHdr", fontName="Helvetica-Bold", fontSize=8, leading=10, alignment=TA_CENTER)
)


def write_gt(doc_id: str, data: dict) -> None:
    payload = {
        "id": doc_id,
        "tier": "hard_precision",
        "suite": "regression_precision",
        "source": "synthetic",
        "icdar_derived": False,
        "path": f"corpus/hard_precision/{doc_id}.pdf",
        "weight_text": 0.15,
        "weight_tables": 0.85,
        "weight_objects": 0.0,
        "expected_images": 0,
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
        payload["expected_table_count"] = len(tables)
    (GT / f"{doc_id}.json").write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def p(text: str, style: str = "PCell"):
    return Paragraph(str(text).replace("\n", "<br/>"), styles[style])


def draw_grid(c, x0, y_bot, col_w, row_h, cells, lw=0.7, fs=8):
    ncols, nrows = len(col_w), len(row_h)
    xs = [x0]
    for w in col_w:
        xs.append(xs[-1] + w)
    total_h = sum(row_h)
    y_top = y_bot + total_h
    ys = [y_top]
    for h in row_h:
        ys.append(ys[-1] - h)
    c.setStrokeColor(colors.black)
    c.setLineWidth(lw)
    for y in ys:
        c.line(xs[0], y, xs[-1], y)
    for x in xs:
        c.line(x, y_bot, x, y_top)
    c.setFillColor(colors.black)
    c.setFont("Helvetica", fs)
    for r in range(nrows):
        for col in range(ncols):
            text = cells[r][col] if r < len(cells) and col < len(cells[r]) else ""
            if not text:
                continue
            cx = (xs[col] + xs[col + 1]) / 2
            cy = (ys[r] + ys[r + 1]) / 2 - fs * 0.3
            c.drawCentredString(cx, cy, text[:40])


# ─── 70 decorative H-rules + one table ───────────────────────────────────────
def gen_70():
    doc_id = "70_deco_rules_one_table"
    path = PREC / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setFont("Helvetica-Bold", 12)
    c.drawString(50, PAGE_H - 40, "Section Noise TOKEN_P70_DOC")
    c.setStrokeColor(colors.Color(0.5, 0.5, 0.55))
    c.setLineWidth(0.5)
    for y in [PAGE_H - 70, PAGE_H - 90, PAGE_H - 110, 100, 80, 60]:
        c.line(40, y, PAGE_W - 40, y)
    cells = [
        ["Key", "Value", "Unit"],
        ["alpha TOKEN_P70_A", "1.1", "m"],
        ["beta", "2.2", "m"],
        ["gamma TOKEN_P70_G", "3.3", "s"],
        ["delta", "4.4", "s"],
    ]
    draw_grid(c, 120, PAGE_H - 320, [90, 60, 50], [18] * 5, cells)
    c.setFont("Helvetica", 8)
    c.drawString(120, PAGE_H - 340, "Exactly one data table expected.")
    c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "deco_rules_one_table",
            "description": "Many decorative full-width H-rules + one lattice table",
            "challenges": ["over_detect", "decorative_rules", "phantom_lines"],
            "failure_modes_covered": ["OVER_DETECT", "WRONG_SHAPE"],
            "must_contain": ["TOKEN_P70_DOC", "TOKEN_P70_A", "TOKEN_P70_G"],
            "table_cells_must_include": ["TOKEN_P70_A", "Key", "gamma TOKEN_P70_G"],
            "expected_tables": [{"cells": cells}],
            "page_count": 1,
            "quality_notes": "Success: n_tables=1 shape 5×3. Fail: extra tables from section rules.",
        },
    )
    print(" ", doc_id)


# ─── 71 page frame + table ───────────────────────────────────────────────────
def gen_71():
    doc_id = "71_frame_and_table"
    path = PREC / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setStrokeColor(colors.Color(0.3, 0.3, 0.35))
    c.setLineWidth(1.5)
    c.rect(24, 24, PAGE_W - 48, PAGE_H - 48)
    c.line(36, PAGE_H - 64, PAGE_W - 36, PAGE_H - 64)
    c.setFillColor(colors.black)
    c.setFont("Helvetica-Bold", 12)
    c.drawString(50, PAGE_H - 48, "Framed Brief TOKEN_P71_DOC")
    cells = [
        ["Item", "Owner", "Status"],
        ["Design TOKEN_P71_R", "Ada", "Done"],
        ["Build", "Lin", "WIP"],
        ["Test", "Sam", "Todo"],
        ["Ship", "Pat", "Todo"],
    ]
    draw_grid(c, 80, PAGE_H - 280, [100, 70, 70], [20] * 5, cells)
    c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "frame_and_table",
            "description": "Full page border + header rule + one table",
            "challenges": ["page_border", "over_detect", "noise_lines"],
            "failure_modes_covered": ["OVER_DETECT", "MEGA_GRID"],
            "must_contain": ["TOKEN_P71_DOC", "TOKEN_P71_R"],
            "expected_tables": [{"cells": cells}],
            "page_count": 1,
        },
    )
    print(" ", doc_id)


# ─── 72 two tables + margin ticks ────────────────────────────────────────────
def gen_72():
    doc_id = "72_two_tables_margin_ticks"
    path = PREC / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setFont("Helvetica-Bold", 11)
    c.drawString(50, PAGE_H - 40, "Ticks Noise TOKEN_P72_DOC")
    c.setStrokeColor(colors.Color(0.55, 0.55, 0.6))
    c.setLineWidth(0.4)
    for y in range(120, 700, 14):
        c.line(36, y, 70, y)  # left margin ticks
    t1 = [
        ["A", "B"],
        ["1 TOKEN_P72_T1", "2"],
        ["3", "4"],
        ["5", "6"],
    ]
    t2 = [
        ["X", "Y", "Z"],
        ["a TOKEN_P72_T2", "b", "c"],
        ["d", "e", "f"],
        ["g", "h", "i"],
        ["j", "k", "l"],
    ]
    draw_grid(c, 100, PAGE_H - 220, [50, 50], [18] * 4, t1)
    draw_grid(c, 100, PAGE_H - 420, [50, 50, 50], [18] * 5, t2)
    c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "two_tables_margin_ticks",
            "description": "Two lattices + left-margin tick marks (must not form tables)",
            "challenges": ["over_detect", "noise_lines", "multiple_tables_per_page"],
            "failure_modes_covered": ["OVER_DETECT", "MULTI_TABLE_PAGE"],
            "must_contain": ["TOKEN_P72_DOC", "TOKEN_P72_T1", "TOKEN_P72_T2"],
            "expected_tables": [{"cells": t1}, {"cells": t2}],
            "page_count": 1,
        },
    )
    print(" ", doc_id)


# ─── 73 false row lines (underlines between data) ────────────────────────────
def gen_73():
    doc_id = "73_false_row_underlines"
    path = PREC / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setFont("Helvetica-Bold", 11)
    c.drawString(50, PAGE_H - 40, "Underline Rows TOKEN_P73_DOC")
    # Real 4-row table (header + 3) but with extra mid-cell underlines drawn shorter
    cells = [
        ["Name", "Score"],
        ["Ada TOKEN_P73_A", "10"],
        ["Lin TOKEN_P73_L", "20"],
        ["Sam", "30"],
    ]
    # Proper grid
    draw_grid(c, 80, PAGE_H - 220, [120, 60], [22, 22, 22, 22], cells)
    # Extra short H segments that might be misread as full row lines if over-eager
    c.setStrokeColor(colors.black)
    c.setLineWidth(0.4)
    # midway in row gaps - already have grid; add deco underlines OUTSIDE table
    c.line(80, PAGE_H - 250, 200, PAGE_H - 250)
    c.line(80, PAGE_H - 280, 180, PAGE_H - 280)
    c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "false_row_underlines",
            "description": "4×2 lattice plus stray underlines below; must stay 4×2 one table",
            "challenges": ["phantom_lines", "row_miscount", "over_detect"],
            "failure_modes_covered": ["ROW_MISCOUNT", "OVER_DETECT", "WRONG_SHAPE"],
            "must_contain": ["TOKEN_P73_DOC", "TOKEN_P73_A", "TOKEN_P73_L"],
            "expected_tables": [{"cells": cells}],
            "page_count": 1,
        },
    )
    print(" ", doc_id)


# ─── 74 sparse filled grid ───────────────────────────────────────────────────
def gen_74():
    doc_id = "74_sparse_data_grid"
    path = PREC / f"{doc_id}.pdf"
    cells = [["R\\C", "C1", "C2", "C3", "C4", "C5"]]
    for i in range(1, 9):
        row = [f"R{i}"]
        for j in range(1, 6):
            if (i + j) % 3 == 0:
                row.append(f"{i}.{j}" if not (i == 1 and j == 1) else "TOKEN_P74_HIT")
            else:
                row.append("")
        cells.append(row)
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setFont("Helvetica-Bold", 11)
    c.drawString(40, PAGE_H - 40, "Sparse Matrix TOKEN_P74_DOC")
    draw_grid(c, 40, PAGE_H - 320, [40, 50, 50, 50, 50, 50], [16] * 9, cells, fs=7)
    c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "sparse_data_grid",
            "description": "9×6 ruled grid with many empty cells (must keep, not FP-scrub)",
            "challenges": ["sparse_fill", "fp_scrub_false_negative"],
            "failure_modes_covered": ["MISS_ALL", "UNDER_DETECT"],
            "must_contain": ["TOKEN_P74_DOC", "TOKEN_P74_HIT"],
            "expected_tables": [{"cells": cells}],
            "page_count": 1,
        },
    )
    print(" ", doc_id)


# ─── 75 prose looks like 2-col stream ────────────────────────────────────────
def gen_75():
    doc_id = "75_prose_not_stream"
    path = PREC / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setFont("Helvetica-Bold", 12)
    c.drawString(50, PAGE_H - 40, "Narrative TOKEN_P75_DOC")
    c.setFont("Helvetica", 10)
    y = PAGE_H - 80
    lines = [
        ("1.", "First point about methodology TOKEN_P75_A with long explanation text here."),
        ("2.", "Second point continues the discussion TOKEN_P75_B without forming a table."),
        ("3.", "Third point remains prose TOKEN_P75_C and should not be extracted as stream."),
        ("4.", "Fourth paragraph elaborates further on sampling and error bounds."),
        ("5.", "Fifth item wraps up the list without multi-column alignment intent."),
        ("6.", "Sixth closing remark TOKEN_P75_END for the section."),
    ]
    for num, text in lines:
        c.drawString(50, y, num)
        # wrap-ish single column body (not multi-col table)
        c.drawString(80, y, text[:85])
        y -= 28
    c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "prose_not_stream",
            "description": "Numbered prose list — expect ZERO tables",
            "challenges": ["stream_fp", "over_detect", "prose_rejection"],
            "failure_modes_covered": ["OVER_DETECT"],
            "must_contain": ["TOKEN_P75_DOC", "TOKEN_P75_A", "TOKEN_P75_END"],
            "expected_tables": [],
            "expected_table_count": 0,
            "page_count": 1,
            "quality_notes": "Success: 0 tables. Any stream table is a false positive.",
        },
    )
    print(" ", doc_id)


# ─── 76 caption not table ────────────────────────────────────────────────────
def gen_76():
    doc_id = "76_caption_not_table"
    path = PREC / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setFont("Helvetica-Bold", 11)
    c.drawString(50, PAGE_H - 40, "Report TOKEN_P76_DOC")
    # tiny 2x2 ruled "Table 1." chrome
    tiny = [["Table", "1."], ["TOKEN_P76_CAP", "fig"]]
    draw_grid(c, 50, PAGE_H - 120, [50, 40], [14, 14], tiny, fs=7)
    cells = [
        ["Metric", "Value"],
        ["Revenue TOKEN_P76_R", "100"],
        ["Cost", "40"],
        ["Profit", "60"],
    ]
    draw_grid(c, 50, PAGE_H - 280, [120, 60], [18] * 4, cells)
    c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "caption_not_table",
            "description": "Tiny caption-like grid + one real data table; prefer 1 table (data)",
            "challenges": ["over_detect", "caption_fp", "form_like"],
            "failure_modes_covered": ["OVER_DETECT"],
            "must_contain": ["TOKEN_P76_DOC", "TOKEN_P76_R", "TOKEN_P76_CAP"],
            # Gold: only the data table (precision suite). Caption is intentional FP trap.
            "expected_tables": [{"cells": cells}],
            "page_count": 1,
            "quality_notes": "Ideal n=1 (data). Emitting caption 2×2 as second table fails detect F1.",
        },
    )
    print(" ", doc_id)


# ─── 77 three stacked, non-connecting verticals ──────────────────────────────
def gen_77():
    doc_id = "77_three_stacked_disjoint"
    path = PREC / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setFont("Helvetica-Bold", 11)
    c.drawString(50, PAGE_H - 36, "Three Stacked TOKEN_P77_DOC")
    t1 = [["H1", "H2"], ["a TOKEN_P77_1", "b"], ["c", "d"], ["e", "f"]]
    t2 = [["P", "Q", "R"], ["1 TOKEN_P77_2", "2", "3"], ["4", "5", "6"]]
    t3 = [["X", "Y"], ["u TOKEN_P77_3", "v"], ["w", "x"], ["y", "z"], ["m", "n"]]
    draw_grid(c, 50, PAGE_H - 180, [60, 60], [16] * 4, t1)
    draw_grid(c, 50, PAGE_H - 320, [50, 50, 50], [16] * 3, t2)
    draw_grid(c, 50, PAGE_H - 500, [70, 70], [16] * 5, t3)
    c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "three_stacked_disjoint",
            "description": "Three disjoint stacked lattices (4×2, 3×3, 5×2)",
            "challenges": ["multiple_tables_per_page", "anti_mega_grid", "over_detect"],
            "failure_modes_covered": ["MULTI_TABLE_PAGE", "UNDER_DETECT", "OVER_DETECT"],
            "must_contain": ["TOKEN_P77_DOC", "TOKEN_P77_1", "TOKEN_P77_2", "TOKEN_P77_3"],
            "expected_tables": [{"cells": t1}, {"cells": t2}, {"cells": t3}],
            "page_count": 1,
        },
    )
    print(" ", doc_id)


# ─── 78 multipage + footer rule ──────────────────────────────────────────────
def gen_78():
    doc_id = "78_multipage_footer_rules"
    path = PREC / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    header = ["Date", "Ref", "Amt"]
    rows = [[f"01-{i:02d}", f"R{i}", f"{i*1.5:.1f}"] for i in range(1, 25)]
    chunks = [rows[0:8], rows[8:16], rows[16:24]]
    for pi, chunk in enumerate(chunks):
        c.setFont("Helvetica-Bold", 10)
        c.drawString(50, PAGE_H - 36, f"Ledger TOKEN_P78_DOC p{pi+1}")
        cells = [header] + [
            [a, b, d if i != 0 or pi != 0 else f"{d} TOKEN_P78_R1"]
            if not (pi == 0 and i == 0)
            else [a, b, f"{d}"]
            for i, (a, b, d) in enumerate(chunk)
        ]
        # fix token on first data row page 0
        if pi == 0:
            cells[1][2] = cells[1][2] + " TOKEN_P78_R1"
        if pi == 2:
            cells[-1][1] = cells[-1][1] + " TOKEN_P78_END"
        draw_grid(c, 50, 100, [70, 50, 50], [14] * len(cells), cells, fs=7)
        # footer rule across page (noise)
        c.setStrokeColor(colors.Color(0.4, 0.4, 0.45))
        c.setLineWidth(0.8)
        c.line(40, 50, PAGE_W - 40, 50)
        c.setFillColor(colors.black)
        c.setFont("Helvetica", 7)
        c.drawString(40, 38, f"Confidential footer line {pi+1}")
        c.showPage()
    c.save()
    page_tables = []
    for pi, chunk in enumerate(chunks):
        cells = [header] + chunk
        if pi == 0:
            cells[1][2] = str(cells[1][2]) + " TOKEN_P78_R1"
        if pi == 2:
            cells[-1][1] = str(cells[-1][1]) + " TOKEN_P78_END"
        page_tables.append({"cells": cells})
    write_gt(
        doc_id,
        {
            "category": "multipage_footer_rules",
            "description": "3-page ledger + footer rules; 3 tables 9×3 (header+8)",
            "challenges": ["multi_page_table", "footer_noise", "over_detect"],
            "failure_modes_covered": ["MULTI_PAGE_DOC", "OVER_DETECT", "ROW_MISCOUNT"],
            "must_contain": ["TOKEN_P78_DOC", "TOKEN_P78_R1", "TOKEN_P78_END"],
            "expected_tables": page_tables,
            "page_count": 3,
        },
    )
    print(" ", doc_id)


# ─── 79 span header ──────────────────────────────────────────────────────────
def gen_79():
    doc_id = "79_span_header_precision"
    path = PREC / f"{doc_id}.pdf"
    visual = [
        [p("Metric", "PHdr"), p("FY24 TOKEN_P79_FY", "PHdr"), p("", "PHdr"), p("Notes", "PHdr")],
        [p(""), p("Act", "PHdr"), p("Bud", "PHdr"), p("")],
        [p("Rev"), p("10"), p("9"), p("TOKEN_P79_REV")],
        [p("Cost"), p("4"), p("5"), p("ops")],
        [p("NI"), p("6"), p("4"), p("TOKEN_P79_NI")],
    ]
    t = Table(visual, colWidths=[70, 50, 50, 90])
    t.setStyle(
        TableStyle(
            [
                ("GRID", (0, 0), (-1, -1), 0.6, colors.black),
                ("SPAN", (1, 0), (2, 0)),
                ("SPAN", (0, 0), (0, 1)),
                ("SPAN", (3, 0), (3, 1)),
                ("BACKGROUND", (0, 0), (-1, 1), colors.Color(0.88, 0.88, 0.9)),
                ("FONTSIZE", (0, 0), (-1, -1), 8),
            ]
        )
    )
    SimpleDocTemplate(str(path), pagesize=letter, leftMargin=48).build(
        [
            Paragraph("Span Header TOKEN_P79_DOC", styles["PTitle"]),
            Spacer(1, 10),
            t,
        ]
    )
    cells = [
        ["Metric", "FY24 TOKEN_P79_FY", "", "Notes"],
        ["", "Act", "Bud", ""],
        ["Rev", "10", "9", "TOKEN_P79_REV"],
        ["Cost", "4", "5", "ops"],
        ["NI", "6", "4", "TOKEN_P79_NI"],
    ]
    write_gt(
        doc_id,
        {
            "category": "span_header_precision",
            "description": "Colspan group header FY24 over Act/Bud",
            "challenges": ["colspan", "merged_headers", "wrong_shape"],
            "failure_modes_covered": ["WRONG_SHAPE", "COL_MISCOUNT", "BAD_STRUCTURE"],
            "must_contain": ["TOKEN_P79_DOC", "TOKEN_P79_FY", "TOKEN_P79_REV", "TOKEN_P79_NI"],
            "expected_tables": [{"cells": cells}],
            "page_count": 1,
        },
    )
    print(" ", doc_id)


# ─── 80 form boxes above data table ──────────────────────────────────────────
def gen_80():
    doc_id = "80_form_boxes_above_table"
    path = PREC / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setFont("Helvetica-Bold", 11)
    c.drawString(50, PAGE_H - 40, "Application TOKEN_P80_DOC")
    # form-like empty ruled boxes
    for i, label in enumerate(["Name", "Date", "ID"]):
        x = 50 + i * 120
        c.setStrokeColor(colors.black)
        c.setLineWidth(0.7)
        c.rect(x, PAGE_H - 120, 100, 36)
        c.setFont("Helvetica", 8)
        c.drawString(x + 4, PAGE_H - 95, label)
    cells = [
        ["SKU", "Qty", "Price"],
        ["A1 TOKEN_P80_A", "2", "9.99"],
        ["B2", "5", "1.50"],
        ["C3 TOKEN_P80_C", "1", "20.00"],
        ["D4", "3", "4.25"],
    ]
    draw_grid(c, 50, PAGE_H - 320, [80, 50, 60], [18] * 5, cells)
    c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "form_boxes_above_table",
            "description": "Empty form field boxes + one data table; expect 1 table only",
            "challenges": ["form_fp", "over_detect", "empty_grid"],
            "failure_modes_covered": ["OVER_DETECT"],
            "must_contain": ["TOKEN_P80_DOC", "TOKEN_P80_A", "TOKEN_P80_C"],
            "expected_tables": [{"cells": cells}],
            "page_count": 1,
        },
    )
    print(" ", doc_id)


# ─── 81 phantom vertical lines through columns ───────────────────────────────
def gen_81():
    doc_id = "81_phantom_verticals"
    path = PREC / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setFont("Helvetica-Bold", 11)
    c.drawString(50, PAGE_H - 40, "Phantom V TOKEN_P81_DOC")
    cells = [
        ["City", "Temp", "Hum"],
        ["NYC TOKEN_P81_N", "72", "55"],
        ["SF", "68", "70"],
        ["CHI TOKEN_P81_C", "70", "48"],
        ["MIA", "84", "80"],
    ]
    # 5 rows × 3 cols
    draw_grid(c, 80, PAGE_H - 240, [80, 50, 50], [18] * 5, cells)
    # short vertical noise segments that don't span full table height (phantom cols)
    c.setStrokeColor(colors.black)
    c.setLineWidth(0.5)
    c.line(155, PAGE_H - 100, 155, PAGE_H - 130)  # short tick mid-col
    c.line(200, PAGE_H - 160, 200, PAGE_H - 190)
    c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "phantom_verticals",
            "description": "True 5×3 table + short non-spanning V ticks; must stay 3 cols",
            "challenges": ["phantom_lines", "col_miscount", "joint_support"],
            "failure_modes_covered": ["COL_MISCOUNT", "WRONG_SHAPE"],
            "must_contain": ["TOKEN_P81_DOC", "TOKEN_P81_N", "TOKEN_P81_C"],
            "expected_tables": [{"cells": cells}],
            "page_count": 1,
        },
    )
    print(" ", doc_id)


# ─── 82 stream gap boundary ──────────────────────────────────────────────────
def gen_82():
    doc_id = "82_stream_gap_boundary"
    path = PREC / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setFont("Helvetica-Bold", 11)
    c.drawString(50, PAGE_H - 40, "Stream Gap TOKEN_P82_DOC")
    c.setFont("Helvetica", 9)
    y = PAGE_H - 70
    t1 = [
        ["Name", "Role", "Loc"],
        ["Ada TOKEN_P82_T1", "Eng", "NYC"],
        ["Lin", "PM", "SF"],
        ["Sam", "SRE", "AUS"],
        ["Pat", "Des", "BOS"],
    ]
    for row in t1:
        c.drawString(50, y, row[0])
        c.drawString(160, y, row[1])
        c.drawString(240, y, row[2])
        y -= 14
    # large prose gap
    y -= 40
    c.drawString(50, y, "Interstitial prose TOKEN_P82_MID that separates two borderless tables.")
    y -= 50
    t2 = [
        ["City", "Pop"],
        ["NYC TOKEN_P82_T2", "8.3"],
        ["SF", "0.8"],
        ["CHI", "2.7"],
        ["MIA", "0.5"],
        ["DEN", "0.7"],
    ]
    for row in t2:
        c.drawString(50, y, row[0])
        c.drawString(160, y, row[1])
        y -= 14
    c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "stream_gap_boundary",
            "description": "Two stream tables with large prose gap (6×3 and 6×2)",
            "challenges": ["stream", "multiple_tables_per_page", "prose_rejection"],
            "failure_modes_covered": ["UNDER_DETECT", "OVER_DETECT", "MULTI_TABLE_PAGE"],
            "must_contain": ["TOKEN_P82_DOC", "TOKEN_P82_T1", "TOKEN_P82_T2", "TOKEN_P82_MID"],
            "expected_tables": [{"cells": t1}, {"cells": t2}],
            "page_count": 1,
        },
    )
    print(" ", doc_id)


GENERATORS = [
    gen_70,
    gen_71,
    gen_72,
    gen_73,
    gen_74,
    gen_75,
    gen_76,
    gen_77,
    gen_78,
    gen_79,
    gen_80,
    gen_81,
    gen_82,
]


def update_suites_json(ids: list[str]) -> None:
    suites_path = CORPUS / "suites.json"
    if suites_path.exists():
        data = json.loads(suites_path.read_text(encoding="utf-8"))
    else:
        data = {"version": 1, "policy": {}, "suites": {}}
    data.setdefault("suites", {})
    data["suites"]["regression_precision"] = {
        "description": (
            "Precision/structure suite (70–82). OVER_DETECT, phantom lines, stream FPs, "
            "sparse grids — ICDAR-like modes without ICDAR files."
        ),
        "tiers": ["hard_precision"],
        "document_ids": ids,
    }
    data.setdefault("policy", {})
    data["policy"]["regression_precision_purpose"] = (
        "Day-to-day gate for FP control and structure work. Not ICDAR."
    )
    suites_path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")


def rebuild_manifest() -> None:
    items = []
    for p in sorted(GT.glob("*.json")):
        items.append(json.loads(p.read_text(encoding="utf-8")))
    tiers = {}
    for i in items:
        t = i.get("tier") or "unknown"
        tiers[t] = tiers.get(t, 0) + 1
    manifest = {
        "name": "pdfparser-competitive-corpus-v3",
        "description": "Basic + stress + hard + hard_precision synthetic; optional real. No ICDAR.",
        "document_count": len(items),
        "tiers": tiers,
        "suites_file": "corpus/suites.json",
        "documents": items,
    }
    (CORPUS / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"Manifest: {len(items)} docs tiers={tiers}")


def main() -> None:
    print("Generating hard_precision suite (70–82)...")
    for gen in GENERATORS:
        gen()
    ids = sorted(p.stem for p in PREC.glob("*.pdf"))
    update_suites_json(ids)
    rebuild_manifest()
    print(f"Done. {len(ids)} precision fixtures.")
    print("Run:")
    print(
        "  python benchmark/scripts/run_accuracy_benchmark.py "
        "--suite regression_precision --libs pdfparser,pdfplumber,camelot_lattice,camelot_auto"
    )


if __name__ == "__main__":
    main()
