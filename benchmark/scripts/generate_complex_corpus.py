#!/usr/bin/env python3
"""Generate complex stress fixtures + register real downloaded PDFs into corpus.

Creates harder scenarios than the basic v1 fixtures:
  - Bank-statement multi-page ledgers
  - Overflowing / wrapped cell text
  - Merged header cells (colspan visual)
  - Side-by-side tables
  - Multi-page continuing tables
  - Dense numeric grids
  - Nested-looking layouts + footnotes
  - Overlapping watermark text
  - Invoice line-items with long descriptions

Also copies validated real PDFs from downloads/ into corpus/real/.
"""
from __future__ import annotations

import json
import shutil
from pathlib import Path

from reportlab.lib import colors
from reportlab.lib.enums import TA_LEFT, TA_RIGHT, TA_CENTER
from reportlab.lib.pagesizes import letter, landscape, A4
from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle
from reportlab.lib.units import inch, mm
from reportlab.platypus import (
    SimpleDocTemplate,
    Paragraph,
    Spacer,
    Table,
    TableStyle,
    PageBreak,
    KeepTogether,
    HRFlowable,
)
from reportlab.pdfgen import canvas
from reportlab.lib.utils import simpleSplit

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "corpus"
REAL = CORPUS / "real"
STRESS = CORPUS / "stress"
GT = ROOT / "ground_truth"
DL = ROOT / "downloads"
SOURCES = ROOT / "sources.json"

for d in (CORPUS, REAL, STRESS, GT):
    d.mkdir(parents=True, exist_ok=True)

styles = getSampleStyleSheet()
styles.add(
    ParagraphStyle(
        name="Cell",
        fontName="Helvetica",
        fontSize=7,
        leading=9,
        wordWrap="CJK",
    )
)
styles.add(
    ParagraphStyle(
        name="CellTiny",
        fontName="Helvetica",
        fontSize=6,
        leading=7.5,
    )
)
styles.add(
    ParagraphStyle(
        name="Hdr",
        fontName="Helvetica-Bold",
        fontSize=8,
        leading=10,
        alignment=TA_CENTER,
    )
)
styles.add(
    ParagraphStyle(
        name="BankTitle",
        fontName="Helvetica-Bold",
        fontSize=14,
        leading=16,
    )
)
styles.add(
    ParagraphStyle(
        name="BankMeta",
        fontName="Helvetica",
        fontSize=8,
        leading=10,
    )
)


def write_gt(doc_id: str, data: dict) -> None:
    data = {"id": doc_id, **data}
    (GT / f"{doc_id}.json").write_text(json.dumps(data, indent=2), encoding="utf-8")


def p(text: str, style="Cell"):
    return Paragraph(text.replace("\n", "<br/>"), styles[style])


# ─────────────────────── STRESS SYNTHETIC ───────────────────────

def gen_bank_statement() -> None:
    """Multi-page bank statement with long overflowing descriptions."""
    doc_id = "20_bank_statement_multipage"
    path = STRESS / f"{doc_id}.pdf"

    long_desc = (
        "POS PURCHASE ACME SUPERMARKET #4821 DOWNTOWN BRANCH CARD ENDING 4412 "
        "AUTH 883920 MEMO: GROCERIES PRODUCE DAIRY HOUSEHOLD SUPPLIES — "
        "THIS DESCRIPTION IS INTENTIONALLY VERY LONG TO FORCE CELL WRAP AND OVERFLOW "
        "TOKEN_BANK_OVERFLOW_DESC"
    )
    long_desc2 = (
        "WIRE TRANSFER INCOMING REF WT-2026-77821 FROM GLOBAL INDUSTRIES LLC "
        "FOR INVOICE INV-88901 SERVICES RENDERED Q1 2026 INCLUDING CONSULTING "
        "TRAVEL REIMBURSEMENT AND SOFTWARE LICENSES TOKEN_BANK_WIRE"
    )

    rows_header = [
        p("Date", "Hdr"),
        p("Description", "Hdr"),
        p("Type", "Hdr"),
        p("Amount", "Hdr"),
        p("Balance", "Hdr"),
    ]

    transactions = []
    # page-break heavy statement
    base_bal = 12543.22
    for i in range(1, 46):
        if i % 7 == 0:
            desc = long_desc if i % 14 != 0 else long_desc2
            typ = "POS" if i % 14 != 0 else "WIRE"
            amt = -(12.5 + i * 1.37) if typ == "POS" else (2500 + i * 10.25)
        elif i % 5 == 0:
            desc = f"ACH PAYROLL DEPOSIT ACME CORP PE {i:02d} TOKEN_BANK_PAYROLL"
            typ = "ACH"
            amt = 3200.00
        elif i % 3 == 0:
            desc = f"ONLINE TRANSFER TO SAVINGS ****9910 CONF {9000+i} TOKEN_BANK_XFER"
            typ = "XFER"
            amt = -500.00
        else:
            desc = f"CHECK #{1000+i} PAYEE VENDOR SERVICES LLC TOKEN_BANK_CHECK"
            typ = "CHK"
            amt = -(45.0 + i)

        base_bal += amt
        transactions.append(
            [
                p(f"2026-0{(i%9)+1}-{10+(i%18):02d}"),
                p(desc),
                p(typ),
                p(f"{amt:,.2f}", "Cell"),
                p(f"{base_bal:,.2f}"),
            ]
        )

    # split into chunks for multipage feel via one long table that flows
    story = []
    story.append(Paragraph("NORTHSTAR NATIONAL BANK", styles["BankTitle"]))
    story.append(Paragraph(
        "Account Statement · Checking ****4412 · Statement period 01 Jan 2026 – 31 Mar 2026 · "
        "TOKEN_BANK_STATEMENT_HEADER · Customer: JANE Q. PUBLIC",
        styles["BankMeta"],
    ))
    story.append(Spacer(1, 8))
    story.append(Paragraph(
        "Beginning balance: $12,543.22 · TOKEN_BANK_BEGIN_BAL · "
        "Questions? 1-800-555-0199 · Page continues across multiple pages with dense ledger rows.",
        styles["BankMeta"],
    ))
    story.append(Spacer(1, 10))

    data = [rows_header] + transactions
    col_w = [70, 280, 45, 70, 70]
    t = Table(data, colWidths=col_w, repeatRows=1)
    t.setStyle(
        TableStyle(
            [
                ("GRID", (0, 0), (-1, -1), 0.4, colors.black),
                ("BACKGROUND", (0, 0), (-1, 0), colors.Color(0.15, 0.25, 0.45)),
                ("TEXTCOLOR", (0, 0), (-1, 0), colors.white),
                ("VALIGN", (0, 0), (-1, -1), "TOP"),
                ("LEFTPADDING", (0, 0), (-1, -1), 3),
                ("RIGHTPADDING", (0, 0), (-1, -1), 3),
                ("TOPPADDING", (0, 0), (-1, -1), 2),
                ("BOTTOMPADDING", (0, 0), (-1, -1), 2),
                ("ROWBACKGROUNDS", (0, 1), (-1, -1), [colors.white, colors.Color(0.95, 0.96, 0.98)]),
                ("ALIGN", (3, 1), (-1, -1), "RIGHT"),
            ]
        )
    )
    story.append(t)
    story.append(Spacer(1, 12))
    story.append(Paragraph(
        "End of statement TOKEN_BANK_END · Fees: $12.00 monthly maintenance · "
        "Overdraft protection: linked savings ****9910",
        styles["BankMeta"],
    ))

    doc = SimpleDocTemplate(
        str(path),
        pagesize=letter,
        leftMargin=36,
        rightMargin=36,
        topMargin=36,
        bottomMargin=36,
        title="Bank Statement Stress Fixture",
    )
    doc.build(story)

    write_gt(
        doc_id,
        {
            "category": "bank_statement",
            "tier": "stress",
            "description": "Multi-page bank ledger with long wrapped descriptions, ACH/wire/POS, repeating header",
            "challenges": [
                "long_cell_wrap",
                "multi_page_table",
                "dense_rows",
                "mixed_transaction_types",
                "numeric_alignment",
                "repeating_header",
            ],
            "source": "synthetic",
            "path": f"corpus/stress/{doc_id}.pdf",
            "must_contain": [
                "TOKEN_BANK_STATEMENT_HEADER",
                "TOKEN_BANK_OVERFLOW_DESC",
                "TOKEN_BANK_WIRE",
                "TOKEN_BANK_PAYROLL",
                "TOKEN_BANK_BEGIN_BAL",
                "TOKEN_BANK_END",
                "NORTHSTAR NATIONAL BANK",
            ],
            "table_cells_must_include": [
                "TOKEN_BANK_OVERFLOW_DESC",
                "TOKEN_BANK_WIRE",
                "Description",
                "Balance",
                "WIRE",
                "ACH",
            ],
            "expected_tables_min": 1,
            "expected_pages_min": 2,
            "our_library_priority": "P2 tables + multi-page",
        },
    )


def gen_overflow_cells() -> None:
    """Table where text deliberately overflows row height / column width expectations."""
    doc_id = "21_table_overflow_cells"
    path = STRESS / f"{doc_id}.pdf"

    mega = (
        "TOKEN_OVERFLOW_MEGA This cell contains an extremely long unbreakable-ish narrative: "
        + ("word-wrap-stress-" * 40)
        + " END_OVERFLOW_MEGA"
    )
    # narrow columns force aggressive wrap
    header = [p(h, "Hdr") for h in ["ID", "Short", "Overflowing Description", "Amt"]]
    rows = [header]
    for i in range(1, 12):
        if i in (3, 7, 10):
            desc = p(mega if i == 3 else mega.replace("MEGA", f"MEGA{i}"))
        else:
            desc = p(
                f"Normal row {i} with moderate text that still wraps twice under narrow column "
                f"TOKEN_OVERFLOW_ROW_{i}"
            )
        rows.append(
            [
                p(f"R{i:02d}"),
                p("OK" if i % 2 else "LONGSHORTLABEL_TOKEN"),
                desc,
                p(f"{i * 99.99:.2f}"),
            ]
        )

    # also a row with multi-line forced breaks and a tiny column
    rows.append(
        [
            p("R99"),
            p("X"),
            p(
                "Line1 TOKEN_OVERFLOW_MULTILINE<br/>Line2 continues inside same cell<br/>"
                "Line3 still same cell with nested meaning"
            ),
            p("0.01"),
        ]
    )

    t = Table(rows, colWidths=[40, 55, 340, 50])
    t.setStyle(
        TableStyle(
            [
                ("GRID", (0, 0), (-1, -1), 0.6, colors.black),
                ("BACKGROUND", (0, 0), (-1, 0), colors.lightgrey),
                ("VALIGN", (0, 0), (-1, -1), "TOP"),
                ("FONTSIZE", (0, 0), (-1, -1), 7),
                # deliberately small row min height conflict via padding
                ("LEFTPADDING", (0, 0), (-1, -1), 2),
                ("RIGHTPADDING", (0, 0), (-1, -1), 2),
                ("TOPPADDING", (0, 0), (-1, -1), 1),
                ("BOTTOMPADDING", (0, 0), (-1, -1), 1),
            ]
        )
    )

    story = [
        Paragraph("Overflow / Wrapping Cell Stress Table", styles["BankTitle"]),
        Paragraph(
            "Cells intentionally force multi-line wrap and large vertical growth. "
            "TOKEN_OVERFLOW_DOC · Parsers often split rows incorrectly.",
            styles["BankMeta"],
        ),
        Spacer(1, 10),
        t,
    ]
    SimpleDocTemplate(str(path), pagesize=letter, leftMargin=40, rightMargin=40).build(story)

    write_gt(
        doc_id,
        {
            "category": "table_overflow",
            "tier": "stress",
            "description": "Narrow columns + multi-line overflowing cells; row-split failure mode",
            "challenges": [
                "cell_text_wrap",
                "uneven_row_heights",
                "narrow_columns",
                "multiline_cells",
                "row_segmentation",
            ],
            "source": "synthetic",
            "path": f"corpus/stress/{doc_id}.pdf",
            "must_contain": [
                "TOKEN_OVERFLOW_DOC",
                "TOKEN_OVERFLOW_MEGA",
                "END_OVERFLOW_MEGA",
                "TOKEN_OVERFLOW_MULTILINE",
                "TOKEN_OVERFLOW_ROW_1",
            ],
            "table_cells_must_include": [
                "TOKEN_OVERFLOW_MEGA",
                "TOKEN_OVERFLOW_MULTILINE",
                "Overflowing Description",
                "R99",
            ],
            "expected_tables_min": 1,
            "quality_notes": "Success only if mega text stays in ONE cell / same row structure",
            "our_library_priority": "P2 table cell integrity",
        },
    )


def gen_merged_headers() -> None:
    """Financial table with visual colspan headers (merged cells)."""
    doc_id = "22_table_merged_headers"
    path = STRESS / f"{doc_id}.pdf"

    # Row0: merged group headers via SPAN
    data = [
        [
            p("Metric", "Hdr"),
            p("FY2024", "Hdr"),
            p("", "Hdr"),
            p("FY2025", "Hdr"),
            p("", "Hdr"),
            p("Notes", "Hdr"),
        ],
        [
            p(""),
            p("Actual", "Hdr"),
            p("Budget", "Hdr"),
            p("Actual", "Hdr"),
            p("Budget", "Hdr"),
            p(""),
        ],
        [p("Revenue"), p("1,200"), p("1,100"), p("1,450"), p("1,400"), p("TOKEN_MERGE_REV organic growth")],
        [p("COGS"), p("480"), p("500"), p("560"), p("550"), p("supply")],
        [
            p("Gross Profit"),
            p("720"),
            p("600"),
            p("890"),
            p("850"),
            p("TOKEN_MERGE_GP"),
        ],
        [
            p("Operating Expenses — Sales & Marketing plus G&A combined line that wraps"),
            p("400"),
            p("420"),
            p("470"),
            p("460"),
            p("hiring TOKEN_MERGE_OPEX"),
        ],
        [p("EBITDA"), p("320"), p("180"), p("420"), p("390"), p("TOKEN_MERGE_EBITDA")],
    ]

    t = Table(data, colWidths=[150, 55, 55, 55, 55, 130])
    t.setStyle(
        TableStyle(
            [
                ("GRID", (0, 0), (-1, -1), 0.5, colors.black),
                ("SPAN", (1, 0), (2, 0)),  # FY2024 spans 2
                ("SPAN", (3, 0), (4, 0)),  # FY2025 spans 2
                ("SPAN", (0, 0), (0, 1)),  # Metric vertical span
                ("SPAN", (5, 0), (5, 1)),  # Notes vertical span
                ("BACKGROUND", (0, 0), (-1, 1), colors.Color(0.85, 0.9, 0.95)),
                ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
                ("ALIGN", (1, 0), (4, -1), "CENTER"),
                ("FONTSIZE", (0, 0), (-1, -1), 8),
            ]
        )
    )

    story = [
        Paragraph("Merged Header / Colspan Stress Table", styles["BankTitle"]),
        Paragraph(
            "Two-level headers with SPAN merges. TOKEN_MERGE_DOC · "
            "Libraries often duplicate or drop rowspan/colspan cells.",
            styles["BankMeta"],
        ),
        Spacer(1, 12),
        t,
    ]
    SimpleDocTemplate(str(path), pagesize=letter).build(story)

    write_gt(
        doc_id,
        {
            "category": "table_merged_headers",
            "tier": "stress",
            "description": "Two-row headers with horizontal and vertical spans",
            "challenges": [
                "colspan",
                "rowspan",
                "multi_level_header",
                "wrapped_stub_column",
                "empty_placeholder_cells",
            ],
            "source": "synthetic",
            "path": f"corpus/stress/{doc_id}.pdf",
            "must_contain": [
                "TOKEN_MERGE_DOC",
                "TOKEN_MERGE_REV",
                "TOKEN_MERGE_GP",
                "TOKEN_MERGE_EBITDA",
                "FY2024",
                "FY2025",
                "Actual",
                "Budget",
            ],
            "table_cells_must_include": [
                "Revenue",
                "EBITDA",
                "1,450",
                "TOKEN_MERGE_EBITDA",
                "FY2024",
            ],
            "expected_tables_min": 1,
            "our_library_priority": "P2 structure/spans",
        },
    )


def gen_side_by_side_tables() -> None:
    doc_id = "23_side_by_side_tables"
    path = STRESS / f"{doc_id}.pdf"

    left = [
        [p("City", "Hdr"), p("Temp", "Hdr")],
        [p("NYC TOKEN_SIDE_L"), p("72")],
        [p("SF"), p("68")],
        [p("CHI"), p("70")],
    ]
    right = [
        [p("City", "Hdr"), p("Pop(M)", "Hdr")],
        [p("NYC TOKEN_SIDE_R"), p("8.3")],
        [p("SF"), p("0.8")],
        [p("CHI"), p("2.7")],
        [p("Extra row only on right"), p("—")],
    ]
    t1 = Table(left, colWidths=[100, 50])
    t2 = Table(right, colWidths=[120, 50])
    for t in (t1, t2):
        t.setStyle(
            TableStyle(
                [
                    ("GRID", (0, 0), (-1, -1), 0.5, colors.black),
                    ("BACKGROUND", (0, 0), (-1, 0), colors.Color(0.9, 0.9, 0.9)),
                ]
            )
        )
    wrap = Table([[t1, t2]], colWidths=[200, 220])
    wrap.setStyle(TableStyle([("VALIGN", (0, 0), (-1, -1), "TOP"), ("LEFTPADDING", (1, 0), (1, 0), 30)]))

    story = [
        Paragraph("Side-by-Side Tables Stress", styles["BankTitle"]),
        Paragraph(
            "Two independent tables on the same baseline. TOKEN_SIDE_DOC · "
            "Parsers may fuse into one table or lose one side.",
            styles["BankMeta"],
        ),
        Spacer(1, 14),
        wrap,
    ]
    SimpleDocTemplate(str(path), pagesize=letter).build(story)

    write_gt(
        doc_id,
        {
            "category": "side_by_side_tables",
            "tier": "stress",
            "description": "Two adjacent tables with different row counts",
            "challenges": ["multiple_tables_per_page", "horizontal_layout", "asymmetric_rows"],
            "source": "synthetic",
            "path": f"corpus/stress/{doc_id}.pdf",
            "must_contain": ["TOKEN_SIDE_DOC", "TOKEN_SIDE_L", "TOKEN_SIDE_R", "Extra row only on right"],
            "table_cells_must_include": ["TOKEN_SIDE_L", "TOKEN_SIDE_R", "Temp", "Pop(M)"],
            "expected_tables_min": 2,
            "our_library_priority": "P2 multi-table NMS",
        },
    )


def gen_invoice_complex() -> None:
    doc_id = "24_invoice_line_items"
    path = STRESS / f"{doc_id}.pdf"

    items = []
    header = [p(h, "Hdr") for h in ["#", "SKU", "Description", "Qty", "Unit", "Total"]]
    items.append(header)
    descs = [
        "Professional services — architecture review workshops spanning multiple days with remote stakeholders TOKEN_INV_SVC",
        "Cloud compute usage: 12,480 vCPU-hours across us-east-1 and eu-west-1 including egress TOKEN_INV_CLOUD",
        "Support retainer (Gold) — 24x7 with 15-minute Sev-1 response; period 2026-01-01 to 2026-03-31 TOKEN_INV_SUPPORT",
        "Travel pass-through: flights, hotels, ground transport per policy POL-22; receipts on file TOKEN_INV_TRAVEL",
    ]
    for i, d in enumerate(descs, 1):
        qty = 1 if i != 2 else 12480
        unit = 15000 if i == 1 else (0.042 if i == 2 else (4500 if i == 3 else 2187.55))
        total = qty * unit if i != 2 else qty * unit
        items.append(
            [
                p(str(i)),
                p(f"SKU-{100+i}"),
                p(d),
                p(str(qty)),
                p(f"{unit:,.2f}"),
                p(f"{total:,.2f}"),
            ]
        )
    # totals block as nested-looking second table
    items.append([p(""), p(""), p("Subtotal TOKEN_INV_SUB", "Cell"), p(""), p(""), p("22,711.15")])
    items.append([p(""), p(""), p("Tax 8.875%"), p(""), p(""), p("2,015.61")])
    items.append([p(""), p(""), p("Amount Due TOKEN_INV_DUE", "Hdr"), p(""), p(""), p("24,726.76")])

    t = Table(items, colWidths=[25, 55, 280, 45, 55, 60])
    t.setStyle(
        TableStyle(
            [
                ("GRID", (0, 0), (-1, 4), 0.4, colors.black),
                ("BOX", (0, 0), (-1, -1), 0.8, colors.black),
                ("LINEBELOW", (0, 0), (-1, 0), 1, colors.black),
                ("BACKGROUND", (0, 0), (-1, 0), colors.Color(0.2, 0.2, 0.2)),
                ("TEXTCOLOR", (0, 0), (-1, 0), colors.white),
                ("VALIGN", (0, 0), (-1, -1), "TOP"),
                ("SPAN", (2, 5), (4, 5)),
                ("SPAN", (2, 6), (4, 6)),
                ("SPAN", (2, 7), (4, 7)),
                ("ALIGN", (3, 1), (-1, -1), "RIGHT"),
                ("BACKGROUND", (0, 7), (-1, 7), colors.Color(0.95, 0.95, 0.85)),
            ]
        )
    )

    story = [
        Paragraph("INVOICE INV-2026-00421 TOKEN_INV_HEADER", styles["BankTitle"]),
        Paragraph(
            "Bill To: Contoso Ltd · 100 Main St · Due: Net 30 · PO: PO-7781 · "
            "Remit to: Northstar Bank ****4412",
            styles["BankMeta"],
        ),
        Spacer(1, 12),
        t,
        Spacer(1, 10),
        Paragraph(
            "Notes: Wire reference must include INV-2026-00421. Late fee 1.5%/mo. TOKEN_INV_NOTES",
            styles["BankMeta"],
        ),
    ]
    SimpleDocTemplate(str(path), pagesize=letter, leftMargin=40, rightMargin=40).build(story)

    write_gt(
        doc_id,
        {
            "category": "invoice",
            "tier": "stress",
            "description": "Invoice with long line-item descriptions + totals spans",
            "challenges": [
                "long_line_items",
                "totals_section",
                "span_rows",
                "mixed_header_body_footer",
                "currency_formatting",
            ],
            "source": "synthetic",
            "path": f"corpus/stress/{doc_id}.pdf",
            "must_contain": [
                "TOKEN_INV_HEADER",
                "TOKEN_INV_SVC",
                "TOKEN_INV_CLOUD",
                "TOKEN_INV_DUE",
                "TOKEN_INV_NOTES",
                "INV-2026-00421",
            ],
            "table_cells_must_include": [
                "TOKEN_INV_SVC",
                "TOKEN_INV_CLOUD",
                "TOKEN_INV_DUE",
                "SKU-101",
                "Description",
            ],
            "expected_tables_min": 1,
            "our_library_priority": "P2 invoices",
        },
    )


def gen_dense_numeric() -> None:
    doc_id = "25_dense_numeric_grid"
    path = STRESS / f"{doc_id}.pdf"
    # landscape dense grid 20 cols x 30 rows
    cols = 12
    header = [p(f"C{c}", "Hdr") for c in range(cols)]
    data = [header]
    for r in range(30):
        row = []
        for c in range(cols):
            val = (r + 1) * 100 + c + (0.01 * ((r * cols + c) % 97))
            token = " TOKEN_DENSE" if r == 15 and c == 6 else ""
            row.append(p(f"{val:,.2f}{token}", "CellTiny"))
        data.append(row)
    t = Table(data, colWidths=[55] * cols)
    t.setStyle(
        TableStyle(
            [
                ("GRID", (0, 0), (-1, -1), 0.25, colors.grey),
                ("BACKGROUND", (0, 0), (-1, 0), colors.Color(0.3, 0.3, 0.35)),
                ("TEXTCOLOR", (0, 0), (-1, 0), colors.white),
                ("FONTSIZE", (0, 0), (-1, -1), 6),
                ("ALIGN", (0, 0), (-1, -1), "RIGHT"),
                ("LEFTPADDING", (0, 0), (-1, -1), 1),
                ("RIGHTPADDING", (0, 0), (-1, -1), 1),
                ("TOPPADDING", (0, 0), (-1, -1), 1),
                ("BOTTOMPADDING", (0, 0), (-1, -1), 1),
            ]
        )
    )
    story = [
        Paragraph("Dense Numeric Grid TOKEN_DENSE_DOC", styles["BankTitle"]),
        Paragraph(
            "12×30 tight numeric table — stress for cell segmentation and memory.",
            styles["BankMeta"],
        ),
        Spacer(1, 8),
        t,
    ]
    SimpleDocTemplate(
        str(path), pagesize=landscape(letter), leftMargin=24, rightMargin=24, topMargin=24, bottomMargin=24
    ).build(story)

    write_gt(
        doc_id,
        {
            "category": "dense_numeric",
            "tier": "stress",
            "description": "Landscape 12x30 dense numeric grid",
            "challenges": ["high_cell_count", "tiny_fonts", "numeric_only", "landscape"],
            "source": "synthetic",
            "path": f"corpus/stress/{doc_id}.pdf",
            "must_contain": ["TOKEN_DENSE_DOC", "TOKEN_DENSE", "C0", "C11"],
            "table_cells_must_include": ["TOKEN_DENSE", "C0", "C6"],
            "expected_tables_min": 1,
            "expected_min_cells": 300,
            "our_library_priority": "P2 performance/quality",
        },
    )


def gen_watermark_overlap() -> None:
    doc_id = "26_watermark_overlap"
    path = STRESS / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    w, h = letter
    # content table-like
    c.setFont("Helvetica-Bold", 14)
    c.drawString(72, h - 72, "Confidential Report TOKEN_WM_DOC")
    c.setFont("Helvetica", 10)
    y = h - 120
    rows = [
        ("Item", "Status", "Owner"),
        ("Alpha", "Open", "A. Smith"),
        ("Beta", "Closed", "B. Jones"),
        ("Gamma TOKEN_WM_ROW", "Blocked", "C. Lee"),
    ]
    for i, row in enumerate(rows):
        x = 72
        for cell in row:
            c.drawString(x, y, cell)
            x += 150
        y -= 22
        if i == 0:
            c.line(72, y + 16, 500, y + 16)
    # big diagonal watermark over text
    c.saveState()
    c.setFillColor(colors.Color(0.85, 0.1, 0.1, alpha=0.35))
    c.setFont("Helvetica-Bold", 48)
    c.translate(300, 400)
    c.rotate(40)
    c.drawCentredString(0, 0, "DRAFT WATERMARK TOKEN_WM_MARK")
    c.restoreState()
    c.setFont("Helvetica", 9)
    c.drawString(72, 100, "Footer text should remain extractable TOKEN_WM_FOOTER")
    c.showPage()
    c.save()

    write_gt(
        doc_id,
        {
            "category": "watermark_overlap",
            "tier": "stress",
            "description": "Diagonal watermark overlapping table-like text",
            "challenges": ["overlapping_text", "watermark", "z_order", "reading_order_noise"],
            "source": "synthetic",
            "path": f"corpus/stress/{doc_id}.pdf",
            "must_contain": [
                "TOKEN_WM_DOC",
                "TOKEN_WM_ROW",
                "TOKEN_WM_FOOTER",
                "TOKEN_WM_MARK",
            ],
            "expected_tables_min": 0,
            "quality_notes": "Watermark text may interleave; libraries differ on order",
            "our_library_priority": "P1 reading order / artifacts",
        },
    )


def gen_footnote_interrupted() -> None:
    doc_id = "27_table_with_footnotes"
    path = STRESS / f"{doc_id}.pdf"
    data = [
        [p("Country", "Hdr"), p("GDP", "Hdr"), p("Pop", "Hdr")],
        [p("USA¹ TOKEN_FN_USA"), p("25.5"), p("331")],
        [p("CHN²"), p("18.1"), p("1412")],
        [p("DEU"), p("4.1"), p("83")],
        [p("IND³ TOKEN_FN_IND"), p("3.4"), p("1408")],
    ]
    t = Table(data, colWidths=[160, 80, 80])
    t.setStyle(
        TableStyle(
            [
                ("GRID", (0, 0), (-1, -1), 0.5, colors.black),
                ("BACKGROUND", (0, 0), (-1, 0), colors.lightgrey),
            ]
        )
    )
    story = [
        Paragraph("Macro Indicators TOKEN_FN_DOC", styles["BankTitle"]),
        Spacer(1, 8),
        t,
        Spacer(1, 16),
        Paragraph("¹ Includes territories. TOKEN_FN_1", styles["BankMeta"]),
        Paragraph("² Official statistics as reported. TOKEN_FN_2", styles["BankMeta"]),
        Paragraph(
            "³ Provisional estimates subject to revision; may be confused as table row. TOKEN_FN_3",
            styles["BankMeta"],
        ),
    ]
    SimpleDocTemplate(str(path), pagesize=letter).build(story)
    write_gt(
        doc_id,
        {
            "category": "table_footnotes",
            "tier": "stress",
            "description": "Table with superscript markers + footnote block below",
            "challenges": ["footnotes", "superscripts", "table_boundary", "annotation_like_text"],
            "source": "synthetic",
            "path": f"corpus/stress/{doc_id}.pdf",
            "must_contain": ["TOKEN_FN_DOC", "TOKEN_FN_USA", "TOKEN_FN_IND", "TOKEN_FN_1", "TOKEN_FN_3"],
            "table_cells_must_include": ["TOKEN_FN_USA", "TOKEN_FN_IND", "GDP", "USA"],
            "expected_tables_min": 1,
            "our_library_priority": "P2 table boundary",
        },
    )


# ─────────────────────── REAL PDF REGISTRY ───────────────────────

REAL_SPECS = [
    {
        "id": "30_real_ca_warn_report",
        "src": "plumber_ca_warn_report.pdf",
        "category": "real_government_table_dump",
        "description": "CA WARN multi-page layoff notices — classic pdfplumber demo (wide tables, many rows)",
        "challenges": ["wide_tables", "multi_page", "many_rows", "government_layout"],
        "must_contain": ["Notice Date", "Company"],
        "must_contain_any": ["Employees", "WARN", "Layoff", "Closure", "Location"],
        "expected_pages_min": 10,
        "expected_tables_min": 1,
        "table_cells_must_include": [],
        "source_url": "https://github.com/jsvine/pdfplumber/blob/stable/examples/pdfs/ca-warn-report.pdf",
        "license_note": "Example from pdfplumber project (public demo data)",
    },
    {
        "id": "31_real_background_checks",
        "src": "plumber_background_checks.pdf",
        "category": "real_wide_header_table",
        "description": "NICS background checks table — multi-line headers, dense columns",
        "challenges": ["multi_line_headers", "dense_columns", "abbreviations", "single_page_wide"],
        "must_contain": ["Handgun", "Long Gun"],
        "must_contain_any": ["Permit", "State", "Multiple", "Admin"],
        "expected_pages_min": 1,
        "expected_tables_min": 1,
        "source_url": "https://github.com/jsvine/pdfplumber/blob/stable/examples/pdfs/background-checks.pdf",
        "license_note": "pdfplumber examples",
    },
    {
        "id": "32_real_census_table324",
        "src": "tabula_12s0324.pdf",
        "category": "real_statistical_abstract",
        "description": "US Census Statistical Abstract table — classic Tabula lattice sample",
        "challenges": ["statistical_table", "footnotes_likely", "numeric_columns", "title_above_table"],
        "must_contain": ["Law Enforcement"],
        "must_contain_any": ["Table 324", "Census", "Prison", "Arrest"],
        "expected_pages_min": 1,
        "expected_tables_min": 1,
        "source_url": "tabula-java test resources",
        "license_note": "US government work / Tabula test fixture",
    },
    {
        "id": "33_real_argentina_votes",
        "src": "tabula_argentina_diputados_voting.pdf",
        "category": "real_complex_grid",
        "description": "Argentina diputados voting record — irregular grid, multi-section",
        "challenges": ["irregular_grid", "political_layout", "sectioned_tables", "spanish"],
        "must_contain": ["Votos"],
        "must_contain_any": ["Afirmativos", "Negativos", "Abstenciones", "Diputados"],
        "expected_pages_min": 1,
        "expected_tables_min": 1,
        "source_url": "tabula-java test resources",
        "license_note": "Tabula test fixture",
    },
    {
        "id": "34_real_schools_contributions",
        "src": "tabula_schools.pdf",
        "category": "real_campaign_finance",
        "description": "Campaign contribution report multi-page — stream-like rows",
        "challenges": ["multi_page", "stream_like", "name_address_money", "period_headers"],
        "must_contain": ["Schools"],
        "must_contain_any": ["Contribution", "Amount", "Name", "Committee", "Yes"],
        "expected_pages_min": 3,
        "expected_tables_min": 1,
        "source_url": "tabula-java test resources",
        "license_note": "Tabula test fixture",
    },
    {
        "id": "35_real_camelot_fuel",
        "src": "camelot_foo.pdf",
        "category": "real_lattice_document",
        "description": "Camelot foo.pdf — multi-table fuel savings research pages",
        "challenges": ["multiple_tables", "scientific_layout", "figure_plus_table"],
        "must_contain": ["Fuel"],
        "must_contain_any": ["Savings", "Driving", "Table", "Quantifying"],
        "expected_pages_min": 1,
        "expected_tables_min": 1,
        "source_url": "camelot docs sample",
        "license_note": "Camelot project sample",
    },
    {
        "id": "36_real_two_tables",
        "src": "camelot_left_twocol.pdf",
        "category": "real_two_tables",
        "description": "Camelot twotables — disease outbreak table(s)",
        "challenges": ["two_tables", "district_disease_counts", "government_health"],
        "must_contain": ["District"],
        "must_contain_any": ["Disease", "Cases", "Deaths", "State"],
        "expected_pages_min": 1,
        "expected_tables_min": 1,
        "source_url": "camelot tests/files/twotables_1.pdf",
        "license_note": "Camelot test fixture",
    },
    {
        "id": "37_real_liabilities_superscript",
        "src": "camelot_superscript.pdf",
        "category": "real_superscript_table",
        "description": "State-wise liabilities table with superscripts (Contd.)",
        "challenges": ["superscripts", "continued_table", "financial_columns", "contd_marker"],
        "must_contain": ["TABLE"],
        "must_contain_any": ["LIABILITIES", "States", "Billion", "1997"],
        "expected_pages_min": 1,
        "expected_tables_min": 1,
        "source_url": "camelot tests/files/superscript.pdf",
        "license_note": "Camelot test fixture",
    },
    {
        "id": "38_real_irs_f1040",
        "src": "irs_f1040.pdf",
        "category": "real_tax_form",
        "description": "IRS Form 1040 — form fields, lines, labels, not a clean data table",
        "challenges": ["acroform_like_lines", "tax_form_layout", "label_value_pairs", "multi_column_form"],
        "must_contain": ["1040"],
        "must_contain_any": ["Income", "Tax", "Return", "IRS", "Spouse"],
        "expected_pages_min": 1,
        "expected_tables_min": 0,
        "source_url": "https://www.irs.gov/pub/irs-pdf/f1040.pdf",
        "license_note": "US government work",
    },
    {
        "id": "39_real_fed_beigebook",
        "src": "fed_beigebook.pdf",
        "category": "real_long_report",
        "description": "Fed Beige Book (~50+ pages) — long narrative + district sections",
        "challenges": ["long_document", "section_headers", "narrative_heavy", "mixed_layout"],
        "must_contain": ["Beige Book"],
        "must_contain_any": ["Federal Reserve", "District", "Economic", "employment", "inflation"],
        "expected_pages_min": 40,
        "expected_tables_min": 0,
        "source_url": "https://www.federalreserve.gov/monetarypolicy/files/BeigeBook_20240306.pdf",
        "license_note": "US government work",
    },
    {
        "id": "40_real_arxiv_tensorflow",
        "src": "arxiv_1603.04467.pdf",
        "category": "real_scientific_multicol",
        "description": "TensorFlow whitepaper (arXiv) — multi-column scientific layout, figures, refs",
        "challenges": ["multi_column", "scientific", "figures", "references", "equations_possible"],
        "must_contain": ["TensorFlow"],
        "must_contain_any": ["machine learning", "Distributed", "Abstract", "Google"],
        "expected_pages_min": 10,
        "expected_tables_min": 0,
        "source_url": "https://arxiv.org/abs/1603.04467",
        "license_note": "arXiv paper — check paper license for redistribution",
    },
    {
        "id": "41_real_nist_withdrawn_notice",
        "src": "nist_fips197.pdf",
        "category": "real_technical_long",
        "description": "NIST technical series PDF (may be withdrawal wrapper + body) — long formal doc",
        "challenges": ["long_document", "formal_publication", "headers_footers"],
        "must_contain_any": ["NIST", "FIPS", "AES", "Publication", "Withdrawn", "Federal"],
        "must_contain": [],
        "expected_pages_min": 5,
        "expected_tables_min": 0,
        "source_url": "NIST publication portal",
        "license_note": "US government work",
    },
    {
        "id": "42_real_insurance_italian",
        "src": "camelot_table_regions.pdf",
        "category": "real_insurance_prose_tables",
        "description": "Italian insurance product PDF with embedded tables/regions",
        "challenges": ["non_english", "prose_plus_tables", "multi_region", "complex_layout"],
        "must_contain_any": ["Vittoria", "PIR", "assicurazione", "contratto", "FINAL"],
        "must_contain": [],
        "expected_pages_min": 1,
        "expected_tables_min": 0,
        "source_url": "camelot sample / related",
        "license_note": "Third-party sample used in open-source tests; verify before shipping",
    },
]


def register_real_pdfs() -> list[dict]:
    registered = []
    for spec in REAL_SPECS:
        src = DL / spec["src"]
        if not src.exists() or src.stat().st_size < 1000:
            print(f"SKIP missing {spec['src']}")
            continue
        dest = REAL / f"{spec['id']}.pdf"
        shutil.copy2(src, dest)
        # page count via pypdf
        try:
            from pypdf import PdfReader

            pages = len(PdfReader(str(dest)).pages)
        except Exception:
            pages = None
        gt = {
            "category": spec["category"],
            "tier": "real",
            "description": spec["description"],
            "challenges": spec["challenges"],
            "source": "real",
            "source_file": spec["src"],
            "source_url": spec.get("source_url"),
            "license_note": spec.get("license_note"),
            "path": f"corpus/real/{spec['id']}.pdf",
            "must_contain": spec.get("must_contain") or [],
            "must_contain_any": spec.get("must_contain_any") or [],
            "table_cells_must_include": spec.get("table_cells_must_include") or [],
            "expected_tables_min": spec.get("expected_tables_min", 0),
            "expected_pages_min": spec.get("expected_pages_min", 1),
            "page_count_observed": pages,
            "our_library_priority": "stress real-world",
        }
        write_gt(spec["id"], gt)
        registered.append(spec["id"])
        print(f"REAL {spec['id']} pages={pages}")
    return registered


def rebuild_manifest() -> None:
    items = []
    for p in sorted(GT.glob("*.json")):
        items.append(json.loads(p.read_text(encoding="utf-8")))
    # also keep basic fixtures if present
    def _tier(i):
        t = i.get("tier")
        if t:
            return str(t)
        did = str(i.get("id", ""))
        if did.startswith("0"):
            return "basic"
        if did.startswith("2"):
            return "stress"
        if did.startswith(("3", "4")):
            return "real"
        if did.startswith(("5", "6")):
            return "hard"
        return "unknown"

    tiers = {}
    for i in items:
        t = _tier(i)
        tiers[t] = tiers.get(t, 0) + 1
    manifest = {
        "name": "pdfparser-competitive-corpus-v3",
        "description": (
            "Basic + stress + hard synthetic regression corpus, plus optional real PDFs. "
            "ICDAR is not included (competitive-only, external)."
        ),
        "document_count": len(items),
        "tiers": tiers,
        "suites_file": "corpus/suites.json",
        "documents": items,
    }
    (CORPUS / "manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    (SOURCES).write_text(
        json.dumps(
            {
                "real_pdfs": REAL_SPECS,
                "downloads_dir": "benchmark/downloads",
                "notes": "Real PDFs copied into corpus/real; do not commit if license uncertain for your use.",
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    print(f"Manifest: {len(items)} documents")


def main() -> None:
    print("Generating stress synthetics...")
    gen_bank_statement()
    gen_overflow_cells()
    gen_merged_headers()
    gen_side_by_side_tables()
    gen_invoice_complex()
    gen_dense_numeric()
    gen_watermark_overlap()
    gen_footnote_interrupted()
    print("Registering real PDFs...")
    register_real_pdfs()
    # Tag basic fixtures if untagged
    for p in GT.glob("0*.json"):
        data = json.loads(p.read_text(encoding="utf-8"))
        if "tier" not in data:
            data["tier"] = "basic"
            data["path"] = data.get("path") or f"corpus/{data['id']}.pdf"
            data["challenges"] = data.get("challenges") or [data.get("category", "basic")]
            p.write_text(json.dumps(data, indent=2), encoding="utf-8")
    rebuild_manifest()
    print("Done.")


if __name__ == "__main__":
    main()
