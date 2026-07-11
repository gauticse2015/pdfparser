#!/usr/bin/env python3
"""Generate RICH competitive-gap synthetic corpus (owned regression).

Suite: regression_compete
Path:  corpus/compete_synthetic/C###_*.pdf
Gold:  ground_truth/C###_*.json

Policy: original content only; modes from ICDAR failure taxonomy;
        NOT a copy of competition PDFs. See docs/compete-dataset-design.md.

  python benchmark/scripts/generate_compete_corpus.py
  python benchmark/scripts/run_accuracy_benchmark.py --suite regression_compete \\
    --libs pdfparser --tag compete
"""
from __future__ import annotations

import json
from pathlib import Path

from reportlab.lib import colors
from reportlab.lib.pagesizes import letter, landscape
from reportlab.pdfgen import canvas
from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, Table, TableStyle, PageBreak
from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "corpus" / "compete_synthetic"
GT = ROOT / "ground_truth"
SUITES = ROOT / "corpus" / "suites.json"
for d in (CORPUS, GT):
    d.mkdir(parents=True, exist_ok=True)

PAGE_W, PAGE_H = letter
styles = getSampleStyleSheet()
styles.add(ParagraphStyle(name="CT", fontName="Helvetica-Bold", fontSize=11, leading=13))
styles.add(ParagraphStyle(name="CM", fontName="Helvetica", fontSize=8, leading=10))

IDS: list[str] = []


def write_gt(doc_id: str, data: dict) -> None:
    payload = {
        "id": doc_id,
        "tier": "compete",
        "suite": "regression_compete",
        "source": "synthetic",
        "icdar_derived": False,
        "path": f"corpus/compete_synthetic/{doc_id}.pdf",
        "weight_text": 0.10,
        "weight_tables": 0.90,
        "weight_objects": 0.0,
        "expected_images": 0,
        "development_role": "open_struggle",
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
    (GT / f"{doc_id}.json").write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    IDS.append(doc_id)


def stroke_grid(c, xs, ys, lw=0.6):
    c.setStrokeColor(colors.black)
    c.setLineWidth(lw)
    for y in ys:
        c.line(xs[0], y, xs[-1], y)
    for x in xs:
        c.line(x, ys[-1], x, ys[0])


def fill_text(c, xs, ys, cells, fs=7.0):
    for r, row in enumerate(cells):
        for col, txt in enumerate(row):
            if not txt:
                continue
            c.setFont("Helvetica-Bold" if r == 0 else "Helvetica", fs)
            c.drawString(xs[col] + 2, (ys[r] + ys[r + 1]) / 2 - fs * 0.3, str(txt)[:28])


def thin_rect_grid(c, xs, ys, th=1.0):
    c.setFillColor(colors.black)
    for y in ys:
        c.rect(xs[0], y - th / 2, xs[-1] - xs[0], th, fill=1, stroke=0)
    for x in xs:
        c.rect(x - th / 2, ys[-1], th, ys[0] - ys[-1], fill=1, stroke=0)


def mk_cells(nr, nc, prefix, token_row=1, token="TOK"):
    hdr = [f"H{j}" for j in range(nc)]
    if nc > 0:
        hdr[0] = "Key"
    rows = [hdr]
    for i in range(1, nr):
        row = [f"{prefix}{i:02d}"] + [str(i * 10 + j) for j in range(1, nc)]
        if i == token_row and nc > 1:
            row[-1] = f"{token}"
        rows.append(row)
    return rows


# ═══════════════════════════════════════════════════════════════════════════
# C1/C2 OVER_DETECT variants
# ═══════════════════════════════════════════════════════════════════════════

def gen_overdetect_family():
    # C001-C008: one real table + escalating chrome
    for k, n_deco in enumerate([3, 6, 10, 15], start=1):
        doc_id = f"C{k:03d}_overdetect_deco_h{n_deco}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 11)
        c.drawString(40, PAGE_H - 36, f"Chrome Soup TOKEN_{doc_id[:8]} DOC")
        c.setStrokeColor(colors.Color(0.5, 0.5, 0.55))
        c.setLineWidth(0.45)
        for i in range(n_deco):
            y = PAGE_H - 60 - i * 8
            c.line(36, y, PAGE_W - 36, y)
            c.line(36, 40 + i * 6, PAGE_W - 36, 40 + i * 6)
        # margin ticks
        for y in range(100, 500, 20):
            c.line(30, y, 42, y)
            c.line(PAGE_W - 42, y, PAGE_W - 30, y)
        # page frame
        c.setLineWidth(1.0)
        c.rect(24, 24, PAGE_W - 48, PAGE_H - 48, stroke=1, fill=0)
        cells = mk_cells(5, 4, "R", 2, f"TOKEN_C{k:03d}_A")
        xs = [100, 170, 230, 290, 370]
        ys = [PAGE_H - 200 - i * 16 for i in range(6)]
        c.setStrokeColor(colors.black)
        stroke_grid(c, xs, ys)
        fill_text(c, xs, ys, cells)
        c.setFont("Helvetica", 7)
        c.drawString(40, 28, f"TOKEN_C{k:03d}_FOOT expect n=1 shape 5x4")
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "over_detect_chrome",
                "failure_classes": ["C1_OVER_DETECT", "C2_SEVERE_OVER_DETECT"] if n_deco >= 10 else ["C1_OVER_DETECT"],
                "description": f"One 5x4 lattice + {n_deco} deco H-rules, frame, margin ticks",
                "challenges": ["over_detect", "decorative_rules", "page_border"],
                "must_contain": [f"TOKEN_C{k:03d}_A", f"TOKEN_C{k:03d}_FOOT"],
                "expected_tables": [{"cells": cells}],
                "page_count": 1,
            },
        )
        print(" ", doc_id)

    # C009-C012: false stream bait columns + one lattice
    for k, nrows in enumerate([8, 12, 16, 20], start=9):
        doc_id = f"C{k:03d}_overdetect_stream_bait_n{nrows}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 11)
        c.drawString(40, PAGE_H - 36, f"Stream Bait TOKEN_C{k:03d}_DOC")
        y = PAGE_H - 80
        for i in range(nrows):
            c.setFont("Helvetica", 8)
            c.drawString(50, y, f"L{i:02d} word{i} TOKEN_C{k:03d}_L" if i == 0 else f"L{i:02d} word{i}")
            c.drawString(320, y, f"R{i:02d} term{i} TOKEN_C{k:03d}_R" if i == 0 else f"R{i:02d} term{i}")
            y -= 14
        cells = mk_cells(4, 3, "D", 1, f"TOKEN_C{k:03d}_T")
        xs = [180, 240, 300, 360]
        ys = [160 - i * 16 for i in range(5)]
        stroke_grid(c, xs, ys)
        fill_text(c, xs, ys, cells)
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "over_detect_stream_bait",
                "failure_classes": ["C1_OVER_DETECT"],
                "description": f"2-col word list ({nrows} rows) + one 4x3 lattice",
                "challenges": ["stream_fp", "over_detect"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC", f"TOKEN_C{k:03d}_T"],
                "expected_tables": [{"cells": cells}],
                "expected_table_count": 1,
                "page_count": 1,
            },
        )
        print(" ", doc_id)


# ═══════════════════════════════════════════════════════════════════════════
# C3 UNDER_DETECT / painted / sparse rules
# ═══════════════════════════════════════════════════════════════════════════

def gen_underdetect_family():
    # C013-C018: painted thin rect grids various sizes
    for k, (nr, nc) in enumerate([(5, 4), (6, 5), (8, 4), (10, 6), (7, 3), (12, 5)], start=13):
        doc_id = f"C{k:03d}_painted_rect_{nr}x{nc}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 11)
        c.drawString(40, PAGE_H - 36, f"Painted {nr}x{nc} TOKEN_C{k:03d}_DOC")
        cells = mk_cells(nr, nc, "P", 2, f"TOKEN_C{k:03d}_A")
        col_w = [min(70, 400 // nc)] * nc
        xs = [80.0]
        for w in col_w:
            xs.append(xs[-1] + w)
        ys = [PAGE_H - 100 - i * 15 for i in range(nr + 1)]
        thin_rect_grid(c, xs, ys, th=0.9)
        fill_text(c, xs, ys, cells, fs=6.5)
        # jitter some body text x
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "painted_rules",
                "failure_classes": ["C3_UNDER_DETECT", "C10_COUNT_OK_STRUCT_BAD"],
                "description": f"Thin-fill-rect ruled table {nr}x{nc}",
                "challenges": ["painted_rules", "thin_rect", "lattice_sensing"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC", f"TOKEN_C{k:03d}_A"],
                "expected_tables": [{"cells": cells}],
                "page_count": 1,
            },
        )
        print(" ", doc_id)

    # C019-C024: partial H (every 2,3,4)
    for k, step in enumerate([2, 3, 4, 2, 3, 4], start=19):
        nr, nc = 12, 5 if k % 2 else 6
        doc_id = f"C{k:03d}_partial_h_step{step}_{nr}x{nc}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 11)
        c.drawString(40, PAGE_H - 36, f"Partial H step={step} TOKEN_C{k:03d}_DOC")
        cells = mk_cells(nr, nc, "M", 1, f"TOKEN_C{k:03d}_R1")
        cells[-1][-1] = f"TOKEN_C{k:03d}_LAST"
        xs = [70 + i * (420 // nc) for i in range(nc + 1)]
        ys = [PAGE_H - 90 - i * 14 for i in range(nr + 1)]
        c.setStrokeColor(colors.black)
        c.setLineWidth(0.65)
        for x in xs:
            c.line(x, ys[-1], x, ys[0])
        for i, y in enumerate(ys):
            if i == 0 or i == len(ys) - 1 or i % step == 0:
                c.line(xs[0], y, xs[-1], y)
        fill_text(c, xs, ys, cells, fs=6.5)
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "partial_hlines",
                "failure_classes": ["C5_HEADER_ONLY_SLICE", "C10_COUNT_OK_STRUCT_BAD", "C3_UNDER_DETECT"],
                "description": f"Full V, H every {step}rd line, gold {nr}x{nc}",
                "challenges": ["partial_hlines", "row_undercount", "text_row_recovery"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC", f"TOKEN_C{k:03d}_R1", f"TOKEN_C{k:03d}_LAST"],
                "expected_tables": [{"cells": cells}],
                "page_count": 1,
            },
        )
        print(" ", doc_id)


# ═══════════════════════════════════════════════════════════════════════════
# C4/C5 large borderless + header slices
# ═══════════════════════════════════════════════════════════════════════════

def gen_borderless_family():
    # C025-C036: large borderless various sizes + optional prose gap
    configs = [
        (20, 6, False),
        (25, 8, False),
        (30, 8, True),
        (35, 10, True),
        (40, 6, False),
        (28, 12, True),
        (22, 5, False),
        (32, 7, True),
        (18, 9, False),
        (45, 5, True),
        (26, 8, False),
        (38, 8, True),
    ]
    for k, (nr, nc, gap) in enumerate(configs, start=25):
        doc_id = f"C{k:03d}_borderless_{nr}x{nc}{'_gap' if gap else ''}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 10)
        c.drawString(30, PAGE_H - 32, f"Borderless {nr}x{nc} TOKEN_C{k:03d}_DOC")
        cells = mk_cells(nr, nc, "B", 1, f"TOKEN_C{k:03d}_R1")
        mid = nr // 2
        cells[mid][1] = f"TOKEN_C{k:03d}_MID"
        cells[-1][1] = f"TOKEN_C{k:03d}_LAST"
        col_w = max(28, min(55, 500 // nc))
        xs = [30.0]
        for _ in range(nc):
            xs.append(xs[-1] + col_w)
        y_top = PAGE_H - 55
        row_h = min(11.0, (PAGE_H - 120) / nr)
        c.setFont("Helvetica", 6)
        half = nr // 2 if gap else nr
        for r in range(nr):
            if gap and r == half:
                y_top -= 18
                c.setFont("Helvetica-Oblique", 7)
                c.drawString(30, y_top, f"Interstitial TOKEN_C{k:03d}_GAP same table continues")
                y_top -= 14
                c.setFont("Helvetica", 6)
            y = y_top - r * row_h if not gap else (
                y_top - r * row_h if r < half else y_top - (r) * row_h
            )
            # simpler layout without double-counting gap
        # redraw cleanly
        c.showPage()  # clear botched - rebuild page
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 10)
        c.drawString(30, PAGE_H - 32, f"Borderless {nr}x{nc} TOKEN_C{k:03d}_DOC")
        y = PAGE_H - 55
        for r in range(nr):
            if gap and r == half:
                y -= 16
                c.setFont("Helvetica-Oblique", 7)
                c.drawString(30, y, f"Interstitial TOKEN_C{k:03d}_GAP continues")
                y -= 14
            c.setFont("Helvetica-Bold" if r == 0 else "Helvetica", 6)
            for col in range(nc):
                c.drawString(xs[col] + 1, y, str(cells[r][col])[:14])
            y -= row_h
            if y < 40:
                break
        c.setFont("Helvetica", 6)
        c.drawString(30, 24, f"TOKEN_C{k:03d}_FOOT gold {nr}x{nc} n=1")
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "large_borderless",
                "failure_classes": ["C4_ROW_FRAGMENT_LARGE", "C5_HEADER_ONLY_SLICE"]
                + (["C10_COUNT_OK_STRUCT_BAD"] if gap else []),
                "description": f"Borderless {nr}x{nc}" + (" with mid prose gap" if gap else ""),
                "challenges": ["borderless", "large_row_count", "network_class"]
                + (["prose_gap"] if gap else []),
                "must_contain": [
                    f"TOKEN_C{k:03d}_DOC",
                    f"TOKEN_C{k:03d}_R1",
                    f"TOKEN_C{k:03d}_LAST",
                ]
                + ([f"TOKEN_C{k:03d}_GAP"] if gap else []),
                "expected_tables": [{"cells": cells}],
                "expected_table_count": 1,
                "page_count": 1,
            },
        )
        print(" ", doc_id)


# ═══════════════════════════════════════════════════════════════════════════
# C6 wide columns
# ═══════════════════════════════════════════════════════════════════════════

def gen_wide_family():
    for k, (nr, nc) in enumerate([(8, 10), (10, 12), (12, 14), (6, 16), (15, 10), (20, 8)], start=37):
        doc_id = f"C{k:03d}_wide_{nr}x{nc}"
        path = CORPUS / f"{doc_id}.pdf"
        # landscape for wide
        c = canvas.Canvas(str(path), pagesize=landscape(letter))
        W, H = landscape(letter)
        c.setFont("Helvetica-Bold", 10)
        c.drawString(30, H - 30, f"Wide grid {nr}x{nc} TOKEN_C{k:03d}_DOC")
        cells = mk_cells(nr, nc, "W", 2, f"TOKEN_C{k:03d}_A")
        cw = max(22, min(40, int((W - 60) / nc)))
        xs = [30.0]
        for _ in range(nc):
            xs.append(xs[-1] + cw)
        ys = [H - 60 - i * 12 for i in range(nr + 1)]
        stroke_grid(c, xs, ys, lw=0.4)
        fill_text(c, xs, ys, cells, fs=5.5)
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "wide_statistical",
                "failure_classes": ["C6_COL_SKEW_WIDE", "C10_COUNT_OK_STRUCT_BAD"],
                "description": f"Wide lattice {nr}x{nc}",
                "challenges": ["wide_table", "many_columns", "lattice"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC", f"TOKEN_C{k:03d}_A"],
                "expected_tables": [{"cells": cells}],
                "page_count": 1,
            },
        )
        print(" ", doc_id)


# ═══════════════════════════════════════════════════════════════════════════
# C7 row overcount (false underlines)
# ═══════════════════════════════════════════════════════════════════════════

def gen_row_overcount_family():
    for k, n_extra in enumerate([5, 10, 15, 20], start=43):
        doc_id = f"C{k:03d}_false_underlines_x{n_extra}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 11)
        c.drawString(40, PAGE_H - 36, f"False underlines TOKEN_C{k:03d}_DOC")
        cells = mk_cells(6, 4, "U", 2, f"TOKEN_C{k:03d}_A")
        xs = [100, 180, 250, 320, 400]
        ys = [PAGE_H - 120 - i * 22 for i in range(7)]
        stroke_grid(c, xs, ys)
        fill_text(c, xs, ys, cells)
        # false H under each text baseline mid-cell
        c.setStrokeColor(colors.Color(0.3, 0.3, 0.35))
        c.setLineWidth(0.35)
        for i in range(n_extra):
            y = PAGE_H - 130 - i * 9
            c.line(xs[0] + 5, y, xs[-1] - 5, y)
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "false_underlines",
                "failure_classes": ["C7_ROW_OVERCOUNT", "C10_COUNT_OK_STRUCT_BAD"],
                "description": f"True 6x4 grid + {n_extra} false H underlines",
                "challenges": ["phantom_lines", "row_overcount"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC", f"TOKEN_C{k:03d}_A"],
                "expected_tables": [{"cells": cells}],
                "page_count": 1,
            },
        )
        print(" ", doc_id)


# ═══════════════════════════════════════════════════════════════════════════
# C8 multi-table heterogeneous
# ═══════════════════════════════════════════════════════════════════════════

def gen_multitable_family():
    for k, ntabs in enumerate([2, 3, 4, 5, 3, 4], start=47):
        doc_id = f"C{k:03d}_multitable_n{ntabs}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 11)
        c.drawString(40, PAGE_H - 32, f"Multi {ntabs} tables TOKEN_C{k:03d}_DOC")
        # deco
        c.setStrokeColor(colors.grey)
        for y in [PAGE_H - 50, 50]:
            c.line(40, y, PAGE_W - 40, y)
        gold_tables = []
        y_cursor = PAGE_H - 80
        shapes = [(4, 3), (5, 4), (6, 2), (4, 5), (5, 3), (3, 4)]
        for t_i in range(ntabs):
            nr, nc = shapes[t_i % len(shapes)]
            cells = mk_cells(nr, nc, f"T{t_i}", 1, f"TOKEN_C{k:03d}_T{t_i}")
            gold_tables.append({"cells": cells})
            xs = [50.0]
            for j in range(nc):
                xs.append(xs[-1] + (80 if j == 0 else 50))
            ys = [y_cursor - i * 14 for i in range(nr + 1)]
            c.setStrokeColor(colors.black)
            stroke_grid(c, xs, ys)
            fill_text(c, xs, ys, cells, fs=6.5)
            y_cursor = ys[-1] - 28
            if y_cursor < 80:
                break
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "multi_table_page",
                "failure_classes": ["C8_MULTI_TABLE_COUNT_ERROR", "C1_OVER_DETECT", "C12_ORDER_SENSITIVE"],
                "description": f"{len(gold_tables)} heterogeneous lattices one page + chrome",
                "challenges": ["multiple_tables_per_page", "heterogeneous_shapes", "order"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC"]
                + [f"TOKEN_C{k:03d}_T{i}" for i in range(len(gold_tables))],
                "expected_tables": gold_tables,
                "expected_table_count": len(gold_tables),
                "page_count": 1,
            },
        )
        print(" ", doc_id)


# ═══════════════════════════════════════════════════════════════════════════
# C9 multipage
# ═══════════════════════════════════════════════════════════════════════════

def gen_multipage_family():
    for k, npages in enumerate([2, 3, 4, 2], start=53):
        doc_id = f"C{k:03d}_multipage_p{npages}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        gold_tables = []
        for p in range(npages):
            c.setFont("Helvetica-Bold", 11)
            c.drawString(40, PAGE_H - 36, f"Page {p+1} TOKEN_C{k:03d}_P{p}")
            # continued table fragment
            nr, nc = 8, 4
            cells = mk_cells(nr, nc, f"P{p}", 1, f"TOKEN_C{k:03d}_P{p}A")
            gold_tables.append({"cells": cells, "page": p})
            xs = [80, 160, 230, 300, 380]
            ys = [PAGE_H - 100 - i * 16 for i in range(nr + 1)]
            stroke_grid(c, xs, ys)
            fill_text(c, xs, ys, cells)
            # footer rule chrome
            c.setStrokeColor(colors.grey)
            c.line(40, 40, PAGE_W - 40, 40)
            c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "multipage_tables",
                "failure_classes": ["C9_MULTIPAGE", "C8_MULTI_TABLE_COUNT_ERROR"],
                "description": f"{npages} pages each with one {nr}x{nc} lattice + footer rule",
                "challenges": ["multi_page_document", "footer_noise", "stitch_or_not"],
                "must_contain": [f"TOKEN_C{k:03d}_P{p}" for p in range(npages)],
                "expected_tables": gold_tables,
                "expected_table_count": npages,
                "page_count": npages,
            },
        )
        print(" ", doc_id)


# ═══════════════════════════════════════════════════════════════════════════
# C11 content assign / C13 mixed / C14 invoice / C15 spans
# ═══════════════════════════════════════════════════════════════════════════

def gen_struct_misc_family():
    # C057-C060 invoice style
    for k, n_items in enumerate([3, 5, 8, 4], start=57):
        doc_id = f"C{k:03d}_invoice_items{n_items}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 12)
        c.drawString(48, PAGE_H - 40, f"Invoice TOKEN_C{k:03d}_DOC")
        rows = [["SKU", "Description", "Qty", "Unit", "Amount"]]
        for i in range(1, n_items + 1):
            rows.append(
                [
                    f"SKU-{i}",
                    f"Item{i} TOKEN_C{k:03d}_I{i}" if i == 1 else f"Item{i}",
                    str(i),
                    str(10 * i),
                    str(10 * i * i),
                ]
            )
        rows.append(["", f"Subtotal TOKEN_C{k:03d}_SUB", "", "", "999"])
        rows.append(["", f"Total TOKEN_C{k:03d}_TOT", "", "", "999"])
        gold = rows[: n_items + 1]
        xs = [70, 130, 300, 350, 400, 470]
        ys = [PAGE_H - 100 - i * 18 for i in range(len(rows) + 1)]
        stroke_grid(c, xs, ys)
        fill_text(c, xs, ys, rows, fs=7)
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "invoice_footer",
                "failure_classes": ["C14_INVOICE_FOOTER", "C7_ROW_OVERCOUNT"],
                "description": f"Invoice {n_items} items; gold excludes totals rows",
                "challenges": ["invoice_totals", "footer_rows"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC", f"TOKEN_C{k:03d}_I1", f"TOKEN_C{k:03d}_TOT"],
                "expected_tables": [{"cells": gold}],
                "page_count": 1,
            },
        )
        print(" ", doc_id)

    # C061-C064 mixed lattice+stream
    for k, (lr, sr) in enumerate([(5, 6), (6, 8), (4, 10), (7, 5)], start=61):
        doc_id = f"C{k:03d}_mixed_lat{lr}_str{sr}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 11)
        c.drawString(40, PAGE_H - 36, f"Mixed TOKEN_C{k:03d}_DOC")
        # lattice top
        lat = mk_cells(lr, 4, "L", 1, f"TOKEN_C{k:03d}_L")
        xs = [60, 140, 210, 280, 360]
        ys = [PAGE_H - 80 - i * 15 for i in range(lr + 1)]
        stroke_grid(c, xs, ys)
        fill_text(c, xs, ys, lat)
        # stream bottom
        stream = mk_cells(sr, 3, "S", 1, f"TOKEN_C{k:03d}_S")
        y = ys[-1] - 40
        c.setFont("Helvetica", 7)
        for r, row in enumerate(stream):
            c.setFont("Helvetica-Bold" if r == 0 else "Helvetica", 7)
            c.drawString(60, y, row[0][:12])
            c.drawString(160, y, row[1][:12])
            c.drawString(260, y, row[2][:12])
            y -= 12
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "mixed_lattice_stream",
                "failure_classes": ["C13_MIXED_LATTICE_STREAM", "C8_MULTI_TABLE_COUNT_ERROR"],
                "description": f"Lattice {lr}x4 + borderless stream {sr}x3",
                "challenges": ["mixed", "multi_strategy"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC", f"TOKEN_C{k:03d}_L", f"TOKEN_C{k:03d}_S"],
                "expected_tables": [{"cells": lat}, {"cells": stream}],
                "expected_table_count": 2,
                "page_count": 1,
            },
        )
        print(" ", doc_id)

    # C065-C068 spans
    for k, span in enumerate([True, True, True, True], start=65):
        doc_id = f"C{k:03d}_span_header_v{k-64}"
        path = CORPUS / f"{doc_id}.pdf"
        # use platypus SPAN
        visual = [
            ["Metric", f"FY TOKEN_C{k:03d}_FY", "", "Notes"],
            ["", "Act", "Bud", ""],
            ["Rev", "10", "9", f"TOKEN_C{k:03d}_R"],
            ["Cost", "4", "5", "ops"],
            ["NI", "6", "4", f"TOKEN_C{k:03d}_N"],
        ]
        gold = [list(r) for r in visual]
        t = Table(visual, colWidths=[70, 55, 55, 90])
        t.setStyle(
            TableStyle(
                [
                    ("GRID", (0, 0), (-1, -1), 0.6, colors.black),
                    ("SPAN", (1, 0), (2, 0)),
                    ("SPAN", (0, 0), (0, 1)),
                    ("SPAN", (3, 0), (3, 1)),
                    ("FONTSIZE", (0, 0), (-1, -1), 8),
                ]
            )
        )
        SimpleDocTemplate(str(path), pagesize=letter, leftMargin=48).build(
            [Paragraph(f"Span TOKEN_C{k:03d}_DOC", styles["CT"]), Spacer(1, 10), t]
        )
        write_gt(
            doc_id,
            {
                "category": "span_complex",
                "failure_classes": ["C15_SPAN_COMPLEX", "C11_SAME_SHAPE_ZERO_TEDS"],
                "description": "Colspan/rowspan header grid 5x4",
                "challenges": ["colspan", "rowspan", "merged_headers"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC", f"TOKEN_C{k:03d}_FY", f"TOKEN_C{k:03d}_N"],
                "expected_tables": [{"cells": gold}],
                "page_count": 1,
            },
        )
        print(" ", doc_id)


def update_suites():
    if SUITES.exists():
        data = json.loads(SUITES.read_text())
    else:
        data = {"version": 1, "suites": {}, "policy": {}}
    data.setdefault("policy", {})
    data["policy"]["compete_suite"] = (
        "Rich competitive-gap corpus (synthetic). Modes from ICDAR taxonomy; "
        "no ICDAR files. Primary development dataset for F1/TEDS/row/col gaps."
    )
    data.setdefault("suites", {})
    data["suites"]["regression_compete"] = {
        "description": "Rich competitive-gap suite (C### synthetic + optional real). Full grids.",
        "tiers": ["compete", "compete_real"],
        "document_ids": list(IDS),
        "icdar_derived": False,
        "scoring_guidance": (
            "Track detect F1, cell F1, shape, and per failure_classes tags. "
            "Suite should remain HARD for current pdfparser until Auto+Network land."
        ),
    }
    # also allow filter by tier compete
    SUITES.write_text(json.dumps(data, indent=2) + "\n")
    # class index
    index = {"version": 1, "n_docs": len(IDS), "document_ids": IDS, "by_class": {}}
    for doc_id in IDS:
        g = json.loads((GT / f"{doc_id}.json").read_text())
        for cl in g.get("failure_classes", []):
            index["by_class"].setdefault(cl, []).append(doc_id)
    (ROOT / "corpus" / "compete_class_index.json").write_text(json.dumps(index, indent=2) + "\n")
    print("suites + class index updated, n=", len(IDS))


def main():
    global IDS
    IDS = []
    print("Generating RICH compete synthetic corpus…")
    gen_overdetect_family()
    gen_underdetect_family()
    gen_borderless_family()
    gen_wide_family()
    gen_row_overcount_family()
    gen_multitable_family()
    gen_multipage_family()
    gen_struct_misc_family()
    update_suites()
    print(f"Done: {len(IDS)} synthetic compete docs")
    # coverage summary
    from collections import Counter
    c = Counter()
    for doc_id in IDS:
        g = json.loads((GT / f"{doc_id}.json").read_text())
        for cl in g.get("failure_classes", []):
            c[cl] += 1
    print("Class coverage counts:")
    for k, v in sorted(c.items()):
        print(f"  {k}: {v}")


if __name__ == "__main__":
    main()
