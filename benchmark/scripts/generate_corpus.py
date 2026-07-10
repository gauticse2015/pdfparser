#!/usr/bin/env python3
"""Generate a multi-scenario PDF corpus for competitive benchmarking.

Scenarios (aligned with pdfparser product design targets):
  - simple_text: single-column digital text
  - multi_column: two-column layout (reading-order stress)
  - large_multipage: many pages of digital text
  - image_heavy: embedded raster images + captions
  - special_objects: annotations/links + form fields + outline
  - table_lattice: fully ruled table (borders)
  - table_stream: whitespace-aligned columns, no rulings
  - table_partial_border: hybrid partial lines
  - table_complex: multi-header / wider financial-style grid
  - mixed_document: text + image + lattice table on one page
  - rotated_page: page /Rotate 90 content
  - encrypted_password: password-protected (open should fail or need password)

Each fixture writes:
  corpus/<id>.pdf
  ground_truth/<id>.json  (expected tokens, page_count, table dims, image counts, etc.)
"""
from __future__ import annotations

import json
import math
from pathlib import Path

from PIL import Image, ImageDraw
from reportlab.lib import colors
from reportlab.lib.pagesizes import letter
from reportlab.lib.styles import getSampleStyleSheet
from reportlab.lib.units import inch
from reportlab.pdfgen import canvas
from reportlab.platypus import (
    SimpleDocTemplate,
    Paragraph,
    Spacer,
    Table,
    TableStyle,
    Image as RLImage,
    PageBreak,
)

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "corpus"
GT = ROOT / "ground_truth"
ASSETS = ROOT / "assets"
CORPUS.mkdir(parents=True, exist_ok=True)
GT.mkdir(parents=True, exist_ok=True)
ASSETS.mkdir(parents=True, exist_ok=True)


def write_gt(doc_id: str, data: dict) -> None:
    data = {"id": doc_id, **data}
    (GT / f"{doc_id}.json").write_text(json.dumps(data, indent=2), encoding="utf-8")


def make_sample_png(path: Path, label: str, size=(320, 200), color=(40, 100, 180)) -> Path:
    img = Image.new("RGB", size, color)
    d = ImageDraw.Draw(img)
    d.rectangle([10, 10, size[0] - 10, size[1] - 10], outline=(255, 255, 255), width=3)
    d.text((20, 20), label, fill=(255, 255, 255))
    path.parent.mkdir(parents=True, exist_ok=True)
    img.save(path)
    return path


# ─── 1. Simple text ─────────────────────────────────────────────────────────
def gen_simple_text() -> None:
    doc_id = "01_simple_text"
    path = CORPUS / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    w, h = letter
    c.setTitle("Simple Text Fixture")
    c.setAuthor("pdfparser-benchmark")
    c.setFont("Helvetica-Bold", 18)
    c.drawString(72, h - 72, "Simple Digital PDF")
    c.setFont("Helvetica", 12)
    paragraphs = [
        "This is a born-digital single-column PDF used for baseline text extraction.",
        "It contains the unique token SIMPLE_TOKEN_ALPHA and another SIMPLE_TOKEN_BETA.",
        "Numbers: 12345, currency $99.50, and a date 2026-07-10 should survive extraction.",
        "The quick brown fox jumps over the lazy dog. Pack my box with five dozen liquor jugs.",
    ]
    y = h - 110
    for p in paragraphs:
        c.drawString(72, y, p[:95])
        y -= 20
        if len(p) > 95:
            c.drawString(72, y, p[95:])
            y -= 20
    c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "simple_text",
            "description": "Single-page single-column digital text",
            "page_count": 1,
            "must_contain": [
                "SIMPLE_TOKEN_ALPHA",
                "SIMPLE_TOKEN_BETA",
                "12345",
                "99.50",
                "2026-07-10",
                "Simple Digital PDF",
            ],
            "expected_images": 0,
            "expected_tables": 0,
            "table_cells": None,
            "reading_order_sensitive": False,
            "our_library_priority": "P1/0.1",
        },
    )


# ─── 2. Multi-column ────────────────────────────────────────────────────────
def gen_multi_column() -> None:
    doc_id = "02_multi_column"
    path = CORPUS / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    w, h = letter
    c.setTitle("Multi Column Fixture")
    c.setFont("Helvetica-Bold", 16)
    c.drawString(72, h - 60, "Two-Column Reading Order Stress")
    c.setFont("Helvetica", 10)
    left = [
        "LEFT_COL_START",
        "Left column paragraph one about finance.",
        "Left continues with LEFT_TOKEN_ONE.",
        "More left text to fill the column height.",
        "LEFT_COL_END",
    ]
    right = [
        "RIGHT_COL_START",
        "Right column paragraph one about markets.",
        "Right continues with RIGHT_TOKEN_TWO.",
        "More right text to fill the column height.",
        "RIGHT_COL_END",
    ]
    y_l = h - 100
    for line in left:
        c.drawString(72, y_l, line)
        y_l -= 16
    y_r = h - 100
    for line in right:
        c.drawString(320, y_r, line)
        y_r -= 16
    # vertical divider
    c.setStrokeColor(colors.grey)
    c.line(300, h - 90, 300, 200)
    c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "multi_column",
            "description": "Two-column text; reading order should prefer left then right",
            "page_count": 1,
            "must_contain": [
                "LEFT_COL_START",
                "LEFT_TOKEN_ONE",
                "LEFT_COL_END",
                "RIGHT_COL_START",
                "RIGHT_TOKEN_TWO",
                "RIGHT_COL_END",
            ],
            "reading_order_sensitive": True,
            "ideal_order_substrings": [
                "LEFT_COL_START",
                "LEFT_COL_END",
                "RIGHT_COL_START",
                "RIGHT_COL_END",
            ],
            "expected_images": 0,
            "expected_tables": 0,
            "our_library_priority": "P1/0.1 layout",
        },
    )


# ─── 3. Large multipage ─────────────────────────────────────────────────────
def gen_large_multipage(pages: int = 80) -> None:
    doc_id = "03_large_multipage"
    path = CORPUS / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    w, h = letter
    c.setTitle(f"Large Multipage ({pages} pages)")
    for i in range(1, pages + 1):
        c.setFont("Helvetica-Bold", 14)
        c.drawString(72, h - 72, f"Large Document Page {i} of {pages}")
        c.setFont("Helvetica", 11)
        c.drawString(72, h - 100, f"PAGE_TOKEN_{i:04d}")
        y = h - 130
        for j in range(25):
            c.drawString(
                72,
                y,
                f"Line {j+1} on page {i}: lorem ipsum dolor sit amet #{i}-{j} LARGE_BODY_TOKEN",
            )
            y -= 18
        c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "large_multipage",
            "description": f"{pages}-page digital text for throughput and memory",
            "page_count": pages,
            "must_contain": ["PAGE_TOKEN_0001", f"PAGE_TOKEN_{pages:04d}", "LARGE_BODY_TOKEN"],
            "expected_images": 0,
            "expected_tables": 0,
            "our_library_priority": "P0/P1 perf",
        },
    )


# ─── 4. Image heavy ─────────────────────────────────────────────────────────
def gen_image_heavy() -> None:
    doc_id = "04_image_heavy"
    path = CORPUS / f"{doc_id}.pdf"
    imgs = []
    for i in range(1, 5):
        p = ASSETS / f"img_{i}.png"
        make_sample_png(p, f"IMG_{i}", color=(30 + i * 40, 80, 120 + i * 20))
        imgs.append(p)
    c = canvas.Canvas(str(path), pagesize=letter)
    w, h = letter
    c.setTitle("Image Heavy Fixture")
    c.setFont("Helvetica-Bold", 16)
    c.drawString(72, h - 60, "Image-Heavy Digital PDF")
    c.setFont("Helvetica", 11)
    c.drawString(72, h - 85, "Caption text IMAGE_HEAVY_TOKEN and four embedded PNGs.")
    positions = [(72, 420), (320, 420), (72, 180), (320, 180)]
    for img_path, (x, y) in zip(imgs, positions):
        c.drawImage(str(img_path), x, y, width=200, height=140, preserveAspectRatio=True)
        c.setFont("Helvetica", 9)
        c.drawString(x, y - 14, f"Caption for {img_path.stem}")
    c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "image_heavy",
            "description": "Four embedded raster images with captions",
            "page_count": 1,
            "must_contain": ["IMAGE_HEAVY_TOKEN", "Image-Heavy Digital PDF", "Caption for img_1"],
            "expected_images": 4,
            "expected_tables": 0,
            "our_library_priority": "P1/0.1 images encoded",
        },
    )


# ─── 5. Special objects (links, form, outline) ───────────────────────────────
def gen_special_objects() -> None:
    doc_id = "05_special_objects"
    path = CORPUS / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    w, h = letter
    c.setTitle("Special Objects Fixture")
    c.setAuthor("benchmark")
    c.setFont("Helvetica-Bold", 16)
    c.drawString(72, h - 72, "Special Objects: Links, Forms, Outline")
    c.setFont("Helvetica", 12)
    c.drawString(72, h - 110, "Body token SPECIAL_OBJ_TOKEN")
    # URI link annotation
    c.setFillColor(colors.blue)
    c.drawString(72, h - 140, "Visit https://example.com/pdfparser-bench")
    c.linkURL(
        "https://example.com/pdfparser-bench",
        (72, h - 150, 400, h - 125),
        relative=0,
    )
    c.setFillColor(colors.black)
    # Form field
    c.acroForm.textfield(
        name="customer_name",
        tooltip="Customer name",
        x=72,
        y=h - 220,
        width=250,
        height=20,
        borderWidth=1,
        borderColor=colors.black,
        fillColor=colors.white,
        textColor=colors.black,
        forceBorder=True,
        value="FORM_FIELD_VALUE_ALPHA",
    )
    c.drawString(72, h - 190, "Form field label: customer_name")
    c.acroForm.checkbox(
        name="agree_terms",
        x=72,
        y=h - 260,
        size=14,
        checked=True,
        buttonStyle="check",
    )
    c.drawString(95, h - 255, "Agree to terms (checkbox)")
    # Outline / bookmarks
    c.bookmarkPage("sec1")
    c.addOutlineEntry("Section 1 Special Objects", "sec1", level=0)
    c.showPage()
    c.bookmarkPage("sec2")
    c.addOutlineEntry("Section 2 Continuation", "sec2", level=0)
    c.setFont("Helvetica", 12)
    c.drawString(72, h - 72, "Second page of special objects SPECIAL_PAGE2_TOKEN")
    c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "special_objects",
            "description": "URI link, AcroForm text+checkbox, outline bookmarks",
            "page_count": 2,
            "must_contain": ["SPECIAL_OBJ_TOKEN", "SPECIAL_PAGE2_TOKEN", "customer_name"],
            "expected_images": 0,
            "expected_tables": 0,
            "expected_form_fields": ["customer_name", "agree_terms"],
            "expected_link_uri_contains": "example.com/pdfparser-bench",
            "expected_outline_titles": ["Section 1 Special Objects", "Section 2 Continuation"],
            "our_library_priority": "P2 annots/forms/outline",
        },
    )


# ─── 6. Lattice table (full borders) ────────────────────────────────────────
def gen_table_lattice() -> None:
    doc_id = "06_table_lattice"
    path = CORPUS / f"{doc_id}.pdf"
    doc = SimpleDocTemplate(str(path), pagesize=letter)
    styles = getSampleStyleSheet()
    data = [
        ["SKU", "Product", "Qty", "Price"],
        ["A-100", "Widget", "10", "2.50"],
        ["B-200", "Gadget", "5", "9.99"],
        ["C-300", "Doohickey", "12", "1.25"],
        ["D-400", "Thingamajig", "3", "15.00"],
    ]
    t = Table(data, colWidths=[80, 140, 60, 70])
    t.setStyle(
        TableStyle(
            [
                ("GRID", (0, 0), (-1, -1), 1, colors.black),
                ("BACKGROUND", (0, 0), (-1, 0), colors.lightgrey),
                ("FONTNAME", (0, 0), (-1, 0), "Helvetica-Bold"),
                ("ALIGN", (2, 1), (-1, -1), "RIGHT"),
                ("FONTSIZE", (0, 0), (-1, -1), 11),
            ]
        )
    )
    story = [
        Paragraph("Lattice / Ruled Table Fixture", styles["Title"]),
        Spacer(1, 12),
        Paragraph("Token LATTICE_TABLE_TOKEN appears above a fully bordered table.", styles["Normal"]),
        Spacer(1, 18),
        t,
    ]
    doc.build(story)
    write_gt(
        doc_id,
        {
            "category": "table_lattice",
            "description": "Fully ruled 5x4 table with header row",
            "page_count": 1,
            "must_contain": [
                "LATTICE_TABLE_TOKEN",
                "SKU",
                "Widget",
                "Thingamajig",
                "15.00",
                "A-100",
            ],
            "expected_tables": 1,
            "table_shape": [5, 4],
            "table_method_hint": "lattice",
            "table_cells_must_include": ["SKU", "Product", "A-100", "Widget", "D-400", "15.00"],
            "our_library_priority": "P2 lattice tables",
        },
    )


# ─── 7. Stream table (no borders) ───────────────────────────────────────────
def gen_table_stream() -> None:
    doc_id = "07_table_stream"
    path = CORPUS / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    w, h = letter
    c.setTitle("Stream Table Fixture")
    c.setFont("Helvetica-Bold", 16)
    c.drawString(72, h - 72, "Stream / Whitespace Table (no rulings)")
    c.setFont("Helvetica", 11)
    c.drawString(72, h - 100, "Token STREAM_TABLE_TOKEN — columns aligned by spaces only.")
    # columns at fixed x positions
    headers = [("Name", 72), ("Role", 220), ("Office", 360), ("Salary", 480)]
    rows = [
        ("Alice", "Engineer", "NYC", "120000"),
        ("Bob", "Designer", "SF", "110000"),
        ("Carol", "PM", "Austin", "125000"),
        ("Dave", "Analyst", "Boston", "95000"),
        ("Eve", "SRE", "Seattle", "130000"),
    ]
    c.setFont("Helvetica-Bold", 11)
    y = h - 140
    for text, x in headers:
        c.drawString(x, y, text)
    c.setFont("Helvetica", 11)
    y -= 22
    for row in rows:
        for text, (_, x) in zip(row, headers):
            c.drawString(x, y, text)
        y -= 20
    c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "table_stream",
            "description": "Whitespace-aligned columns without vector borders",
            "page_count": 1,
            "must_contain": [
                "STREAM_TABLE_TOKEN",
                "Alice",
                "Engineer",
                "Seattle",
                "130000",
                "Name",
            ],
            "expected_tables": 1,
            "table_shape": [6, 4],
            "table_method_hint": "stream",
            "table_cells_must_include": ["Name", "Alice", "Eve", "130000", "Salary"],
            "our_library_priority": "P2 stream tables",
        },
    )


# ─── 8. Partial border hybrid ───────────────────────────────────────────────
def gen_table_partial_border() -> None:
    doc_id = "08_table_partial_border"
    path = CORPUS / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    w, h = letter
    c.setTitle("Partial Border Hybrid Table")
    c.setFont("Helvetica-Bold", 16)
    c.drawString(72, h - 72, "Partial Border / Hybrid Table")
    c.setFont("Helvetica", 11)
    c.drawString(72, h - 100, "Token HYBRID_TABLE_TOKEN — only outer box + header underline.")
    # outer rectangle
    x0, y0, x1, y1 = 70, 380, 520, 560
    c.rect(x0, y0, x1 - x0, y1 - y0, stroke=1, fill=0)
    # header underline only
    c.line(x0, 530, x1, 530)
    headers = ["Region", "Q1", "Q2", "Q3", "Q4"]
    rows = [
        ["North", "10", "12", "11", "15"],
        ["South", "8", "9", "10", "12"],
        ["East", "14", "13", "16", "18"],
        ["West", "7", "8", "9", "11"],
    ]
    xs = [80, 180, 260, 340, 420]
    c.setFont("Helvetica-Bold", 11)
    for text, x in zip(headers, xs):
        c.drawString(x, 540, text)
    c.setFont("Helvetica", 11)
    y = 510
    for row in rows:
        for text, x in zip(row, xs):
            c.drawString(x, y, text)
        y -= 28
    c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "table_partial_border",
            "description": "Outer border + header rule only (hybrid lattice/stream)",
            "page_count": 1,
            "must_contain": ["HYBRID_TABLE_TOKEN", "Region", "North", "West", "Q4", "18"],
            "expected_tables": 1,
            "table_shape": [5, 5],
            "table_method_hint": "hybrid",
            "table_cells_must_include": ["Region", "North", "West", "18"],
            "our_library_priority": "P2 hybrid tables",
        },
    )


# ─── 9. Complex financial-style table ───────────────────────────────────────
def gen_table_complex() -> None:
    doc_id = "09_table_complex"
    path = CORPUS / f"{doc_id}.pdf"
    doc = SimpleDocTemplate(str(path), pagesize=letter, leftMargin=40, rightMargin=40)
    styles = getSampleStyleSheet()
    header = [
        "Metric",
        "FY2023",
        "FY2024",
        "YoY%",
        "Notes",
    ]
    rows = [
        header,
        ["Revenue", "1,200,000", "1,450,000", "20.8%", "Organic + M&A"],
        ["COGS", "480,000", "560,000", "16.7%", "Supply costs"],
        ["Gross Profit", "720,000", "890,000", "23.6%", ""],
        ["OpEx", "400,000", "470,000", "17.5%", "Hiring"],
        ["EBITDA", "320,000", "420,000", "31.3%", "COMPLEX_TABLE_TOKEN"],
        ["Net Income", "210,000", "290,000", "38.1%", "Tax rate 21%"],
    ]
    t = Table(rows, colWidths=[100, 90, 90, 60, 160])
    t.setStyle(
        TableStyle(
            [
                ("GRID", (0, 0), (-1, -1), 0.5, colors.black),
                ("BACKGROUND", (0, 0), (-1, 0), colors.Color(0.85, 0.9, 0.95)),
                ("BACKGROUND", (0, 3), (-1, 3), colors.Color(0.95, 0.95, 0.95)),
                ("FONTNAME", (0, 0), (-1, 0), "Helvetica-Bold"),
                ("FONTNAME", (0, 0), (0, -1), "Helvetica-Bold"),
                ("ALIGN", (1, 1), (3, -1), "RIGHT"),
                ("FONTSIZE", (0, 0), (-1, -1), 9),
                ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
            ]
        )
    )
    story = [
        Paragraph("Complex Financial Grid", styles["Title"]),
        Spacer(1, 10),
        Paragraph(
            "Multi-row financial statement style table with numeric formatting and notes column.",
            styles["Normal"],
        ),
        Spacer(1, 16),
        t,
    ]
    doc.build(story)
    write_gt(
        doc_id,
        {
            "category": "table_complex",
            "description": "7x5 financial lattice with commas, percents, notes",
            "page_count": 1,
            "must_contain": [
                "COMPLEX_TABLE_TOKEN",
                "Revenue",
                "EBITDA",
                "1,450,000",
                "38.1%",
                "Net Income",
            ],
            "expected_tables": 1,
            "table_shape": [7, 5],
            "table_method_hint": "lattice",
            "table_cells_must_include": ["Revenue", "EBITDA", "Net Income", "1,450,000", "38.1%"],
            "our_library_priority": "P2 tables fidelity",
        },
    )


# ─── 10. Mixed document ─────────────────────────────────────────────────────
def gen_mixed() -> None:
    doc_id = "10_mixed_document"
    path = CORPUS / f"{doc_id}.pdf"
    img = make_sample_png(ASSETS / "mixed_chart.png", "CHART", size=(280, 160), color=(20, 120, 90))
    doc = SimpleDocTemplate(str(path), pagesize=letter)
    styles = getSampleStyleSheet()
    data = [
        ["Item", "Count"],
        ["Apples", "12"],
        ["Oranges", "8"],
        ["Bananas", "15"],
    ]
    t = Table(data, colWidths=[120, 80])
    t.setStyle(
        TableStyle(
            [
                ("GRID", (0, 0), (-1, -1), 1, colors.black),
                ("BACKGROUND", (0, 0), (-1, 0), colors.lightgrey),
            ]
        )
    )
    story = [
        Paragraph("Mixed Document Fixture", styles["Title"]),
        Paragraph("Intro paragraph with MIXED_DOC_TOKEN before figure and table.", styles["Normal"]),
        Spacer(1, 12),
        RLImage(str(img), width=280, height=160),
        Spacer(1, 8),
        Paragraph("Figure 1: sample chart image.", styles["Normal"]),
        Spacer(1, 16),
        t,
        Spacer(1, 12),
        Paragraph("Closing remarks MIXED_END_TOKEN after the table.", styles["Normal"]),
    ]
    doc.build(story)
    write_gt(
        doc_id,
        {
            "category": "mixed_document",
            "description": "Text + image + small lattice table on one page",
            "page_count": 1,
            "must_contain": ["MIXED_DOC_TOKEN", "MIXED_END_TOKEN", "Apples", "Bananas", "15"],
            "expected_images": 1,
            "expected_tables": 1,
            "table_shape": [4, 2],
            "table_cells_must_include": ["Item", "Apples", "Bananas", "15"],
            "our_library_priority": "P1+P2 integration",
        },
    )


# ─── 11. Rotated page ───────────────────────────────────────────────────────
def gen_rotated() -> None:
    doc_id = "11_rotated_page"
    path = CORPUS / f"{doc_id}.pdf"
    c = canvas.Canvas(str(path), pagesize=letter)
    w, h = letter
    c.setTitle("Rotated Page Fixture")
    # Page 1 normal
    c.setFont("Helvetica", 14)
    c.drawString(72, h - 72, "Normal page before rotation NORMAL_PAGE_TOKEN")
    c.showPage()
    # Page 2 rotated 90
    c.setPageRotation(90)
    c.setFont("Helvetica-Bold", 14)
    c.drawString(72, 72, "Rotated page content ROTATED_PAGE_TOKEN")
    c.setFont("Helvetica", 11)
    c.drawString(72, 100, "This page has /Rotate 90 in the page dictionary.")
    c.showPage()
    c.save()
    write_gt(
        doc_id,
        {
            "category": "rotated_page",
            "description": "Second page has /Rotate 90",
            "page_count": 2,
            "must_contain": ["NORMAL_PAGE_TOKEN", "ROTATED_PAGE_TOKEN"],
            "page_rotations": [0, 90],
            "expected_images": 0,
            "expected_tables": 0,
            "our_library_priority": "P1 coordinate spaces",
        },
    )


# ─── 12. Encrypted ──────────────────────────────────────────────────────────
def gen_encrypted() -> None:
    doc_id = "12_encrypted_password"
    path = CORPUS / f"{doc_id}.pdf"
    # Create plain then encrypt with pypdf
    tmp = ASSETS / "_plain_enc.pdf"
    c = canvas.Canvas(str(tmp), pagesize=letter)
    w, h = letter
    c.setFont("Helvetica", 14)
    c.drawString(72, h - 72, "Secret content ENCRYPT_SECRET_TOKEN")
    c.showPage()
    c.save()
    from pypdf import PdfReader, PdfWriter

    reader = PdfReader(str(tmp))
    writer = PdfWriter()
    writer.append(reader)
    writer.encrypt(user_password="benchpass", owner_password="ownerpass", algorithm="AES-256")
    with open(path, "wb") as f:
        writer.write(f)
    write_gt(
        doc_id,
        {
            "category": "encrypted_password",
            "description": "AES encrypted PDF; password 'benchpass'",
            "page_count": 1,
            "must_contain": ["ENCRYPT_SECRET_TOKEN"],  # only if password provided
            "password": "benchpass",
            "expected_open_without_password": "error_or_empty",
            "expected_images": 0,
            "expected_tables": 0,
            "our_library_priority": "0.1: Error::Encryption; 0.2: decrypt subset",
        },
    )


def gen_manifest() -> None:
    items = []
    for p in sorted(GT.glob("*.json")):
        items.append(json.loads(p.read_text(encoding="utf-8")))
    manifest = {
        "name": "pdfparser-competitive-corpus-v1",
        "description": "Synthetic multi-scenario corpus for market analysis of PDF parsers",
        "document_count": len(items),
        "documents": items,
    }
    (CORPUS / "manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    print(f"Wrote {len(items)} fixtures to {CORPUS}")


def main() -> None:
    gen_simple_text()
    gen_multi_column()
    gen_large_multipage(80)
    gen_image_heavy()
    gen_special_objects()
    gen_table_lattice()
    gen_table_stream()
    gen_table_partial_border()
    gen_table_complex()
    gen_mixed()
    gen_rotated()
    gen_encrypted()
    gen_manifest()


if __name__ == "__main__":
    main()
