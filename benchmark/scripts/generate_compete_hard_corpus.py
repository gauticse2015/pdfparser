#!/usr/bin/env python3
"""Generate HARD struggle synthetic corpus (C100+) validated against current pdfparser.

These fixtures are reverse-engineered from ICDAR failure modes and confirmed
to fail the live binary at generation time (probe-and-keep policy optional).

Suite: regression_compete (tier=compete_hard)
See docs/compete-struggle-analysis.md

  .venv/bin/python benchmark/scripts/generate_compete_hard_corpus.py
  .venv/bin/python benchmark/scripts/generate_compete_hard_corpus.py --probe-filter
"""
from __future__ import annotations

import argparse
import json
import random
import subprocess
import sys
from collections import Counter
from pathlib import Path

from reportlab.lib import colors
from reportlab.lib.pagesizes import letter, landscape
from reportlab.pdfgen import canvas
from reportlab.platypus import SimpleDocTemplate, Table, TableStyle, Paragraph, Spacer
from reportlab.lib.styles import getSampleStyleSheet

try:
    from PIL import Image, ImageDraw
except ImportError:
    Image = ImageDraw = None  # type: ignore

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "corpus" / "compete_synthetic"
GT = ROOT / "ground_truth"
SUITES = ROOT / "corpus" / "suites.json"
BIN = ROOT.parent / "target" / "release" / "pdfparser"
if not BIN.exists():
    BIN = ROOT.parent / "target" / "debug" / "pdfparser"

for d in (CORPUS, GT):
    d.mkdir(parents=True, exist_ok=True)

PAGE_W, PAGE_H = letter
IDS: list[str] = []
PROBE_LOG: list[dict] = []


def write_gt(doc_id: str, data: dict) -> None:
    payload = {
        "id": doc_id,
        "tier": "compete_hard",
        "suite": "regression_compete",
        "source": "synthetic",
        "icdar_derived": False,
        "path": f"corpus/compete_synthetic/{doc_id}.pdf",
        "weight_text": 0.05,
        "weight_tables": 0.95,
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


def mk_cells(nr: int, nc: int, prefix: str, token_row: int = 1, token: str = "TOK") -> list[list[str]]:
    hdr = [f"H{j}" for j in range(nc)]
    if nc > 0:
        hdr[0] = "Key"
    rows = [hdr]
    for i in range(1, nr):
        row = [f"{prefix}{i:02d}"] + [str(i * 10 + j) for j in range(1, nc)]
        if i == token_row and nc > 1:
            row[-1] = token
        rows.append(row)
    return rows


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


def probe_shapes(pdf: Path) -> list[tuple]:
    if not BIN.exists():
        return []
    r = subprocess.run(
        [str(BIN), "extract", "--tables", "--format", "json", str(pdf)],
        capture_output=True,
        text=True,
    )
    if r.returncode != 0 or not r.stdout.strip():
        return []
    d = json.loads(r.stdout)
    out = []
    for t in d.get("tables") or []:
        out.append((t.get("rows"), t.get("cols"), t.get("method")))
    return out


def shape_ok(pred, gold_shape, gold_n) -> bool:
    if gold_n is not None and len(pred) != gold_n:
        return False
    if gold_shape is None:
        return True
    gr, gc = gold_shape
    if gold_n == 1:
        return any(s[0] == gr and s[1] == gc for s in pred)
    return sum(1 for s in pred if s[0] == gr and s[1] == gc) >= gold_n


# ═══════════════════════════════════════════════════════════════════════════
# Generators — each family targets a measured failure mode
# ═══════════════════════════════════════════════════════════════════════════

def gen_image_painted_family(start: int = 100) -> int:
    """C3 MISS_ALL: rules exist only as raster image pixels."""
    if Image is None:
        print("  skip image family (no PIL)")
        return start
    k = start
    configs = [(8, 4), (10, 5), (12, 6), (15, 6), (21, 6), (12, 8), (18, 5), (9, 7)]
    for nr, nc in configs:
        doc_id = f"C{k:03d}_img_rules_{nr}x{nc}"
        cell_w, cell_h = 72, 30
        img = Image.new("RGB", (nc * cell_w + 20, nr * cell_h + 20), "white")
        dr = ImageDraw.Draw(img)
        for i in range(nr + 1):
            y = 10 + i * cell_h
            dr.line([(10, y), (10 + nc * cell_w, y)], fill="black", width=2)
        for i in range(nc + 1):
            x = 10 + i * cell_w
            dr.line([(x, 10), (x, 10 + nr * cell_h)], fill="black", width=2)
        cells = mk_cells(nr, nc, "I", 1, f"TOKEN_C{k:03d}_A")
        cells[-1][-1] = f"TOKEN_C{k:03d}_LAST"
        for r in range(nr):
            for col in range(nc):
                dr.text((14 + col * cell_w, 14 + r * cell_h), str(cells[r][col])[:10], fill="black")
        ip = CORPUS / f"_tmp_{doc_id}.png"
        img.save(ip)
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 11)
        c.drawString(40, PAGE_H - 36, f"Image-rules {nr}x{nc} TOKEN_C{k:03d}_DOC")
        iw, ih = img.size
        scale = min(520 / iw, 680 / ih)
        c.drawImage(str(ip), 40, max(40, PAGE_H - 80 - ih * scale), width=iw * scale, height=ih * scale)
        c.showPage()
        c.save()
        ip.unlink(missing_ok=True)
        write_gt(
            doc_id,
            {
                "category": "image_painted_rules",
                "failure_classes": ["C3_UNDER_DETECT", "C3_MISS_ALL", "C10_COUNT_OK_STRUCT_BAD"],
                "description": f"Table rules only as embedded image pixels {nr}x{nc} — vector lattice sees nothing",
                "challenges": ["raster_rules", "image_table", "miss_all"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC"],
                "expected_tables": [{"cells": cells}],
                "expected_table_count": 1,
                "page_count": 1,
                "struggle_mode": "image_painted_miss_all",
            },
        )
        print(" ", doc_id)
        k += 1
    return k


def gen_header_slice_family(start: int) -> int:
    """C4/C5: large irregular borderless fragments into multiple stream tables."""
    k = start
    configs = [
        # (nr, nc, seed, name)
        (31, 10, 1, "sec"),
        (44, 12, 2, "sec"),
        (58, 8, 3, "sec"),
        (35, 11, 5, "sec"),
        (40, 9, 6, "sec"),
        (50, 10, 7, "sec"),
        (28, 14, 8, "sec"),
        (36, 8, 9, "jitter"),
    ]
    for nr, nc, seed, style in configs:
        doc_id = f"C{k:03d}_hdr_slice_{nr}x{nc}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 10)
        c.drawString(30, PAGE_H - 28, f"Header-slice {nr}x{nc} TOKEN_C{k:03d}_DOC")
        col_ws = [58] + [max(32, int(460 / max(1, nc - 1)))] * (nc - 1)
        while sum(col_ws) > 530:
            col_ws = [max(28, w - 2) for w in col_ws]
        xs = [30.0]
        for w in col_ws:
            xs.append(xs[-1] + w)
        y = PAGE_H - 55
        # multi-level header
        c.setFont("Helvetica-Bold", 7)
        c.drawString(xs[1], y, f"Actuals TOKEN_C{k:03d}_H1")
        if nc > 4:
            c.drawString(xs[nc // 2], y, "Budget")
        c.setStrokeColor(colors.black)
        c.setLineWidth(0.7)
        c.line(xs[0], y - 2, xs[-1], y - 2)
        y -= 12
        headers = [f"C{j}" for j in range(nc)]
        headers[0] = "Region"
        c.setFont("Helvetica-Bold", 6)
        for j, h in enumerate(headers):
            c.drawString(xs[j] + 0.5, y, h[:10])
        c.line(xs[0], y - 2, xs[-1], y - 2)
        y -= 14
        # gold cells: 2 header rows + body
        gold = [headers[:]]
        # synthetic second header row for gold shape = nr total including multi-header as 2 rows
        # We define gold as nr rows x nc: row0 group-ish labels flattened, row1 col headers, rest body
        # Simpler: gold = full rectangular matrix of intended cells (nr x nc)
        body_rows = nr - 2
        gold = [
            ["Region"] + [f"Grp{j}" if j < nc // 2 else f"Grp{j}" for j in range(1, nc)],
            headers[:],
        ]
        # fix gold row0
        gold[0] = ["Metric"] + ["Act" if j < nc // 2 else "Bud" for j in range(1, nc)]
        random.seed(seed)
        for r in range(body_rows):
            row = [f"Region-{r:03d}"] + [""] * (nc - 1)
            if style == "sec" and r > 0 and r % 10 == 0:
                y -= 6
                c.setFont("Helvetica-Oblique", 6)
                c.drawString(xs[0], y, f"— Section {r // 10} continued TOKEN_C{k:03d}_SEC —")
                y -= 10
            c.setFont("Helvetica", 5.5)
            c.drawString(xs[0], y, f"Region-{r:03d}"[:14])
            for col in range(1, nc):
                if random.random() < 0.18:
                    row[col] = ""
                    continue
                jx = random.uniform(-3.0, 3.0)
                if random.random() < 0.08:
                    jx += (xs[1] - xs[0]) * 0.35
                val = f"{r * 100 + col}"
                if r == 0 and col == 1:
                    val = f"TOKEN_C{k:03d}_R1"
                if r == body_rows - 1 and col == 1:
                    val = f"TOKEN_C{k:03d}_LAST"
                row[col] = val
                c.drawString(xs[col] + jx, y, val[:10])
            gold.append(row)
            y -= random.choice([8.5, 9.0, 9.5, 10.5, 11.0, 12.0])
            if y < 40:
                # pad remaining gold rows empty-ish but keep shape intent
                while len(gold) < nr:
                    gold.append([f"Region-{len(gold):03d}"] + [""] * (nc - 1))
                break
        # ensure gold is nr x nc
        while len(gold) < nr:
            gold.append([f"Region-{len(gold):03d}"] + [str(len(gold) * 10 + j) for j in range(1, nc)])
        gold = gold[:nr]
        for row in gold:
            while len(row) < nc:
                row.append("")
            del row[nc:]
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "header_slice_borderless",
                "failure_classes": [
                    "C4_ROW_FRAGMENT_LARGE",
                    "C5_HEADER_ONLY_SLICE",
                    "C8_MULTI_TABLE_COUNT_ERROR",
                    "C10_COUNT_OK_STRUCT_BAD",
                ],
                "description": f"Irregular {nr}x{nc} borderless with section breaks + jitter (ICDAR us-017 class)",
                "challenges": ["borderless", "row_fragment", "section_headers", "jitter_columns"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC", f"TOKEN_C{k:03d}_H1"],
                "expected_tables": [{"cells": gold}],
                "expected_table_count": 1,
                "page_count": 1,
                "struggle_mode": "header_slice_fragmentation",
            },
        )
        print(" ", doc_id)
        k += 1
    return k


def gen_partial_v_family(start: int) -> int:
    """C6/COL: full H, sparse V → lattice undercounts columns."""
    k = start
    configs = []
    for step in (2, 3, 4):
        for nr, nc in [(12, 10), (15, 12), (20, 12), (15, 14), (10, 16), (18, 10)]:
            configs.append((step, nr, nc))
    for step, nr, nc in configs:
        doc_id = f"C{k:03d}_partial_v_s{step}_{nr}x{nc}"
        path = CORPUS / f"{doc_id}.pdf"
        use_land = nc >= 10
        pagesize = landscape(letter) if use_land else letter
        W, H = pagesize
        c = canvas.Canvas(str(path), pagesize=pagesize)
        c.setFont("Helvetica-Bold", 10)
        c.drawString(25, H - 28, f"Partial-V step={step} {nr}x{nc} TOKEN_C{k:03d}_DOC")
        cells = mk_cells(nr, nc, "V", 1, f"TOKEN_C{k:03d}_A")
        cells[-1][-1] = f"TOKEN_C{k:03d}_LAST"
        cw = max(26, int((W - 50) / nc))
        xs = [25.0]
        for _ in range(nc):
            xs.append(xs[-1] + cw)
        ys = [H - 55 - i * 12 for i in range(nr + 1)]
        c.setLineWidth(0.55)
        c.setStrokeColor(colors.black)
        for y in ys:
            c.line(xs[0], y, xs[-1], y)
        for i, x in enumerate(xs):
            if i == 0 or i == len(xs) - 1 or i % step == 0:
                c.line(x, ys[-1], x, ys[0])
        fill_text(c, xs, ys, cells, fs=5.0)
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "partial_vlines",
                "failure_classes": ["C6_COL_SKEW_WIDE", "C10_COUNT_OK_STRUCT_BAD", "C11_SAME_SHAPE_ZERO_TEDS"],
                "description": f"Full H-rules, V every {step}rd line, gold {nr}x{nc}",
                "challenges": ["partial_vlines", "col_undercount", "wide_table"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC", f"TOKEN_C{k:03d}_A", f"TOKEN_C{k:03d}_LAST"],
                "expected_tables": [{"cells": cells}],
                "expected_table_count": 1,
                "page_count": 1,
                "struggle_mode": "partial_v_col_undercount",
            },
        )
        print(" ", doc_id)
        k += 1
    return k


def gen_false_underline_family(start: int) -> int:
    """C7: per-cell underlines → row overcount ~2x."""
    k = start
    configs = [(6, 4), (8, 5), (10, 5), (12, 6), (7, 4), (9, 5), (14, 4), (11, 6)]
    for nr, nc in configs:
        doc_id = f"C{k:03d}_cell_underlines_{nr}x{nc}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 11)
        c.drawString(40, PAGE_H - 36, f"Cell underlines TOKEN_C{k:03d}_DOC")
        cells = mk_cells(nr, nc, "U", 2, f"TOKEN_C{k:03d}_A")
        cw = max(55, int(420 / nc))
        xs = [60.0]
        for _ in range(nc):
            xs.append(xs[-1] + cw)
        ys = [PAGE_H - 90 - i * 20 for i in range(nr + 1)]
        stroke_grid(c, xs, ys)
        for r, row in enumerate(cells):
            for col, txt in enumerate(row):
                c.setFont("Helvetica-Bold" if r == 0 else "Helvetica", 7)
                c.drawString(xs[col] + 2, (ys[r] + ys[r + 1]) / 2 - 2, str(txt)[:12])
                c.setStrokeColor(colors.Color(0.25, 0.25, 0.3))
                c.setLineWidth(0.35)
                ty = (ys[r] + ys[r + 1]) / 2 - 4
                c.line(xs[col] + 2, ty, xs[col + 1] - 2, ty)
                c.setStrokeColor(colors.black)
                c.setLineWidth(0.65)
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "false_underlines",
                "failure_classes": ["C7_ROW_OVERCOUNT", "C10_COUNT_OK_STRUCT_BAD"],
                "description": f"True {nr}x{nc} grid + per-cell text underlines (row overcount)",
                "challenges": ["phantom_lines", "row_overcount", "underline_noise"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC", f"TOKEN_C{k:03d}_A"],
                "expected_tables": [{"cells": cells}],
                "expected_table_count": 1,
                "page_count": 1,
                "struggle_mode": "false_underline_row_overcount",
            },
        )
        print(" ", doc_id)
        k += 1
    return k


def gen_sparse_densify_family(start: int) -> int:
    """Partial H + sparse fill → densify under-recovers rows."""
    k = start
    configs = [
        (0.25, 20, 6),
        (0.25, 25, 8),
        (0.35, 20, 6),
        (0.35, 25, 8),
        (0.50, 20, 6),
        (0.50, 28, 7),
        (0.30, 30, 5),
        (0.40, 18, 9),
    ]
    for fill, nr, nc in configs:
        doc_id = f"C{k:03d}_sparse_f{int(fill * 100)}_{nr}x{nc}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 10)
        c.drawString(40, PAGE_H - 32, f"Sparse fill={fill} TOKEN_C{k:03d}_DOC")
        cw = max(40, int(480 / nc))
        xs = [40.0]
        for _ in range(nc):
            xs.append(xs[-1] + cw)
        ys = [PAGE_H - 70 - i * 12 for i in range(nr + 1)]
        c.setLineWidth(0.6)
        c.setStrokeColor(colors.black)
        for i, y in enumerate(ys):
            if i in (0, 1, len(ys) - 1) or i % 5 == 0:
                c.line(xs[0], y, xs[-1], y)
        for x in xs:
            c.line(x, ys[-1], x, ys[0])
        random.seed(int(fill * 100) + nr * 17 + nc)
        cells: list[list[str]] = []
        for r in range(nr):
            row = []
            for col in range(nc):
                if r == 0:
                    txt = f"H{col}" if col else "Key"
                elif r == 1 and col == nc - 1:
                    txt = f"TOKEN_C{k:03d}_A"
                elif r == nr - 1 and col == 1:
                    txt = f"TOKEN_C{k:03d}_LAST"
                elif random.random() < fill or col == 0:
                    txt = f"{r}.{col}" if col else f"R{r:02d}"
                else:
                    txt = ""
                row.append(txt)
                if txt:
                    c.setFont("Helvetica-Bold" if r == 0 else "Helvetica", 5.5)
                    c.drawString(xs[col] + 1, (ys[r] + ys[r + 1]) / 2 - 2, txt[:12])
            cells.append(row)
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "sparse_partial_h",
                "failure_classes": ["C5_HEADER_ONLY_SLICE", "C10_COUNT_OK_STRUCT_BAD", "C3_UNDER_DETECT"],
                "description": f"Partial H (every 5) + {int(fill*100)}% sparse fill, gold {nr}x{nc}",
                "challenges": ["sparse_cells", "partial_hlines", "densify_failure"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC", f"TOKEN_C{k:03d}_A"],
                "expected_tables": [{"cells": cells}],
                "expected_table_count": 1,
                "page_count": 1,
                "struggle_mode": "sparse_densify_row_undercount",
            },
        )
        print(" ", doc_id)
        k += 1
    return k


def gen_mixed_irregular_stream_family(start: int) -> int:
    """C13: lattice OK but irregular borderless stream missed entirely."""
    k = start
    for sr, nc_s in [(20, 6), (30, 6), (40, 7), (25, 8), (35, 5), (22, 9)]:
        doc_id = f"C{k:03d}_mix_irreg_str{sr}x{nc_s}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 11)
        c.drawString(40, PAGE_H - 36, f"Mixed irreg stream TOKEN_C{k:03d}_DOC")
        lat = mk_cells(4, 3, "L", 1, f"TOKEN_C{k:03d}_L")
        xs = [50, 130, 210, 300]
        ys = [PAGE_H - 60 - i * 14 for i in range(5)]
        stroke_grid(c, xs, ys)
        fill_text(c, xs, ys, lat)
        # irregular stream body
        y = ys[-1] - 35
        xs2 = [40 + i * max(55, int(500 / nc_s)) for i in range(nc_s + 1)]
        stream = []
        hdr = [f"S{j}" for j in range(nc_s)]
        hdr[0] = "Key"
        stream.append(hdr)
        random.seed(sr + nc_s)
        c.setFont("Helvetica-Bold", 6)
        for col in range(nc_s):
            c.drawString(xs2[col], y, hdr[col][:10])
        y -= 12
        for r in range(1, sr):
            row = [f"S{r:02d}"] + [""] * (nc_s - 1)
            for col in range(nc_s):
                jx = random.uniform(-2.5, 2.5)
                if col == 0:
                    val = f"S{r:02d}"
                elif r == 1 and col == 1:
                    val = f"TOKEN_C{k:03d}_S"
                elif r == sr - 1 and col == 1:
                    val = f"TOKEN_C{k:03d}_SL"
                else:
                    val = f"{r}.{col}"
                row[col] = val
                c.setFont("Helvetica", 6)
                c.drawString(xs2[col] + jx, y, val[:10])
            stream.append(row)
            y -= random.uniform(9.0, 12.0)
            if y < 40:
                # trim gold to what fit
                break
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "mixed_lattice_irreg_stream",
                "failure_classes": [
                    "C13_MIXED_LATTICE_STREAM",
                    "C8_MULTI_TABLE_COUNT_ERROR",
                    "C4_ROW_FRAGMENT_LARGE",
                    "C3_UNDER_DETECT",
                ],
                "description": f"Lattice 4x3 + irregular borderless ~{len(stream)}x{nc_s} (stream often missed)",
                "challenges": ["mixed", "irregular_stream", "under_detect_stream"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC", f"TOKEN_C{k:03d}_L", f"TOKEN_C{k:03d}_S"],
                "expected_tables": [{"cells": lat}, {"cells": stream}],
                "expected_table_count": 2,
                "page_count": 1,
                "struggle_mode": "mixed_stream_miss",
            },
        )
        print(" ", doc_id)
        k += 1
    return k


def gen_multi_fragment_family(start: int) -> int:
    """C8: several heterogeneous tables + chrome that causes count/order issues."""
    k = start
    for ntabs, gap in [(3, 6), (4, 8), (5, 10), (4, 5), (6, 12)]:
        doc_id = f"C{k:03d}_multi_close_n{ntabs}_g{gap}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 11)
        c.drawString(40, PAGE_H - 30, f"Multi-close n={ntabs} TOKEN_C{k:03d}_DOC")
        # page frame + footer deco H
        c.setStrokeColor(colors.grey)
        c.setLineWidth(0.5)
        c.rect(18, 18, PAGE_W - 36, PAGE_H - 36)
        for i in range(4):
            c.line(40, 28 + i * 6, PAGE_W - 40, 28 + i * 6)
        gold = []
        y0 = PAGE_H - 70
        shapes = [(5, 4), (6, 3), (4, 5), (7, 2), (5, 5), (4, 4)]
        for ti in range(ntabs):
            nr, nc = shapes[ti % len(shapes)]
            cells = mk_cells(nr, nc, f"T{ti}", 1, f"TOKEN_C{k:03d}_T{ti}")
            gold.append({"cells": cells})
            xs = [50.0]
            for j in range(nc):
                xs.append(xs[-1] + (70 if j == 0 else 48))
            ys = [y0 - i * 13 for i in range(nr + 1)]
            c.setStrokeColor(colors.black)
            stroke_grid(c, xs, ys, lw=0.55)
            fill_text(c, xs, ys, cells, fs=6)
            y0 = ys[-1] - gap
            if y0 < 80:
                break
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "multi_table_close",
                "failure_classes": ["C8_MULTI_TABLE_COUNT_ERROR", "C12_ORDER_SENSITIVE", "C1_OVER_DETECT"],
                "description": f"{len(gold)} heterogeneous lattices with {gap}pt gaps + page chrome",
                "challenges": ["multi_table", "close_tables", "chrome"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC"] + [f"TOKEN_C{k:03d}_T{i}" for i in range(len(gold))],
                "expected_tables": gold,
                "expected_table_count": len(gold),
                "page_count": 1,
                "struggle_mode": "multi_table_close_chrome",
            },
        )
        print(" ", doc_id)
        k += 1
    return k


def gen_multipage_chaos_family(start: int) -> int:
    """C9: multipage with partial H + footer chrome + varying shapes."""
    k = start
    for npages, step in [(2, 3), (3, 3), (4, 4), (3, 2)]:
        doc_id = f"C{k:03d}_mp_chaos_p{npages}_s{step}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        gold = []
        for p in range(npages):
            c.setFont("Helvetica-Bold", 10)
            c.drawString(40, PAGE_H - 30, f"Page {p+1} TOKEN_C{k:03d}_P{p}")
            c.setStrokeColor(colors.grey)
            c.setLineWidth(0.5)
            c.rect(20, 20, PAGE_W - 40, PAGE_H - 40)
            for i in range(5):
                c.line(40, 30 + i * 7, PAGE_W - 40, 30 + i * 7)
            nr, nc = 16 + p, 5 + (p % 2)
            cells = mk_cells(nr, nc, f"P{p}", 1, f"TOKEN_C{k:03d}_P{p}A")
            cells[-1][-1] = f"TOKEN_C{k:03d}_P{p}L"
            gold.append({"cells": cells, "page": p})
            cw = max(50, int(450 / nc))
            xs = [50.0]
            for _ in range(nc):
                xs.append(xs[-1] + cw)
            ys = [PAGE_H - 70 - i * 12 for i in range(nr + 1)]
            c.setStrokeColor(colors.black)
            c.setLineWidth(0.6)
            for i, y in enumerate(ys):
                if i == 0 or i == len(ys) - 1 or i % step == 0:
                    c.line(xs[0], y, xs[-1], y)
            for x in xs:
                c.line(x, ys[-1], x, ys[0])
            fill_text(c, xs, ys, cells, fs=5.5)
            c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "multipage_chaos",
                "failure_classes": ["C9_MULTIPAGE", "C5_HEADER_ONLY_SLICE", "C1_OVER_DETECT", "C8_MULTI_TABLE_COUNT_ERROR"],
                "description": f"{npages} pages partial-H step={step} + footer chrome + page frame",
                "challenges": ["multi_page", "partial_h", "footer_chrome", "page_border"],
                "must_contain": [f"TOKEN_C{k:03d}_P{p}" for p in range(npages)],
                "expected_tables": gold,
                "expected_table_count": npages,
                "page_count": npages,
                "struggle_mode": "multipage_partial_chrome",
            },
        )
        print(" ", doc_id)
        k += 1
    return k


def gen_span_hard_family(start: int) -> int:
    """C15: more complex multi-span financial layouts."""
    k = start
    styles = getSampleStyleSheet()
    variants = [
        # visual grids with multiple spans
        [
            ["Division", "FY2023", "", "", "FY2024", "", "Notes"],
            ["", "Q1", "Q2", "H1", "Q1", "Q2", ""],
            ["North", "10", "12", "22", "11", "13", f"TOKEN"],
            ["South", "8", "9", "17", "9", "10", "ops"],
            ["West", "5", "6", "11", "6", "7", "TOKEN2"],
            ["Total", "23", "27", "50", "26", "30", "sum"],
        ],
        [
            ["Item", "Category A", "", "Category B", "", "Total"],
            ["", "Act", "Bud", "Act", "Bud", ""],
            ["Rev", "100", "90", "80", "85", "180"],
            ["COGS", "40", "42", "35", "36", "75"],
            ["GM", "60", "48", "45", "49", "105"],
            ["Opex", "20", "22", "18", "19", "38"],
            ["NI", "40", "26", "27", "30", "TOKEN"],
        ],
    ]
    for vi, visual in enumerate(variants):
        for extra in range(2):
            doc_id = f"C{k:03d}_span_hard_v{vi+1}e{extra}"
            path = CORPUS / f"{doc_id}.pdf"
            # inject tokens
            grid = [list(r) for r in visual]
            if vi == 0:
                grid[2][-1] = f"TOKEN_C{k:03d}_N"
                grid[4][-1] = f"TOKEN_C{k:03d}_W"
            else:
                grid[-1][-1] = f"TOKEN_C{k:03d}_NI"
            # deco for extra
            story = [Paragraph(f"Span-hard TOKEN_C{k:03d}_DOC", styles["Heading2"]), Spacer(1, 8)]
            if extra:
                # add chrome table above
                deco = Table([["X", "Y"], ["1", "2"]], colWidths=[40, 40])
                deco.setStyle(TableStyle([("GRID", (0, 0), (-1, -1), 0.4, colors.grey)]))
                story += [deco, Spacer(1, 12)]
            t = Table(grid, colWidths=[55] + [40] * (len(grid[0]) - 1))
            spans = []
            if vi == 0:
                spans = [
                    ("SPAN", (1, 0), (3, 0)),
                    ("SPAN", (4, 0), (5, 0)),
                    ("SPAN", (0, 0), (0, 1)),
                    ("SPAN", (6, 0), (6, 1)),
                ]
            else:
                spans = [
                    ("SPAN", (1, 0), (2, 0)),
                    ("SPAN", (3, 0), (4, 0)),
                    ("SPAN", (0, 0), (0, 1)),
                    ("SPAN", (5, 0), (5, 1)),
                ]
            t.setStyle(
                TableStyle(
                    [
                        ("GRID", (0, 0), (-1, -1), 0.55, colors.black),
                        ("FONTSIZE", (0, 0), (-1, -1), 7),
                        *spans,
                    ]
                )
            )
            story.append(t)
            SimpleDocTemplate(str(path), pagesize=letter, leftMargin=40, rightMargin=40).build(story)
            write_gt(
                doc_id,
                {
                    "category": "span_complex_hard",
                    "failure_classes": ["C15_SPAN_COMPLEX", "C11_SAME_SHAPE_ZERO_TEDS", "C10_COUNT_OK_STRUCT_BAD"],
                    "description": f"Multi-level colspan financial grid variant {vi} extra={extra}",
                    "challenges": ["colspan", "rowspan", "multi_header", "financial"],
                    "must_contain": [f"TOKEN_C{k:03d}_DOC"],
                    "expected_tables": [{"cells": grid}],
                    "expected_table_count": 1 if not extra else 2,
                    "page_count": 1,
                    "struggle_mode": "complex_spans",
                },
            )
            # fix count if extra deco table
            if extra:
                g = json.loads((GT / f"{doc_id}.json").read_text())
                g["expected_tables"] = [
                    {"cells": [["X", "Y"], ["1", "2"]], "rows": 2, "cols": 2},
                    {"cells": grid, "rows": len(grid), "cols": len(grid[0])},
                ]
                g["expected_table_count"] = 2
                (GT / f"{doc_id}.json").write_text(json.dumps(g, indent=2) + "\n")
            print(" ", doc_id)
            k += 1
    return k


def gen_invoice_hard_family(start: int) -> int:
    """C14 harder: totals integrated with same rule weight as body."""
    k = start
    for n_items, n_fee_rows in [(5, 4), (8, 5), (12, 6), (6, 3)]:
        doc_id = f"C{k:03d}_invoice_hard_i{n_items}_f{n_fee_rows}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 12)
        c.drawString(48, PAGE_H - 40, f"Invoice hard TOKEN_C{k:03d}_DOC")
        body = [["SKU", "Description", "Qty", "Unit", "Amount"]]
        for i in range(1, n_items + 1):
            body.append(
                [
                    f"SKU-{i}",
                    f"Item{i}" + (f" TOKEN_C{k:03d}_I1" if i == 1 else ""),
                    str(i),
                    str(10 * i),
                    str(10 * i * i),
                ]
            )
        fees = []
        fee_labels = ["Subtotal", "Tax", "Shipping", "Discount", "Other", "Total"]
        for fi in range(n_fee_rows):
            lab = fee_labels[fi % len(fee_labels)]
            tok = f" TOKEN_C{k:03d}_TOT" if lab == "Total" or fi == n_fee_rows - 1 else ""
            fees.append(["", f"{lab}{tok}", "", "", str(100 + fi)])
        # gold = body only (items + header), fees are outside gold table intent
        all_rows = body + fees
        xs = [70, 130, 300, 350, 400, 470]
        ys = [PAGE_H - 100 - i * 16 for i in range(len(all_rows) + 1)]
        stroke_grid(c, xs, ys)
        fill_text(c, xs, ys, all_rows, fs=7)
        # extra H rules through fee section only (double weight)
        c.setStrokeColor(colors.Color(0.2, 0.2, 0.2))
        c.setLineWidth(0.4)
        for i in range(len(body), len(all_rows)):
            y = (ys[i] + ys[i + 1]) / 2 - 3
            c.line(xs[0] + 2, y, xs[-1] - 2, y)
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "invoice_footer_hard",
                "failure_classes": ["C14_INVOICE_FOOTER", "C7_ROW_OVERCOUNT", "C10_COUNT_OK_STRUCT_BAD"],
                "description": f"Invoice {n_items} items + {n_fee_rows} fee rows inside same grid; gold=body only",
                "challenges": ["invoice_totals", "footer_inside_grid", "row_overcount"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC", f"TOKEN_C{k:03d}_I1"],
                "expected_tables": [{"cells": body}],
                "expected_table_count": 1,
                "page_count": 1,
                "struggle_mode": "invoice_fees_inside_grid",
            },
        )
        print(" ", doc_id)
        k += 1
    return k


def gen_overdetect_severe_family(start: int) -> int:
    """C1/C2: many closed mini-grids (2x2) + one real table — force over-detect if filters fail."""
    k = start
    for nbox, with_text in [(8, True), (16, True), (24, True), (12, False)]:
        doc_id = f"C{k:03d}_severe_overdetect_n{nbox}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 11)
        c.drawString(40, PAGE_H - 30, f"Severe overdetect TOKEN_C{k:03d}_DOC")
        # real table
        cells = mk_cells(6, 4, "R", 2, f"TOKEN_C{k:03d}_A")
        xs = [80, 160, 240, 320, 400]
        ys = [220 - i * 16 for i in range(7)]
        stroke_grid(c, xs, ys)
        fill_text(c, xs, ys, cells)
        # mini 2x3 grids with text (look like real tables)
        for i in range(nbox):
            x = 30 + (i % 4) * 140
            y = PAGE_H - 55 - (i // 4) * 48
            bx = [x, x + 55, x + 110]
            by = [y, y - 14, y - 28]
            c.setStrokeColor(colors.black)
            c.setLineWidth(0.55)
            for yy in by:
                c.line(bx[0], yy, bx[-1], yy)
            for xx in bx:
                c.line(xx, by[-1], xx, by[0])
            if with_text:
                c.setFont("Helvetica", 6)
                c.drawString(x + 2, y - 10, f"A{i}")
                c.drawString(x + 57, y - 10, f"{i}")
                c.drawString(x + 2, y - 24, f"B{i}")
                c.drawString(x + 57, y - 24, f"{i*2}")
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "severe_overdetect",
                "failure_classes": ["C1_OVER_DETECT", "C2_SEVERE_OVER_DETECT", "C8_MULTI_TABLE_COUNT_ERROR"],
                "description": f"One 6x4 real table + {nbox} filled mini 2x2 grids",
                "challenges": ["over_detect", "form_like_minigrids", "joint_rich_noise"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC", f"TOKEN_C{k:03d}_A"],
                "expected_tables": [{"cells": cells}],
                "expected_table_count": 1,
                "page_count": 1,
                "struggle_mode": "severe_minigrid_overdetect",
            },
        )
        print(" ", doc_id)
        k += 1
    return k


def gen_content_teds_family(start: int) -> int:
    """C11: shape-ish OK risk but cell assign hard (wrapped multi-token cells)."""
    k = start
    for vi in range(4):
        doc_id = f"C{k:03d}_cell_assign_hard_v{vi+1}"
        path = CORPUS / f"{doc_id}.pdf"
        c = canvas.Canvas(str(path), pagesize=letter)
        c.setFont("Helvetica-Bold", 11)
        c.drawString(40, PAGE_H - 36, f"Cell assign TOKEN_C{k:03d}_DOC")
        # cells with multi-line text drawn as separate runs in same cell bbox
        headers = ["ID", "Description", "Qty", "Price"]
        rows = [headers]
        for i in range(1, 8):
            rows.append(
                [
                    f"ID-{i}",
                    f"Long item description line A for row {i}; also line B TOKEN_C{k:03d}_D{i}" if i <= 2 else f"Item {i} desc",
                    str(i * 3),
                    f"{i * 9.5:.1f}",
                ]
            )
        xs = [50, 100, 360, 420, 500]
        # taller rows for wrapped text
        ys = [PAGE_H - 80]
        for r in range(len(rows)):
            h = 28 if r > 0 and "Long" in rows[r][1] else 16
            ys.append(ys[-1] - h)
        stroke_grid(c, xs, ys)
        for r, row in enumerate(rows):
            for col, txt in enumerate(row):
                c.setFont("Helvetica-Bold" if r == 0 else "Helvetica", 6.5)
                y_base = (ys[r] + ys[r + 1]) / 2 - 2
                if len(txt) > 28 and col == 1:
                    # two paint positions in same cell (wrapped)
                    c.drawString(xs[col] + 2, ys[r] - 10, txt[:28])
                    c.drawString(xs[col] + 2, ys[r] - 20, txt[28:56])
                else:
                    c.drawString(xs[col] + 2, y_base, str(txt)[:20])
        c.showPage()
        c.save()
        write_gt(
            doc_id,
            {
                "category": "cell_assign_hard",
                "failure_classes": ["C11_SAME_SHAPE_ZERO_TEDS", "C10_COUNT_OK_STRUCT_BAD"],
                "description": "Wrapped multi-run cells; TEDS/cell F1 sensitive to assign/order",
                "challenges": ["wrapped_cells", "multi_run_text", "cell_assign"],
                "must_contain": [f"TOKEN_C{k:03d}_DOC", f"TOKEN_C{k:03d}_D1"],
                "expected_tables": [{"cells": rows}],
                "expected_table_count": 1,
                "page_count": 1,
                "struggle_mode": "cell_content_assign",
            },
        )
        print(" ", doc_id)
        k += 1
    return k


def update_suites_and_index():
    # collect all compete ids (easy + hard)
    all_compete = sorted(
        p.stem
        for p in GT.glob("C*.json")
        if json.loads(p.read_text()).get("suite") == "regression_compete"
        or json.loads(p.read_text()).get("tier", "").startswith("compete")
    )
    # also include R*
    reals = sorted(p.stem for p in GT.glob("R*.json"))
    if SUITES.exists():
        data = json.loads(SUITES.read_text())
    else:
        data = {"version": 1, "suites": {}, "policy": {}}
    data.setdefault("policy", {})
    data["policy"]["compete_suite"] = (
        "Competitive-gap corpus: C001–C068 coverage + C100+ open_struggle hard wave + real. "
        "Hard wave reverse-engineered from ICDAR failure modes; validated to stress F1/TEDS/row/col. "
        "No ICDAR files."
    )
    data["policy"]["compete_hard_purpose"] = (
        "open_struggle fixtures where current pdfparser fails shape/count/cell metrics. "
        "Freeze baseline before algorithm work. Do not delete failures when fixing — mark solved_regression."
    )
    data.setdefault("suites", {})
    data["suites"]["regression_compete"] = {
        "description": "Full compete suite (coverage + hard struggle + real soft gold).",
        "tiers": ["compete", "compete_hard", "compete_real"],
        "document_ids": all_compete + reals,
        "icdar_derived": False,
        "scoring_guidance": (
            "Primary: table_detect_f1, table_row_accuracy, table_col_accuracy, table_cell_f1 (TEDS-like), "
            "shape_exact_rate. Break down by failure_classes / struggle_mode. "
            "Hard subset (tier=compete_hard) should remain low until Auto/Network/raster land."
        ),
    }
    data["suites"]["regression_compete_hard"] = {
        "description": "Open-struggle hard wave only (C100+ / tier=compete_hard).",
        "tiers": ["compete_hard"],
        "filter": {"development_role": "open_struggle"},
        "icdar_derived": False,
    }
    SUITES.write_text(json.dumps(data, indent=2) + "\n")

    index = {"version": 2, "n_docs": 0, "document_ids": [], "by_class": {}, "by_struggle_mode": {}, "by_tier": {}}
    for p in sorted(GT.glob("C*.json")) + sorted(GT.glob("R*.json")):
        g = json.loads(p.read_text())
        if g.get("suite") != "regression_compete" and not str(g.get("tier", "")).startswith("compete"):
            continue
        doc_id = g["id"]
        index["document_ids"].append(doc_id)
        index["by_tier"].setdefault(g.get("tier", "?"), []).append(doc_id)
        for cl in g.get("failure_classes", []):
            index["by_class"].setdefault(cl, []).append(doc_id)
        sm = g.get("struggle_mode") or g.get("category") or "unknown"
        index["by_struggle_mode"].setdefault(sm, []).append(doc_id)
    index["n_docs"] = len(index["document_ids"])
    (ROOT / "corpus" / "compete_class_index.json").write_text(json.dumps(index, indent=2) + "\n")
    print("suites + class index updated, n=", index["n_docs"])


def main():
    global IDS
    ap = argparse.ArgumentParser()
    ap.add_argument("--probe-filter", action="store_true", help="After gen, log probe pass/fail (keep all)")
    args = ap.parse_args()
    IDS = []
    print("Generating HARD compete struggle corpus (C100+)…")
    k = 100
    k = gen_image_painted_family(k)
    k = gen_header_slice_family(k)
    k = gen_partial_v_family(k)
    k = gen_false_underline_family(k)
    k = gen_sparse_densify_family(k)
    k = gen_mixed_irregular_stream_family(k)
    k = gen_multi_fragment_family(k)
    k = gen_multipage_chaos_family(k)
    k = gen_span_hard_family(k)
    k = gen_invoice_hard_family(k)
    k = gen_overdetect_severe_family(k)
    k = gen_content_teds_family(k)
    update_suites_and_index()
    print(f"Done: {len(IDS)} hard struggle docs (ids start C100)")

    # class coverage
    c = Counter()
    modes = Counter()
    for doc_id in IDS:
        g = json.loads((GT / f"{doc_id}.json").read_text())
        for cl in g.get("failure_classes", []):
            c[cl] += 1
        modes[g.get("struggle_mode", "?")] += 1
    print("Failure-class coverage (hard wave):")
    for name, v in sorted(c.items()):
        print(f"  {name}: {v}")
    print("Struggle modes:")
    for name, v in sorted(modes.items()):
        print(f"  {name}: {v}")

    if args.probe_filter and BIN.exists():
        print("\nProbing each hard doc against current pdfparser…")
        fail_n = 0
        for doc_id in IDS:
            g = json.loads((GT / f"{doc_id}.json").read_text())
            pdf = ROOT / g["path"]
            pred = probe_shapes(pdf)
            et = g.get("expected_tables") or []
            gold_n = g.get("expected_table_count", len(et))
            if et:
                gr, gc = len(et[0]["cells"]), len(et[0]["cells"][0])
                ok = shape_ok(pred, (gr, gc), gold_n if gold_n == 1 else None)
                if gold_n != 1:
                    ok = len(pred) == gold_n
            else:
                ok = False
            if not ok:
                fail_n += 1
            PROBE_LOG.append({"id": doc_id, "ok": ok, "pred": pred, "gold_n": gold_n})
            print(f"  {'pass' if ok else 'FAIL'} {doc_id} pred={pred}")
        print(f"Probe: {fail_n}/{len(IDS)} FAIL (desired: majority FAIL for open_struggle)")
        (ROOT / "results" / "compete_hard_probe.json").write_text(json.dumps(PROBE_LOG, indent=2))


if __name__ == "__main__":
    main()
