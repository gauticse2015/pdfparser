#!/usr/bin/env python3
"""Generate synthetic HARD regression fixtures (ICDAR-class difficulty, not ICDAR data).

Policy
------
* This suite is for **regression / improvement** of pdfparser.
* Documents are **100% synthetic** (reportlab). No ICDAR PDFs, no ICDAR XML,
  no copies of competition files.
* Difficulty modes are *inspired by* failure classes observed on external
  competitive sets (multi-table pages, spans, decorative line noise, multipage
  continuity) but content, layout, and tokens are original.

Outputs
-------
  corpus/hard/50_…62_….pdf
  ground_truth/50_….json   (full grid gold where applicable)
  corpus/suites.json       (suite membership policy)
  corpus/manifest.json     (rebuilt to include hard tier)

Run
---
  source .venv/bin/activate
  python benchmark/scripts/generate_hard_corpus.py
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Optional

from reportlab.lib import colors
from reportlab.lib.enums import TA_CENTER, TA_LEFT
from reportlab.lib.pagesizes import letter
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.lib.units import inch
from reportlab.platypus import (
    Paragraph,
    SimpleDocTemplate,
    Spacer,
    Table,
    TableStyle,
    PageBreak,
    KeepTogether,
)
from reportlab.pdfgen import canvas

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "corpus"
HARD = CORPUS / "hard"
GT = ROOT / "ground_truth"
for d in (CORPUS, HARD, GT):
    d.mkdir(parents=True, exist_ok=True)

PAGE_W, PAGE_H = letter

styles = getSampleStyleSheet()
styles.add(
    ParagraphStyle(name="HTitle", fontName="Helvetica-Bold", fontSize=12, leading=14)
)
styles.add(
    ParagraphStyle(name="HMeta", fontName="Helvetica", fontSize=8, leading=10)
)
styles.add(
    ParagraphStyle(name="HCell", fontName="Helvetica", fontSize=8, leading=10)
)
styles.add(
    ParagraphStyle(
        name="HHdr",
        fontName="Helvetica-Bold",
        fontSize=8,
        leading=10,
        alignment=TA_CENTER,
    )
)
styles.add(
    ParagraphStyle(name="HTiny", fontName="Helvetica", fontSize=6.5, leading=8)
)


def write_gt(doc_id: str, data: dict) -> None:
    payload = {"id": doc_id, **data}
    (GT / f"{doc_id}.json").write_text(
        json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )


def p(text: str, style: str = "HCell") -> Paragraph:
    return Paragraph(str(text).replace("\n", "<br/>"), styles[style])


def grid_style(extra: Optional[list] = None) -> TableStyle:
    cmds = [
        ("GRID", (0, 0), (-1, -1), 0.6, colors.black),
        ("BACKGROUND", (0, 0), (-1, 0), colors.Color(0.88, 0.88, 0.9)),
        ("FONTNAME", (0, 0), (-1, 0), "Helvetica-Bold"),
        ("FONTSIZE", (0, 0), (-1, -1), 8),
        ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
        ("LEFTPADDING", (0, 0), (-1, -1), 3),
        ("RIGHTPADDING", (0, 0), (-1, -1), 3),
        ("TOPPADDING", (0, 0), (-1, -1), 3),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 3),
    ]
    if extra:
        cmds.extend(extra)
    return TableStyle(cmds)


def table_from_cells(
    cells: list[list[str]],
    col_widths: Optional[list[float]] = None,
    extra_style: Optional[list] = None,
    header: bool = True,
) -> Table:
    data = []
    for ri, row in enumerate(cells):
        style = "HHdr" if header and ri == 0 else "HCell"
        data.append([p(c, style) for c in row])
    t = Table(data, colWidths=col_widths)
    t.setStyle(grid_style(extra_style))
    return t


def base_gt(
    doc_id: str,
    *,
    category: str,
    description: str,
    challenges: list[str],
    must_contain: list[str],
    expected_tables: list[dict],
    page_count: int = 1,
    table_cells_must_include: Optional[list[str]] = None,
    notes: str = "",
    failure_modes_covered: Optional[list[str]] = None,
) -> dict:
    """Full grid-gold GT record for the hard regression suite."""
    return {
        "category": category,
        "tier": "hard",
        "suite": "regression_hard",
        "source": "synthetic",
        "icdar_derived": False,
        "description": description,
        "challenges": challenges,
        "failure_modes_covered": failure_modes_covered or [],
        "path": f"corpus/hard/{doc_id}.pdf",
        "page_count": page_count,
        "must_contain": must_contain,
        "table_cells_must_include": table_cells_must_include
        or [c for t in expected_tables for row in t.get("cells", []) for c in row if c][
            :12
        ],
        "expected_table_count": len(expected_tables),
        "expected_tables": [
            {
                "rows": len(t["cells"]),
                "cols": len(t["cells"][0]) if t["cells"] else 0,
                "cells": t["cells"],
                **{k: v for k, v in t.items() if k != "cells"},
            }
            for t in expected_tables
        ],
        "expected_images": 0,
        "weight_text": 0.15,
        "weight_tables": 0.85,
        "weight_objects": 0.0,
        "our_library_priority": "hard regression / structure quality",
        "quality_notes": notes,
    }


# ═══════════════════════════════════════════════════════════════════════════
# Drawing helpers (canvas) for precise multi-region / noise cases
# ═══════════════════════════════════════════════════════════════════════════


def draw_ruled_grid(
    c: canvas.Canvas,
    x0: float,
    y_bottom: float,
    col_widths: list[float],
    row_heights: list[float],
    cells: list[list[str]],
    *,
    line_width: float = 0.7,
    font_size: float = 8,
    gap_corners: float = 0.0,
) -> None:
    """Draw an axis-aligned ruled table.

    Coordinate system: PDF y-up. ``y_bottom`` is the bottom of the table.
    If ``gap_corners`` > 0, each segment stops short of intersections
    (broken-corner stress for lattice joiners).
    """
    ncols = len(col_widths)
    nrows = len(row_heights)
    total_w = sum(col_widths)
    total_h = sum(row_heights)
    y_top = y_bottom + total_h

    # Vertical line x positions
    xs = [x0]
    for w in col_widths:
        xs.append(xs[-1] + w)
    # Horizontal line y positions from top to bottom
    ys = [y_top]
    for h in row_heights:
        ys.append(ys[-1] - h)

    c.setStrokeColor(colors.black)
    c.setLineWidth(line_width)
    g = gap_corners

    # Horizontal segments
    for y in ys:
        if g <= 0:
            c.line(xs[0], y, xs[-1], y)
        else:
            for i in range(ncols):
                c.line(xs[i] + g, y, xs[i + 1] - g, y)

    # Vertical segments
    for x in xs:
        if g <= 0:
            c.line(x, y_bottom, x, y_top)
        else:
            for j in range(nrows):
                # ys is top-to-bottom; segment between ys[j] and ys[j+1]
                c.line(x, ys[j + 1] + g, x, ys[j] - g)

    # Text (centers)
    c.setFillColor(colors.black)
    c.setFont("Helvetica", font_size)
    for r in range(nrows):
        for col in range(ncols):
            text = cells[r][col] if r < len(cells) and col < len(cells[r]) else ""
            if not text:
                continue
            cx = (xs[col] + xs[col + 1]) / 2
            cy = (ys[r] + ys[r + 1]) / 2 - font_size * 0.3
            c.drawCentredString(cx, cy, text[:48])


def draw_page_frame(c: canvas.Canvas, margin: float = 28.0, lw: float = 1.2) -> None:
    """Decorative full-page rectangular border (noise for global line snap)."""
    c.setStrokeColor(colors.Color(0.35, 0.35, 0.4))
    c.setLineWidth(lw)
    c.rect(margin, margin, PAGE_W - 2 * margin, PAGE_H - 2 * margin)


def draw_deco_hlines(
    c: canvas.Canvas, ys: list[float], x0: float, x1: float, lw: float = 0.4
) -> None:
    c.setStrokeColor(colors.Color(0.55, 0.55, 0.6))
    c.setLineWidth(lw)
    for y in ys:
        c.line(x0, y, x1, y)


# ═══════════════════════════════════════════════════════════════════════════
# 50 — Three stacked lattice tables on one page
# ═══════════════════════════════════════════════════════════════════════════


def gen_50_multi_table_stacked() -> None:
    doc_id = "50_multi_table_stacked_page"
    path = HARD / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setTitle("Hard: multi-table stacked page")
    c.setAuthor("pdfparser-benchmark-hard")

    c.setFont("Helvetica-Bold", 13)
    c.drawString(50, PAGE_H - 40, "Regional Operations Report")
    c.setFont("Helvetica", 8)
    c.drawString(
        50,
        PAGE_H - 54,
        "TOKEN_HARD50_DOC · Three independent ruled tables stacked on one page "
        "(must not fuse into one mega-grid).",
    )

    t1 = [
        ["Region", "Q1", "Q2", "Q3", "Q4"],
        ["North TOKEN_H50_T1", "10", "12", "11", "15"],
        ["South", "8", "9", "10", "12"],
        ["East", "14", "13", "16", "18"],
        ["West", "7", "8", "9", "11"],
    ]
    t2 = [
        ["Dept", "Headcount", "Budget"],
        ["Eng TOKEN_H50_T2", "42", "1.2M"],
        ["Sales", "18", "0.9M"],
        ["Ops", "25", "0.6M"],
    ]
    t3 = [
        ["SKU", "OnHand", "Reorder"],
        ["A-10 TOKEN_H50_T3", "120", "50"],
        ["B-20", "40", "30"],
        ["C-30", "200", "80"],
        ["D-40", "15", "20"],
        ["E-50", "90", "40"],
    ]

    # Top table
    draw_ruled_grid(
        c, 50, PAGE_H - 220, [90, 50, 50, 50, 50], [18] * 5, t1
    )
    c.setFont("Helvetica-Oblique", 8)
    c.drawString(50, PAGE_H - 238, "Table A — quarterly by region")

    # Middle table (different column structure)
    draw_ruled_grid(
        c, 50, PAGE_H - 380, [100, 80, 80], [18] * 4, t2
    )
    c.setFont("Helvetica-Oblique", 8)
    c.drawString(50, PAGE_H - 398, "Table B — department staffing")

    # Bottom table
    draw_ruled_grid(
        c, 50, PAGE_H - 560, [90, 70, 70], [18] * 6, t3
    )
    c.setFont("Helvetica-Oblique", 8)
    c.drawString(50, PAGE_H - 578, "Table C — inventory")

    c.showPage()
    c.save()

    write_gt(
        doc_id,
        base_gt(
            doc_id,
            category="multi_table_stacked",
            description="Three vertically stacked lattice tables with different shapes on one page",
            challenges=[
                "multiple_tables_per_page",
                "vertical_stack",
                "heterogeneous_shapes",
                "lattice",
                "anti_mega_grid",
            ],
            failure_modes_covered=[
                "MULTI_TABLE_PAGE",
                "UNDER_DETECT",
                "WRONG_SHAPE",
                "ROW_MISCOUNT",
                "COL_MISCOUNT",
            ],
            must_contain=[
                "TOKEN_HARD50_DOC",
                "TOKEN_H50_T1",
                "TOKEN_H50_T2",
                "TOKEN_H50_T3",
            ],
            expected_tables=[
                {"cells": t1, "label": "A"},
                {"cells": t2, "label": "B"},
                {"cells": t3, "label": "C"},
            ],
            notes="Success: detect 3 tables with exact shapes 5x5, 4x3, 6x3. Fail: one fused mega-grid.",
        ),
    )
    print(f"  {doc_id}")


# ═══════════════════════════════════════════════════════════════════════════
# 51 — Multi-table multi-page
# ═══════════════════════════════════════════════════════════════════════════


def gen_51_multi_table_multipage() -> None:
    doc_id = "51_multi_table_multipage"
    path = HARD / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setTitle("Hard: multi-table multipage")

    pages = [
        {
            "title": "EU Ops Summary — Page 1 TOKEN_HARD51_P1",
            "tables": [
                [
                    ["Country", "Sites", "Staff"],
                    ["DE TOKEN_H51_A", "12", "340"],
                    ["FR", "8", "210"],
                    ["IT", "6", "180"],
                ],
                [
                    ["Metric", "2024", "2025"],
                    ["Revenue TOKEN_H51_B", "4.2", "4.8"],
                    ["EBITDA", "0.9", "1.1"],
                    ["Margin%", "21", "23"],
                    ["NPS", "42", "48"],
                ],
            ],
        },
        {
            "title": "EU Ops Summary — Page 2 TOKEN_HARD51_P2",
            "tables": [
                [
                    ["Vendor", "SLA", "Score"],
                    ["Alpha TOKEN_H51_C", "99.9", "A"],
                    ["Beta", "99.5", "B"],
                ],
                [
                    ["City", "Lat", "Lon", "Pop"],
                    ["Berlin TOKEN_H51_D", "52.5", "13.4", "3.6M"],
                    ["Paris", "48.9", "2.3", "2.1M"],
                    ["Rome", "41.9", "12.5", "2.8M"],
                    ["Madrid", "40.4", "-3.7", "3.2M"],
                ],
            ],
        },
    ]

    all_tables: list[dict] = []
    for pi, page in enumerate(pages):
        c.setFont("Helvetica-Bold", 12)
        c.drawString(50, PAGE_H - 40, page["title"])
        c.setFont("Helvetica", 8)
        c.drawString(
            50,
            PAGE_H - 56,
            "TOKEN_HARD51_DOC · Two independent tables per page; four tables total.",
        )
        y = PAGE_H - 200
        for ti, cells in enumerate(page["tables"]):
            nrows = len(cells)
            ncols = len(cells[0])
            col_w = [70] * ncols if ncols <= 3 else [80, 50, 50, 60]
            row_h = [18] * nrows
            draw_ruled_grid(c, 50, y - sum(row_h), col_w[:ncols], row_h, cells)
            all_tables.append({"cells": cells, "page": pi + 1})
            y -= sum(row_h) + 50
        c.showPage()
    c.save()

    write_gt(
        doc_id,
        base_gt(
            doc_id,
            category="multi_table_multipage",
            description="Two pages, two lattice tables each (4 tables total), heterogeneous shapes",
            challenges=[
                "multiple_tables_per_page",
                "multi_page_document",
                "heterogeneous_shapes",
                "lattice",
            ],
            failure_modes_covered=[
                "MULTI_TABLE_PAGE",
                "MULTI_PAGE_DOC",
                "UNDER_DETECT",
                "WRONG_SHAPE",
            ],
            must_contain=[
                "TOKEN_HARD51_DOC",
                "TOKEN_HARD51_P1",
                "TOKEN_HARD51_P2",
                "TOKEN_H51_A",
                "TOKEN_H51_B",
                "TOKEN_H51_C",
                "TOKEN_H51_D",
            ],
            expected_tables=all_tables,
            page_count=2,
            notes="Detect 4 tables; do not merge across vertical gaps or pages.",
        ),
    )
    print(f"  {doc_id}")


# ═══════════════════════════════════════════════════════════════════════════
# 52 — Page border noise + real table
# ═══════════════════════════════════════════════════════════════════════════


def gen_52_page_border_noise() -> None:
    doc_id = "52_page_border_noise"
    path = HARD / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setTitle("Hard: page border noise")

    draw_page_frame(c, margin=24, lw=1.5)
    # Extra inner decorative lines (header rule + footer rule spanning full width)
    c.setStrokeColor(colors.Color(0.4, 0.4, 0.45))
    c.setLineWidth(0.8)
    c.line(36, PAGE_H - 70, PAGE_W - 36, PAGE_H - 70)
    c.line(36, 50, PAGE_W - 36, 50)
    # Vertical decorative margin rules
    c.line(40, 60, 40, PAGE_H - 80)
    c.line(PAGE_W - 40, 60, PAGE_W - 40, PAGE_H - 80)

    c.setFont("Helvetica-Bold", 13)
    c.drawString(60, PAGE_H - 50, "Confidential Brief TOKEN_HARD52_DOC")
    c.setFont("Helvetica", 8)
    c.drawString(
        60,
        PAGE_H - 90,
        "Decorative page border + margin rules must NOT be interpreted as a table grid.",
    )

    cells = [
        ["Item", "Owner", "Status", "Due"],
        ["Design TOKEN_H52_ROW", "Ada", "Done", "Jan"],
        ["Build", "Lin", "WIP", "Feb"],
        ["Test", "Sam", "Todo", "Mar"],
        ["Ship", "Pat", "Todo", "Apr"],
    ]
    draw_ruled_grid(c, 80, PAGE_H - 320, [100, 70, 70, 60], [20] * 5, cells)

    c.setFont("Helvetica", 8)
    c.drawString(80, PAGE_H - 340, "Only ONE data table expected (4 cols × 5 rows).")
    c.showPage()
    c.save()

    write_gt(
        doc_id,
        base_gt(
            doc_id,
            category="page_border_noise",
            description="Full-page decorative frame + margin rules surrounding one real lattice table",
            challenges=[
                "decorative_lines",
                "page_border",
                "noise_lines",
                "lattice",
                "false_positive_grid",
            ],
            failure_modes_covered=["OVER_DETECT", "WRONG_SHAPE", "MEGA_GRID", "BAD_STRUCTURE"],
            must_contain=["TOKEN_HARD52_DOC", "TOKEN_H52_ROW", "Confidential Brief"],
            expected_tables=[{"cells": cells}],
            notes="expected_table_count=1. Page border must not create a second table or inflate rows/cols.",
        ),
    )
    print(f"  {doc_id}")


# ═══════════════════════════════════════════════════════════════════════════
# 53 — Column-span header (true SPAN)
# ═══════════════════════════════════════════════════════════════════════════


def gen_53_column_span_header() -> None:
    doc_id = "53_column_span_header"
    path = HARD / f"{doc_id}.pdf"

    # Logical cell grid after span resolution (what extractors should emit).
    # Visual: row0 has Metric | FY2024 (span 2) | FY2025 (span 2) | Notes
    # row1 has blank | Actual | Budget | Actual | Budget | blank
    logical = [
        ["Metric", "FY2024 Actual", "FY2024 Budget", "FY2025 Actual", "FY2025 Budget", "Notes"],
        ["Revenue", "1200", "1100", "1450", "1400", "TOKEN_H53_REV"],
        ["COGS", "480", "500", "560", "550", "supply"],
        ["Gross Profit", "720", "600", "890", "850", "TOKEN_H53_GP"],
        ["OpEx", "310", "300", "340", "330", "ops"],
        ["Net Income", "410", "300", "550", "520", "TOKEN_H53_NI"],
    ]

    # Visual layout with SPAN (reportlab) — harder for lattice without span support
    visual = [
        [
            p("Metric", "HHdr"),
            p("FY2024 TOKEN_H53_FY24", "HHdr"),
            p("", "HHdr"),
            p("FY2025 TOKEN_H53_FY25", "HHdr"),
            p("", "HHdr"),
            p("Notes", "HHdr"),
        ],
        [
            p(""),
            p("Actual", "HHdr"),
            p("Budget", "HHdr"),
            p("Actual", "HHdr"),
            p("Budget", "HHdr"),
            p(""),
        ],
        [p("Revenue"), p("1200"), p("1100"), p("1450"), p("1400"), p("TOKEN_H53_REV")],
        [p("COGS"), p("480"), p("500"), p("560"), p("550"), p("supply")],
        [p("Gross Profit"), p("720"), p("600"), p("890"), p("850"), p("TOKEN_H53_GP")],
        [p("OpEx"), p("310"), p("300"), p("340"), p("330"), p("ops")],
        [p("Net Income"), p("410"), p("300"), p("550"), p("520"), p("TOKEN_H53_NI")],
    ]
    t = Table(visual, colWidths=[80, 55, 55, 55, 55, 90])
    t.setStyle(
        TableStyle(
            [
                ("GRID", (0, 0), (-1, -1), 0.6, colors.black),
                ("BACKGROUND", (0, 0), (-1, 1), colors.Color(0.86, 0.88, 0.92)),
                ("SPAN", (1, 0), (2, 0)),  # FY2024
                ("SPAN", (3, 0), (4, 0)),  # FY2025
                ("SPAN", (0, 0), (0, 1)),  # Metric rowspan-ish visual
                ("SPAN", (5, 0), (5, 1)),  # Notes
                ("ALIGN", (1, 0), (4, 1), "CENTER"),
                ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
                ("FONTSIZE", (0, 0), (-1, -1), 8),
            ]
        )
    )

    story = [
        Paragraph("Merged Column Headers — Hard", styles["HTitle"]),
        Paragraph(
            "TOKEN_HARD53_DOC · Group headers span two columns. Extractors must recover "
            "a coherent 6-column body grid (or equivalent span-aware structure).",
            styles["HMeta"],
        ),
        Spacer(1, 12),
        t,
    ]
    SimpleDocTemplate(
        str(path), pagesize=letter, leftMargin=48, rightMargin=48, topMargin=48
    ).build(story)

    # Gold: prefer flattened body-aware grid (common competitive target).
    # Also accept 7-row visual including subheader as alternate via notes.
    write_gt(
        doc_id,
        {
            **base_gt(
                doc_id,
                category="column_span_header",
                description="Financial table with colspan group headers (FY2024/FY2025 span 2 cols)",
                challenges=[
                    "colspan",
                    "merged_headers",
                    "multi_level_header",
                    "lattice",
                    "span_text_assign",
                ],
                failure_modes_covered=["WRONG_SHAPE", "COL_MISCOUNT", "BAD_STRUCTURE", "ROW_MISCOUNT"],
                must_contain=[
                    "TOKEN_HARD53_DOC",
                    "TOKEN_H53_FY24",
                    "TOKEN_H53_FY25",
                    "TOKEN_H53_REV",
                    "TOKEN_H53_GP",
                    "TOKEN_H53_NI",
                ],
                # Use visual row-expanded grid (7x6) as primary gold — matches drawn cells
                # after reportlab materializes spans into the cell matrix.
                expected_tables=[
                    {
                        "cells": [
                            ["Metric", "FY2024 TOKEN_H53_FY24", "", "FY2025 TOKEN_H53_FY25", "", "Notes"],
                            ["", "Actual", "Budget", "Actual", "Budget", ""],
                            ["Revenue", "1200", "1100", "1450", "1400", "TOKEN_H53_REV"],
                            ["COGS", "480", "500", "560", "550", "supply"],
                            ["Gross Profit", "720", "600", "890", "850", "TOKEN_H53_GP"],
                            ["OpEx", "310", "300", "340", "330", "ops"],
                            ["Net Income", "410", "300", "550", "520", "TOKEN_H53_NI"],
                        ],
                        "span_notes": "cols 1-2 header span FY2024; cols 3-4 header span FY2025",
                    }
                ],
                notes=(
                    "Primary gold is 7×6 visual grid. Span-aware extractors may emit "
                    "colspan metadata; cell tokens TOKEN_H53_* must land in correct columns."
                ),
            ),
            "alternate_acceptable_shapes": [[6, 6], [7, 6]],
        },
    )
    print(f"  {doc_id}")


# ═══════════════════════════════════════════════════════════════════════════
# 54 — Row-span category column
# ═══════════════════════════════════════════════════════════════════════════


def gen_54_row_span_categories() -> None:
    doc_id = "54_row_span_categories"
    path = HARD / f"{doc_id}.pdf"

    visual = [
        [p("Category", "HHdr"), p("Item", "HHdr"), p("Qty", "HHdr"), p("Unit", "HHdr")],
        [p("Fruit TOKEN_H54_FRUIT", "HHdr"), p("Apples"), p("10"), p("kg")],
        [p(""), p("Bananas"), p("8"), p("kg")],
        [p(""), p("Cherries TOKEN_H54_CHERRY"), p("3"), p("kg")],
        [p("Veg TOKEN_H54_VEG", "HHdr"), p("Carrots"), p("12"), p("kg")],
        [p(""), p("Peas"), p("5"), p("kg")],
        [p("Dairy TOKEN_H54_DAIRY", "HHdr"), p("Milk"), p("20"), p("L")],
        [p(""), p("Cheese TOKEN_H54_CHEESE"), p("4"), p("kg")],
    ]
    t = Table(visual, colWidths=[100, 120, 50, 50])
    t.setStyle(
        TableStyle(
            [
                ("GRID", (0, 0), (-1, -1), 0.6, colors.black),
                ("BACKGROUND", (0, 0), (-1, 0), colors.Color(0.88, 0.88, 0.9)),
                ("SPAN", (0, 1), (0, 3)),  # Fruit
                ("SPAN", (0, 4), (0, 5)),  # Veg
                ("SPAN", (0, 6), (0, 7)),  # Dairy
                ("VALIGN", (0, 0), (0, -1), "MIDDLE"),
                ("FONTSIZE", (0, 0), (-1, -1), 8),
            ]
        )
    )
    story = [
        Paragraph("Row-Span Categories — Hard", styles["HTitle"]),
        Paragraph(
            "TOKEN_HARD54_DOC · First column categories span multiple rows. "
            "Must not invent empty category rows or split incorrectly.",
            styles["HMeta"],
        ),
        Spacer(1, 12),
        t,
    ]
    SimpleDocTemplate(str(path), pagesize=letter, leftMargin=50, rightMargin=50).build(
        story
    )

    write_gt(
        doc_id,
        {
            **base_gt(
                doc_id,
                category="row_span_categories",
                description="Inventory table with rowspan category labels in column 0",
                challenges=["rowspan", "merged_cells", "lattice", "span_text_assign"],
                failure_modes_covered=["WRONG_SHAPE", "ROW_MISCOUNT", "BAD_STRUCTURE"],
                must_contain=[
                    "TOKEN_HARD54_DOC",
                    "TOKEN_H54_FRUIT",
                    "TOKEN_H54_VEG",
                    "TOKEN_H54_DAIRY",
                    "TOKEN_H54_CHERRY",
                    "TOKEN_H54_CHEESE",
                ],
                expected_tables=[
                    {
                        "cells": [
                            ["Category", "Item", "Qty", "Unit"],
                            ["Fruit TOKEN_H54_FRUIT", "Apples", "10", "kg"],
                            ["", "Bananas", "8", "kg"],
                            ["", "Cherries TOKEN_H54_CHERRY", "3", "kg"],
                            ["Veg TOKEN_H54_VEG", "Carrots", "12", "kg"],
                            ["", "Peas", "5", "kg"],
                            ["Dairy TOKEN_H54_DAIRY", "Milk", "20", "L"],
                            ["", "Cheese TOKEN_H54_CHEESE", "4", "kg"],
                        ],
                    }
                ],
                notes="8×4 visual grid with empty category placeholders under spans.",
            ),
        },
    )
    print(f"  {doc_id}")


# ═══════════════════════════════════════════════════════════════════════════
# 55 — Broken corner gaps in lattice
# ═══════════════════════════════════════════════════════════════════════════


def gen_55_gap_broken_corners() -> None:
    doc_id = "55_gap_broken_corners"
    path = HARD / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setTitle("Hard: gap broken corners")
    c.setFont("Helvetica-Bold", 12)
    c.drawString(50, PAGE_H - 40, "Broken Corner Lattice TOKEN_HARD55_DOC")
    c.setFont("Helvetica", 8)
    c.drawString(
        50,
        PAGE_H - 56,
        "Ruling segments intentionally stop short of intersections (gap=3pt). "
        "Must still recover a 5×4 grid.",
    )

    cells = [
        ["Code", "Name", "Qty", "Price"],
        ["P1 TOKEN_H55_P1", "Alpha", "3", "1.50"],
        ["P2", "Bravo", "7", "2.25"],
        ["P3 TOKEN_H55_P3", "Charlie", "2", "9.99"],
        ["P4", "Delta", "11", "0.40"],
    ]
    draw_ruled_grid(
        c,
        60,
        PAGE_H - 280,
        [90, 90, 50, 50],
        [22] * 5,
        cells,
        gap_corners=3.0,
        line_width=0.8,
    )
    c.showPage()
    c.save()

    write_gt(
        doc_id,
        base_gt(
            doc_id,
            category="gap_broken_corners",
            description="Lattice table whose H/V strokes do not meet at corners (3pt gaps)",
            challenges=["broken_lines", "gap_close", "lattice", "incomplete_joints"],
            failure_modes_covered=["MISS_ALL", "BAD_STRUCTURE", "UNDER_DETECT", "WRONG_SHAPE"],
            must_contain=["TOKEN_HARD55_DOC", "TOKEN_H55_P1", "TOKEN_H55_P3", "Charlie"],
            expected_tables=[{"cells": cells}],
            notes="Vector joiners without gap-close may miss the table or invent wrong cell count.",
        ),
    )
    print(f"  {doc_id}")


# ═══════════════════════════════════════════════════════════════════════════
# 56 — Two stacked tables, different col counts, tight gap
# ═══════════════════════════════════════════════════════════════════════════


def gen_56_stacked_uneven() -> None:
    doc_id = "56_stacked_uneven_tables"
    path = HARD / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setFont("Helvetica-Bold", 12)
    c.drawString(50, PAGE_H - 40, "Stacked Uneven Tables TOKEN_HARD56_DOC")
    c.setFont("Helvetica", 8)
    c.drawString(
        50,
        PAGE_H - 54,
        "Only 14pt vertical gap between tables — fusion risk if using global y-snap.",
    )

    top = [
        ["A", "B", "C", "D", "E", "F"],
        ["1 TOKEN_H56_TOP", "2", "3", "4", "5", "6"],
        ["7", "8", "9", "10", "11", "12"],
        ["13", "14", "15", "16", "17", "18"],
    ]
    bot = [
        ["X", "Y"],
        ["yes TOKEN_H56_BOT", "no"],
        ["true", "false"],
        ["on", "off"],
        ["high", "low"],
    ]
    # top table
    draw_ruled_grid(c, 50, PAGE_H - 200, [45] * 6, [18] * 4, top)
    # 14pt gap: top bottom at PAGE_H-200; bot top = that - 14
    bot_h = 18 * 5
    bot_bottom = (PAGE_H - 200) - 14 - bot_h
    draw_ruled_grid(c, 50, bot_bottom, [80, 80], [18] * 5, bot)
    c.showPage()
    c.save()

    write_gt(
        doc_id,
        base_gt(
            doc_id,
            category="stacked_uneven_tables",
            description="Two lattice tables stacked with only 14pt gap; 4×6 then 5×2",
            challenges=[
                "multiple_tables_per_page",
                "tight_vertical_gap",
                "heterogeneous_shapes",
                "anti_mega_grid",
            ],
            failure_modes_covered=["MULTI_TABLE_PAGE", "WRONG_SHAPE", "UNDER_DETECT"],
            must_contain=["TOKEN_HARD56_DOC", "TOKEN_H56_TOP", "TOKEN_H56_BOT"],
            expected_tables=[{"cells": top}, {"cells": bot}],
            notes="Fail mode: one table ~9×6 or wrong col count from merged y-lines.",
        ),
    )
    print(f"  {doc_id}")


# ═══════════════════════════════════════════════════════════════════════════
# 57 — Wide statistical grid
# ═══════════════════════════════════════════════════════════════════════════


def gen_57_wide_statistical() -> None:
    doc_id = "57_wide_statistical_grid"
    path = HARD / f"{doc_id}.pdf"

    headers = ["State"] + [f"Y{y}" for y in range(2014, 2026)]  # 1+12 = 13 cols
    rows = [headers]
    states = [
        ("CA TOKEN_H57_CA", [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21]),
        ("TX", [9, 9, 10, 11, 12, 12, 13, 14, 15, 16, 17, 18]),
        ("NY TOKEN_H57_NY", [8, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13]),
        ("FL", [7, 8, 8, 9, 9, 10, 11, 11, 12, 13, 14, 14]),
        ("IL", [6, 6, 7, 7, 7, 8, 8, 8, 9, 9, 9, 10]),
        ("PA TOKEN_H57_PA", [5, 5, 5, 6, 6, 6, 7, 7, 7, 8, 8, 8]),
        ("OH", [4, 4, 5, 5, 5, 6, 6, 6, 7, 7, 7, 8]),
        ("GA", [3, 4, 4, 4, 5, 5, 5, 6, 6, 7, 7, 7]),
        ("NC", [3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6, 7]),
        ("MI", [2, 3, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6]),
        ("NJ TOKEN_H57_NJ", [2, 2, 3, 3, 3, 3, 4, 4, 4, 5, 5, 5]),
        ("VA", [2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5]),
        ("WA", [1, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 5]),
        ("AZ", [1, 1, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4]),
        ("MA", [1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4]),
    ]
    for name, vals in states:
        rows.append([name] + [str(v) for v in vals])

    # Landscape-ish: use smaller fonts / narrow cols on letter
    col_w = [55] + [32] * 12
    data = [[p(c, "HTiny") for c in row] for row in rows]
    # header bold
    data[0] = [p(c, "HHdr") for c in rows[0]]
    t = Table(data, colWidths=col_w)
    t.setStyle(
        TableStyle(
            [
                ("GRID", (0, 0), (-1, -1), 0.4, colors.black),
                ("BACKGROUND", (0, 0), (-1, 0), colors.Color(0.85, 0.85, 0.88)),
                ("FONTSIZE", (0, 0), (-1, -1), 6.5),
                ("ALIGN", (1, 0), (-1, -1), "RIGHT"),
                ("LEFTPADDING", (0, 0), (-1, -1), 1),
                ("RIGHTPADDING", (0, 0), (-1, -1), 1),
                ("TOPPADDING", (0, 0), (-1, -1), 2),
                ("BOTTOMPADDING", (0, 0), (-1, -1), 2),
            ]
        )
    )
    story = [
        Paragraph("Wide Statistical Grid — Hard", styles["HTitle"]),
        Paragraph(
            "TOKEN_HARD57_DOC · 16×13 ruled statistical table (state × year). "
            "Stresses column snap and noise line rejection.",
            styles["HMeta"],
        ),
        Spacer(1, 8),
        t,
    ]
    SimpleDocTemplate(
        str(path), pagesize=letter, leftMargin=28, rightMargin=28, topMargin=40
    ).build(story)

    write_gt(
        doc_id,
        base_gt(
            doc_id,
            category="wide_statistical_grid",
            description="16-row × 13-col ruled statistical table (states × years)",
            challenges=["wide_table", "many_columns", "dense_numeric", "lattice"],
            failure_modes_covered=["COL_MISCOUNT", "ROW_MISCOUNT", "WRONG_SHAPE", "BAD_STRUCTURE"],
            must_contain=[
                "TOKEN_HARD57_DOC",
                "TOKEN_H57_CA",
                "TOKEN_H57_NY",
                "TOKEN_H57_PA",
                "TOKEN_H57_NJ",
                "Y2014",
                "Y2025",
            ],
            expected_tables=[{"cells": rows}],
            notes="Exact shape 16×13 required for shape_exact credit.",
        ),
    )
    print(f"  {doc_id}")


# ═══════════════════════════════════════════════════════════════════════════
# 58 — Multipage continued single table (stitch)
# ═══════════════════════════════════════════════════════════════════════════


def gen_58_multipage_continued() -> None:
    doc_id = "58_multipage_continued_table"
    path = HARD / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)

    header = ["Date", "Ref", "Description", "Amount"]
    # 36 data rows → 3 pages × 12 rows body (+ header each page visual)
    rows_all = []
    for i in range(1, 37):
        rows_all.append(
            [
                f"2026-01-{(i % 28) + 1:02d}",
                f"R{i:03d}",
                f"Ledger entry {i} TOKEN_H58_R{i}" if i in (1, 13, 25, 36) else f"Ledger entry {i}",
                f"{i * 10.5:.2f}",
            ]
        )

    page_chunks = [rows_all[0:12], rows_all[12:24], rows_all[24:36]]
    for pi, chunk in enumerate(page_chunks):
        c.setFont("Helvetica-Bold", 11)
        c.drawString(
            50,
            PAGE_H - 36,
            f"Continued Ledger TOKEN_HARD58_DOC · page {pi + 1}/3",
        )
        c.setFont("Helvetica", 8)
        c.drawString(
            50,
            PAGE_H - 50,
            "Same column structure continues; logical table is ONE across pages (stitch).",
        )
        cells = [header] + chunk
        draw_ruled_grid(
            c, 50, 80, [70, 50, 280, 60], [16] * len(cells), cells, font_size=7
        )
        if pi < 2:
            c.setFont("Helvetica-Oblique", 7)
            c.drawString(50, 60, "Continued on next page → TOKEN_H58_CONT")
        c.showPage()
    c.save()

    # Gold: either 1 stitched logical table 37×4, or 3 page fragments 13×4 each.
    # Primary: 3 page-local tables (detect without stitch) + note for stitch bonus.
    page_tables = []
    for chunk in page_chunks:
        page_tables.append({"cells": [header] + chunk})

    logical_cells = [header] + rows_all
    write_gt(
        doc_id,
        {
            **base_gt(
                doc_id,
                category="multipage_continued_table",
                description="One ledger table continued across 3 pages (repeating header)",
                challenges=[
                    "multi_page_table",
                    "repeating_header",
                    "table_stitch",
                    "lattice",
                ],
                failure_modes_covered=["MULTI_PAGE_DOC", "ROW_MISCOUNT", "WRONG_SHAPE"],
                must_contain=[
                    "TOKEN_HARD58_DOC",
                    "TOKEN_H58_R1",
                    "TOKEN_H58_R13",
                    "TOKEN_H58_R25",
                    "TOKEN_H58_R36",
                    "TOKEN_H58_CONT",
                ],
                expected_tables=page_tables,
                page_count=3,
                notes=(
                    "Primary gold: 3 page fragments of 13×4 (header+12). "
                    "Stitch-aware systems may emit 1 logical 37×4 — see alternate_logical_table."
                ),
            ),
            "expected_table_count": 3,
            "alternate_logical_table": {
                "rows": 37,
                "cols": 4,
                "cells": logical_cells,
                "note": "If stitch merges pages and drops repeated headers: 1+36 rows also acceptable",
            },
            "stitch_expected": True,
        },
    )
    print(f"  {doc_id}")


# ═══════════════════════════════════════════════════════════════════════════
# 59 — Two stream (borderless) tables separated by prose
# ═══════════════════════════════════════════════════════════════════════════


def gen_59_stream_multi_region() -> None:
    doc_id = "59_stream_multi_region"
    path = HARD / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setFont("Helvetica-Bold", 12)
    c.drawString(50, PAGE_H - 40, "Borderless Multi-Region TOKEN_HARD59_DOC")
    c.setFont("Helvetica", 9)
    y = PAGE_H - 70
    for line in [
        "Prose preamble that is NOT a table. Discussing methodology and sampling.",
        "More narrative text to separate regions. TOKEN_H59_PROSE",
    ]:
        c.drawString(50, y, line)
        y -= 14

    def draw_stream_block(c, x_cols, start_y, rows, font=9):
        c.setFont("Helvetica", font)
        y = start_y
        for row in rows:
            for xi, text in zip(x_cols, row):
                c.drawString(xi, y, text)
            y -= 14
        return y

    t1 = [
        ["Name", "Role", "Office", "Level"],
        ["Ada TOKEN_H59_T1", "Eng", "NYC", "L5"],
        ["Lin", "Design", "SF", "L4"],
        ["Sam", "PM", "Austin", "L5"],
        ["Pat", "SRE", "SEA", "L4"],
        ["Kim", "Data", "BOS", "L3"],
    ]
    y = draw_stream_block(c, [50, 150, 250, 350], y - 10, t1)

    y -= 24
    c.setFont("Helvetica", 9)
    c.drawString(50, y, "Interstitial paragraph — still not a table. TOKEN_H59_MID")
    y -= 28

    t2 = [
        ["City", "Temp", "Humidity"],
        ["NYC TOKEN_H59_T2", "72", "55%"],
        ["SF", "68", "70%"],
        ["CHI", "70", "48%"],
        ["MIA", "84", "80%"],
        ["DEN", "65", "30%"],
        ["SEA", "60", "75%"],
    ]
    draw_stream_block(c, [50, 150, 250], y, t2)

    c.showPage()
    c.save()

    write_gt(
        doc_id,
        base_gt(
            doc_id,
            category="stream_multi_region",
            description="Two borderless stream tables separated by prose on one page",
            challenges=[
                "stream",
                "multiple_tables_per_page",
                "prose_rejection",
                "borderless",
            ],
            failure_modes_covered=["UNDER_DETECT", "MULTI_TABLE_PAGE", "OVER_DETECT", "BAD_STRUCTURE"],
            must_contain=[
                "TOKEN_HARD59_DOC",
                "TOKEN_H59_PROSE",
                "TOKEN_H59_MID",
                "TOKEN_H59_T1",
                "TOKEN_H59_T2",
            ],
            expected_tables=[{"cells": t1}, {"cells": t2}],
            notes="Detect 2 stream tables; prose must not form a third table.",
        ),
    )
    print(f"  {doc_id}")


# ═══════════════════════════════════════════════════════════════════════════
# 60 — Lattice top + stream bottom
# ═══════════════════════════════════════════════════════════════════════════


def gen_60_mixed_lattice_stream() -> None:
    doc_id = "60_mixed_lattice_stream_page"
    path = HARD / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setFont("Helvetica-Bold", 12)
    c.drawString(50, PAGE_H - 40, "Mixed Lattice + Stream TOKEN_HARD60_DOC")
    c.setFont("Helvetica", 8)
    c.drawString(50, PAGE_H - 54, "Top: ruled lattice. Bottom: borderless stream. Both must be found.")

    lattice = [
        ["ID", "Product", "Qty"],
        ["1 TOKEN_H60_LAT", "Widget", "10"],
        ["2", "Gadget", "5"],
        ["3", "Gizmo", "8"],
        ["4", "Doohickey", "2"],
    ]
    draw_ruled_grid(c, 50, PAGE_H - 220, [50, 100, 50], [18] * 5, lattice)

    c.setFont("Helvetica-Oblique", 8)
    c.drawString(50, PAGE_H - 240, "Borderless staff roster below:")

    stream = [
        ["Name", "Dept", "Loc"],
        ["Ada TOKEN_H60_STR", "Eng", "NYC"],
        ["Lin", "Sales", "SF"],
        ["Sam", "Ops", "CHI"],
        ["Pat", "HR", "AUS"],
        ["Kim", "Legal", "BOS"],
    ]
    c.setFont("Helvetica", 9)
    y = PAGE_H - 270
    for row in stream:
        c.drawString(50, y, row[0])
        c.drawString(160, y, row[1])
        c.drawString(250, y, row[2])
        y -= 14

    c.showPage()
    c.save()

    write_gt(
        doc_id,
        base_gt(
            doc_id,
            category="mixed_lattice_stream",
            description="One lattice and one stream table on the same page",
            challenges=["lattice", "stream", "multiple_tables_per_page", "mixed_methods"],
            failure_modes_covered=["UNDER_DETECT", "MULTI_TABLE_PAGE", "WRONG_SHAPE"],
            must_contain=["TOKEN_HARD60_DOC", "TOKEN_H60_LAT", "TOKEN_H60_STR"],
            expected_tables=[{"cells": lattice}, {"cells": stream}],
            notes="Strong lattice must not suppress stream region below.",
        ),
    )
    print(f"  {doc_id}")


# ═══════════════════════════════════════════════════════════════════════════
# 61 — Decorative horizontal rules outside a real table
# ═══════════════════════════════════════════════════════════════════════════


def gen_61_deco_hline_noise() -> None:
    doc_id = "61_decorative_hline_noise"
    path = HARD / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)

    # Many decorative H lines across the page
    draw_deco_hlines(
        c,
        [PAGE_H - 80, PAGE_H - 100, PAGE_H - 120, 120, 100, 80, 60],
        40,
        PAGE_W - 40,
    )
    # Short section dividers
    c.setStrokeColor(colors.Color(0.5, 0.5, 0.55))
    c.setLineWidth(0.5)
    for y in range(200, 500, 18):
        c.line(40, y, 120, y)  # left margin hash marks — noise

    c.setFillColor(colors.black)
    c.setFont("Helvetica-Bold", 12)
    c.drawString(140, PAGE_H - 50, "Section Rules Noise TOKEN_HARD61_DOC")
    c.setFont("Helvetica", 8)
    c.drawString(
        140,
        PAGE_H - 66,
        "Decorative horizontal rules and margin ticks must not become extra rows/cols.",
    )

    cells = [
        ["Key", "Value", "Unit"],
        ["alpha TOKEN_H61_A", "1.23", "m"],
        ["beta", "4.56", "m"],
        ["gamma TOKEN_H61_G", "7.89", "s"],
        ["delta", "0.12", "s"],
        ["epsilon", "3.14", "rad"],
    ]
    draw_ruled_grid(c, 160, PAGE_H - 320, [100, 60, 50], [18] * 6, cells)
    c.showPage()
    c.save()

    write_gt(
        doc_id,
        base_gt(
            doc_id,
            category="decorative_hline_noise",
            description="Real 6×3 lattice amid many decorative horizontal rules and margin ticks",
            challenges=["noise_lines", "decorative_rules", "lattice", "false_rows"],
            failure_modes_covered=["OVER_DETECT", "ROW_MISCOUNT", "WRONG_SHAPE", "BAD_STRUCTURE"],
            must_contain=["TOKEN_HARD61_DOC", "TOKEN_H61_A", "TOKEN_H61_G"],
            expected_tables=[{"cells": cells}],
            notes="expected 1 table 6×3; deco lines must not inflate row count.",
        ),
    )
    print(f"  {doc_id}")


# ═══════════════════════════════════════════════════════════════════════════
# 62 — Two close grids (small horizontal gutter, stacked not side-by-side only)
# ═══════════════════════════════════════════════════════════════════════════


def gen_62_two_close_grids() -> None:
    doc_id = "62_two_close_grids"
    path = HARD / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    c.setFont("Helvetica-Bold", 12)
    c.drawString(50, PAGE_H - 40, "Two Close Grids TOKEN_HARD62_DOC")
    c.setFont("Helvetica", 8)
    c.drawString(
        50,
        PAGE_H - 54,
        "Left and right lattice tables with a narrow gutter; also a third table below.",
    )

    left = [
        ["L", "V"],
        ["a TOKEN_H62_L", "1"],
        ["b", "2"],
        ["c", "3"],
        ["d", "4"],
    ]
    right = [
        ["R", "V", "W"],
        ["x TOKEN_H62_R", "9", "8"],
        ["y", "7", "6"],
        ["z", "5", "4"],
    ]
    bottom = [
        ["M", "N", "O", "P"],
        ["1 TOKEN_H62_B", "2", "3", "4"],
        ["5", "6", "7", "8"],
        ["9", "10", "11", "12"],
        ["13", "14", "15", "16"],
    ]

    draw_ruled_grid(c, 50, PAGE_H - 220, [40, 40], [18] * 5, left)
    # narrow gutter (~20pt)
    draw_ruled_grid(c, 50 + 80 + 20, PAGE_H - 200, [40, 40, 40], [18] * 4, right)
    draw_ruled_grid(c, 50, PAGE_H - 400, [50, 50, 50, 50], [18] * 5, bottom)

    c.showPage()
    c.save()

    write_gt(
        doc_id,
        base_gt(
            doc_id,
            category="two_close_grids",
            description="Two side-by-side lattices with narrow gutter + third table below (3 total)",
            challenges=[
                "multiple_tables_per_page",
                "side_by_side",
                "narrow_gutter",
                "vertical_stack",
                "lattice",
            ],
            failure_modes_covered=[
                "MULTI_TABLE_PAGE",
                "UNDER_DETECT",
                "WRONG_SHAPE",
                "OVER_DETECT",
            ],
            must_contain=[
                "TOKEN_HARD62_DOC",
                "TOKEN_H62_L",
                "TOKEN_H62_R",
                "TOKEN_H62_B",
            ],
            expected_tables=[
                {"cells": left, "label": "left"},
                {"cells": right, "label": "right"},
                {"cells": bottom, "label": "bottom"},
            ],
            notes="Detect 3 tables: 5×2, 4×3, 5×4. Fail: fuse left+right or all three.",
        ),
    )
    print(f"  {doc_id}")


# ═══════════════════════════════════════════════════════════════════════════
# Suites policy + manifest
# ═══════════════════════════════════════════════════════════════════════════

HARD_GENERATORS = [
    gen_50_multi_table_stacked,
    gen_51_multi_table_multipage,
    gen_52_page_border_noise,
    gen_53_column_span_header,
    gen_54_row_span_categories,
    gen_55_gap_broken_corners,
    gen_56_stacked_uneven,
    gen_57_wide_statistical,
    gen_58_multipage_continued,
    gen_59_stream_multi_region,
    gen_60_mixed_lattice_stream,
    gen_61_deco_hline_noise,
    gen_62_two_close_grids,
]


def write_suites_json(hard_ids: list[str]) -> None:
    """Document suite membership. ICDAR is external-only — never listed here as a path."""
    suites = {
        "version": 1,
        "policy": {
            "regression_never_includes_icdar": True,
            "icdar_use": (
                "Competitive analysis only. ICDAR PDFs and gold XML must not be copied "
                "into benchmark/corpus/ or used as regression targets during algorithm "
                "development. Run competitive evals from an external Camelot/ICDAR checkout."
            ),
            "synthetic_hard_purpose": (
                "Hard fixtures synthesize multi-table, span, noise-line, and multipage "
                "failure modes inspired by competitive gaps — original content only."
            ),
        },
        "suites": {
            "regression": {
                "description": "Day-to-day accuracy & improvement suite (no ICDAR).",
                "tiers": ["basic", "stress", "hard"],
                "include_sources": ["synthetic"],
                "exclude_sources": ["icdar"],
                "exclude_tiers": [],
                "notes": "Optionally include tier=real for soft probes; real has weaker grid gold.",
            },
            "regression_grid_gold": {
                "description": "Synthetic docs with full expected_tables cell grids.",
                "filter": {
                    "source": "synthetic",
                    "requires": ["expected_tables"],
                    "tiers": ["basic", "stress", "hard"],
                },
            },
            "regression_hard": {
                "description": "Hard structure suite (50–62). Primary improvement target for lattice multi-region/spans.",
                "tiers": ["hard"],
                "document_ids": hard_ids,
            },
            "regression_basic_stress": {
                "description": "Legacy product fixtures 01–12 + stress 20–27.",
                "tiers": ["basic", "stress"],
            },
            "competitive_icdar": {
                "description": "External ICDAR-2013 head-to-head only. Not part of corpus.",
                "external": True,
                "data_source": "camelot-dev/camelot tests/files/tabula/icdar2013-dataset (external clone)",
                "runner": "docs/camelot-comparison-replication.md",
                "results": [
                    "benchmark/results/camelot_icdar_headtohead.json",
                    "benchmark/results/icdar_failure_analysis.json",
                ],
                "forbidden_in_regression": True,
            },
        },
        "hard_failure_mode_coverage": {
            "MULTI_TABLE_PAGE": [
                "50_multi_table_stacked_page",
                "51_multi_table_multipage",
                "56_stacked_uneven_tables",
                "59_stream_multi_region",
                "60_mixed_lattice_stream_page",
                "62_two_close_grids",
            ],
            "WRONG_SHAPE_SPANS": [
                "53_column_span_header",
                "54_row_span_categories",
            ],
            "NOISE_LINES": [
                "52_page_border_noise",
                "61_decorative_hline_noise",
            ],
            "GAP_LINES": ["55_gap_broken_corners"],
            "MULTI_PAGE": [
                "51_multi_table_multipage",
                "58_multipage_continued_table",
            ],
            "STREAM": [
                "59_stream_multi_region",
                "60_mixed_lattice_stream_page",
            ],
            "WIDE_GRID": ["57_wide_statistical_grid"],
        },
    }
    (CORPUS / "suites.json").write_text(
        json.dumps(suites, indent=2) + "\n", encoding="utf-8"
    )


def rebuild_manifest() -> None:
    items = []
    for p in sorted(GT.glob("*.json")):
        items.append(json.loads(p.read_text(encoding="utf-8")))

    def tier_of(i: dict) -> str:
        t = i.get("tier")
        if t:
            return str(t)
        did = str(i.get("id", ""))
        if did.startswith("0"):
            return "basic"
        if did.startswith("2"):
            return "stress"
        if did.startswith("3") or did.startswith("4"):
            return "real"
        if did.startswith("5") or did.startswith("6"):
            return "hard"
        return "unknown"

    tiers: dict[str, int] = {}
    for i in items:
        t = tier_of(i)
        tiers[t] = tiers.get(t, 0) + 1

    manifest = {
        "name": "pdfparser-competitive-corpus-v3",
        "description": (
            "Basic + stress + hard synthetic regression corpus, plus optional real PDFs. "
            "ICDAR is NOT included (competitive-only, external)."
        ),
        "document_count": len(items),
        "tiers": tiers,
        "suites_file": "corpus/suites.json",
        "documents": items,
    }
    (CORPUS / "manifest.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
    )
    print(f"Manifest: {len(items)} documents · tiers={tiers}")


def main() -> None:
    print("Generating hard synthetic regression suite (no ICDAR)...")
    hard_ids: list[str] = []
    for gen in HARD_GENERATORS:
        gen()
        # recover id from last written is awkward; list after
    hard_ids = sorted(p.stem for p in HARD.glob("*.pdf"))
    write_suites_json(hard_ids)
    rebuild_manifest()
    print(f"Done. Hard fixtures: {len(hard_ids)}")
    for hid in hard_ids:
        print(f"  - {hid}")


if __name__ == "__main__":
    main()
