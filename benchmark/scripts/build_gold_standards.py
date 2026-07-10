#!/usr/bin/env python3
"""Enrich ground_truth/*.json with quantitative gold for accuracy scoring.

Synthetic docs get full reference_text + expected_tables grids where possible.
Real docs get expected_table_count ranges / mins, image counts, and soft probes.
"""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GT = ROOT / "ground_truth"


def load(doc_id: str) -> dict:
    p = GT / f"{doc_id}.json"
    return json.loads(p.read_text(encoding="utf-8"))


def save(doc_id: str, data: dict) -> None:
    (GT / f"{doc_id}.json").write_text(json.dumps(data, indent=2), encoding="utf-8")


def merge(doc_id: str, **kwargs) -> None:
    d = load(doc_id)
    d.update(kwargs)
    save(doc_id, d)
    print(f"gold {doc_id}")


def main() -> None:
    # ── basic synthetic ────────────────────────────────────────────
    merge(
        "01_simple_text",
        reference_text=(
            "Simple Digital PDF\n"
            "This is a born-digital single-column PDF used for baseline text extraction.\n"
            "It contains the unique token SIMPLE_TOKEN_ALPHA and another SIMPLE_TOKEN_BETA.\n"
            "Numbers: 12345, currency $99.50, and a date 2026-07-10 should survive extraction.\n"
            "The quick brown fox jumps over the lazy dog. Pack my box with five dozen liquor jugs."
        ),
        expected_table_count=0,
        expected_images=0,
        weight_text=0.8,
        weight_tables=0.0,
        weight_objects=0.2,
    )

    merge(
        "02_multi_column",
        reference_text=(
            "Two-Column Reading Order Stress\n"
            "LEFT_COL_START\nLeft column paragraph one about finance.\n"
            "Left continues with LEFT_TOKEN_ONE.\nMore left text to fill the column height.\nLEFT_COL_END\n"
            "RIGHT_COL_START\nRight column paragraph one about markets.\n"
            "Right continues with RIGHT_TOKEN_TWO.\nMore right text to fill the column height.\nRIGHT_COL_END"
        ),
        expected_table_count=0,
        expected_images=0,
        weight_text=1.0,
        weight_tables=0.0,
        weight_objects=0.0,
    )

    merge(
        "03_large_multipage",
        # reference_text omitted (huge); token metrics only
        expected_table_count=0,
        expected_images=0,
        weight_text=1.0,
        weight_tables=0.0,
        weight_objects=0.0,
    )

    merge(
        "04_image_heavy",
        expected_table_count=0,
        expected_images=4,
        weight_text=0.4,
        weight_tables=0.0,
        weight_objects=0.6,
    )

    merge(
        "05_special_objects",
        expected_table_count=0,
        expected_images=0,
        expected_links=["https://example.com/pdfparser-bench"],
        expected_form_fields=["customer_name", "agree_terms"],
        expected_outline_titles=["Section 1 Special Objects", "Section 2 Continuation"],
        weight_text=0.3,
        weight_tables=0.0,
        weight_objects=0.7,
    )

    lattice_cells = [
        ["SKU", "Product", "Qty", "Price"],
        ["A-100", "Widget", "10", "2.50"],
        ["B-200", "Gadget", "5", "9.99"],
        ["C-300", "Doohickey", "12", "1.25"],
        ["D-400", "Thingamajig", "3", "15.00"],
    ]
    merge(
        "06_table_lattice",
        expected_table_count=1,
        expected_tables=[{"rows": 5, "cols": 4, "cells": lattice_cells}],
        expected_images=0,
        weight_text=0.25,
        weight_tables=0.75,
        weight_objects=0.0,
    )

    stream_cells = [
        ["Name", "Role", "Office", "Salary"],
        ["Alice", "Engineer", "NYC", "120000"],
        ["Bob", "Designer", "SF", "110000"],
        ["Carol", "PM", "Austin", "125000"],
        ["Dave", "Analyst", "Boston", "95000"],
        ["Eve", "SRE", "Seattle", "130000"],
    ]
    merge(
        "07_table_stream",
        expected_table_count=1,
        expected_tables=[{"rows": 6, "cols": 4, "cells": stream_cells}],
        expected_images=0,
        weight_text=0.25,
        weight_tables=0.75,
        weight_objects=0.0,
    )

    hybrid_cells = [
        ["Region", "Q1", "Q2", "Q3", "Q4"],
        ["North", "10", "12", "11", "15"],
        ["South", "8", "9", "10", "12"],
        ["East", "14", "13", "16", "18"],
        ["West", "7", "8", "9", "11"],
    ]
    merge(
        "08_table_partial_border",
        expected_table_count=1,
        expected_tables=[{"rows": 5, "cols": 5, "cells": hybrid_cells}],
        expected_images=0,
        weight_text=0.25,
        weight_tables=0.75,
        weight_objects=0.0,
    )

    complex_cells = [
        ["Metric", "FY2023", "FY2024", "YoY%", "Notes"],
        ["Revenue", "1,200,000", "1,450,000", "20.8%", "Organic + M&A"],
        ["COGS", "480,000", "560,000", "16.7%", "Supply costs"],
        ["Gross Profit", "720,000", "890,000", "23.6%", ""],
        ["OpEx", "400,000", "470,000", "17.5%", "Hiring"],
        ["EBITDA", "320,000", "420,000", "31.3%", "COMPLEX_TABLE_TOKEN"],
        ["Net Income", "210,000", "290,000", "38.1%", "Tax rate 21%"],
    ]
    merge(
        "09_table_complex",
        expected_table_count=1,
        expected_tables=[{"rows": 7, "cols": 5, "cells": complex_cells}],
        expected_images=0,
        weight_text=0.2,
        weight_tables=0.8,
        weight_objects=0.0,
    )

    mixed_cells = [
        ["Item", "Count"],
        ["Apples", "12"],
        ["Oranges", "8"],
        ["Bananas", "15"],
    ]
    merge(
        "10_mixed_document",
        expected_table_count=1,
        expected_tables=[{"rows": 4, "cols": 2, "cells": mixed_cells}],
        expected_images=1,
        weight_text=0.3,
        weight_tables=0.5,
        weight_objects=0.2,
    )

    merge(
        "11_rotated_page",
        expected_table_count=0,
        expected_images=0,
        weight_text=1.0,
        weight_tables=0.0,
        weight_objects=0.0,
    )

    merge(
        "12_encrypted_password",
        expected_table_count=0,
        expected_images=0,
        # when password provided
        weight_text=1.0,
        weight_tables=0.0,
        weight_objects=0.0,
    )

    # ── stress synthetic ───────────────────────────────────────────
    # Bank: logical 1 table but page-split often → tolerance
    bank_header = ["Date", "Description", "Type", "Amount", "Balance"]
    # we don't embed all 45 rows in gold (large); use expected count + content tokens
    # Prefer expected_table_count=1 with tolerance 1 (allow 1-2 page splits)
    merge(
        "20_bank_statement_multipage",
        expected_table_count=1,
        table_count_tolerance=1,  # 1 or 2 acceptable
        expected_images=0,
        # partial grid: header + first data row pattern checked via content tokens
        weight_text=0.3,
        weight_tables=0.7,
        weight_objects=0.0,
        accuracy_notes="Logical single ledger; page split into 2 tables scores detection within tolerance",
    )

    # Rebuild overflow gold grid programmatically matching generator
    overflow_rows = [["ID", "Short", "Overflowing Description", "Amt"]]
    for i in range(1, 12):
        if i in (3, 7, 10):
            mega = (
                f"TOKEN_OVERFLOW_MEGA{'' if i==3 else i} This cell contains an extremely long"
                if i != 3
                else "TOKEN_OVERFLOW_MEGA This cell contains an extremely long"
            )
            # use must-contain tokens rather than full mega string for grid
            desc = "TOKEN_OVERFLOW_MEGA" if i == 3 else f"TOKEN_OVERFLOW_MEGA{i}"
        else:
            desc = f"TOKEN_OVERFLOW_ROW_{i}"
        overflow_rows.append([f"R{i:02d}", "OK" if i % 2 else "LONGSHORTLABEL_TOKEN", desc, f"{i * 99.99:.2f}"])
    overflow_rows.append(["R99", "X", "TOKEN_OVERFLOW_MULTILINE", "0.01"])
    merge(
        "21_table_overflow_cells",
        expected_table_count=1,
        expected_tables=[{"rows": 13, "cols": 4, "cells": overflow_rows}],
        expected_images=0,
        weight_text=0.2,
        weight_tables=0.8,
        weight_objects=0.0,
    )

    merge_cells = [
        ["Metric", "FY2024", "", "FY2025", "", "Notes"],
        ["", "Actual", "Budget", "Actual", "Budget", ""],
        ["Revenue", "1,200", "1,100", "1,450", "1,400", "TOKEN_MERGE_REV organic growth"],
        ["COGS", "480", "500", "560", "550", "supply"],
        ["Gross Profit", "720", "600", "890", "850", "TOKEN_MERGE_GP"],
        [
            "Operating Expenses — Sales & Marketing plus G&A combined line that wraps",
            "400",
            "420",
            "470",
            "460",
            "hiring TOKEN_MERGE_OPEX",
        ],
        ["EBITDA", "320", "180", "420", "390", "TOKEN_MERGE_EBITDA"],
    ]
    merge(
        "22_table_merged_headers",
        expected_table_count=1,
        expected_tables=[{"rows": 7, "cols": 6, "cells": merge_cells}],
        expected_images=0,
        weight_text=0.2,
        weight_tables=0.8,
        weight_objects=0.0,
    )

    side_left = [
        ["City", "Temp"],
        ["NYC TOKEN_SIDE_L", "72"],
        ["SF", "68"],
        ["CHI", "70"],
    ]
    side_right = [
        ["City", "Pop(M)"],
        ["NYC TOKEN_SIDE_R", "8.3"],
        ["SF", "0.8"],
        ["CHI", "2.7"],
        ["Extra row only on right", "—"],
    ]
    merge(
        "23_side_by_side_tables",
        expected_table_count=2,
        expected_tables=[
            {"rows": 4, "cols": 2, "cells": side_left},
            {"rows": 5, "cols": 2, "cells": side_right},
        ],
        expected_images=0,
        weight_text=0.2,
        weight_tables=0.8,
        weight_objects=0.0,
    )

    inv_cells = [
        ["#", "SKU", "Description", "Qty", "Unit", "Total"],
        ["1", "SKU-101", "TOKEN_INV_SVC", "1", "15000.00", "15000.00"],
        ["2", "SKU-102", "TOKEN_INV_CLOUD", "12480", "0.04", "524.16"],
        ["3", "SKU-103", "TOKEN_INV_SUPPORT", "1", "4500.00", "4500.00"],
        ["4", "SKU-104", "TOKEN_INV_TRAVEL", "1", "2187.55", "2187.55"],
        # totals rows are messy; keep body gold only emphasis via content tokens
    ]
    merge(
        "24_invoice_line_items",
        expected_table_count=1,
        expected_tables=[{"rows": 5, "cols": 6, "cells": inv_cells}],
        table_count_tolerance=0,
        expected_images=0,
        weight_text=0.25,
        weight_tables=0.75,
        weight_objects=0.0,
        accuracy_notes="Gold grid is line-item body (5x6); totals span rows may expand predicted shape",
    )

    # dense: header + check shape 31x12
    dense_header = [f"C{c}" for c in range(12)]
    dense_rows = [dense_header]
    for r in range(30):
        row = []
        for c in range(12):
            val = (r + 1) * 100 + c + (0.01 * ((r * 12 + c) % 97))
            token = " TOKEN_DENSE" if r == 15 and c == 6 else ""
            row.append(f"{val:,.2f}{token}".strip())
        dense_rows.append(row)
    merge(
        "25_dense_numeric_grid",
        expected_table_count=1,
        expected_tables=[{"rows": 31, "cols": 12, "cells": dense_rows}],
        expected_images=0,
        weight_text=0.15,
        weight_tables=0.85,
        weight_objects=0.0,
    )

    merge(
        "26_watermark_overlap",
        expected_table_count=0,
        expected_images=0,
        weight_text=1.0,
        weight_tables=0.0,
        weight_objects=0.0,
    )

    fn_cells = [
        ["Country", "GDP", "Pop"],
        ["USA¹ TOKEN_FN_USA", "25.5", "331"],
        ["CHN²", "18.1", "1412"],
        ["DEU", "4.1", "83"],
        ["IND³ TOKEN_FN_IND", "3.4", "1408"],
    ]
    merge(
        "27_table_with_footnotes",
        expected_table_count=1,
        expected_tables=[{"rows": 5, "cols": 3, "cells": fn_cells}],
        expected_images=0,
        weight_text=0.3,
        weight_tables=0.7,
        weight_objects=0.0,
    )

    # ── real docs: weak gold (counts / probes only) ────────────────
    real_specs = {
        "30_real_ca_warn_report": dict(
            expected_tables_min=1,
            expected_table_count=None,  # unknown exact
            table_count_tolerance=0,
            # use min-only via expected_tables_min already in file
            expected_images=None,
            weight_text=0.4,
            weight_tables=0.6,
            weight_objects=0.0,
            accuracy_mode="weak_real",
        ),
        "31_real_background_checks": dict(
            expected_table_count=1,
            table_count_tolerance=0,
            weight_text=0.3,
            weight_tables=0.7,
            weight_objects=0.0,
            accuracy_mode="weak_real",
        ),
        "32_real_census_table324": dict(
            expected_table_count=1,
            table_count_tolerance=1,
            weight_text=0.3,
            weight_tables=0.7,
            weight_objects=0.0,
            accuracy_mode="weak_real",
        ),
        "33_real_argentina_votes": dict(
            expected_table_count=1,
            table_count_tolerance=1,
            weight_text=0.3,
            weight_tables=0.7,
            weight_objects=0.0,
            accuracy_mode="weak_real",
        ),
        "34_real_schools_contributions": dict(
            expected_tables_min=1,
            weight_text=0.4,
            weight_tables=0.6,
            weight_objects=0.0,
            accuracy_mode="weak_real",
        ),
        "35_real_camelot_fuel": dict(
            expected_table_count=1,
            table_count_tolerance=2,
            weight_text=0.3,
            weight_tables=0.7,
            weight_objects=0.0,
            accuracy_mode="weak_real",
        ),
        "36_real_two_tables": dict(
            expected_table_count=2,
            table_count_tolerance=0,
            weight_text=0.3,
            weight_tables=0.7,
            weight_objects=0.0,
            accuracy_mode="weak_real",
            accuracy_notes="Ideal ~2 tables; over-segmentation heavily penalized via detection precision",
        ),
        "37_real_liabilities_superscript": dict(
            expected_table_count=1,
            table_count_tolerance=0,
            weight_text=0.3,
            weight_tables=0.7,
            weight_objects=0.0,
            accuracy_mode="weak_real",
        ),
        "38_real_irs_f1040": dict(
            expected_table_count=0,  # form, not data tables — FPs penalized
            table_count_tolerance=0,
            weight_text=0.6,
            weight_tables=0.4,
            weight_objects=0.0,
            accuracy_mode="weak_real",
            accuracy_notes="Tax form: expected_table_count=0 to penalize false-positive tables",
        ),
        "39_real_fed_beigebook": dict(
            expected_table_count=0,
            table_count_tolerance=2,  # allow a couple incidental
            weight_text=0.9,
            weight_tables=0.1,
            weight_objects=0.0,
            accuracy_mode="weak_real",
        ),
        "40_real_arxiv_tensorflow": dict(
            expected_tables_min=0,
            expected_table_count=None,
            expected_images=None,
            weight_text=1.0,
            weight_tables=0.0,
            weight_objects=0.0,
            accuracy_mode="weak_real",
            accuracy_notes="Text-primary scientific paper; tables not gold-labeled",
        ),
        "41_real_nist_withdrawn_notice": dict(
            expected_table_count=0,
            table_count_tolerance=5,  # some tables may exist; heavy FP still hurts
            weight_text=0.7,
            weight_tables=0.3,
            weight_objects=0.0,
            accuracy_mode="weak_real",
            accuracy_notes="Penalize mass false-positive table detection",
        ),
        "42_real_insurance_italian": dict(
            expected_tables_min=0,
            weight_text=1.0,
            weight_tables=0.0,
            weight_objects=0.0,
            accuracy_mode="weak_real",
        ),
    }

    for doc_id, kw in real_specs.items():
        p = GT / f"{doc_id}.json"
        if p.exists():
            merge(doc_id, **kw)

    print("Done enriching gold standards")


if __name__ == "__main__":
    main()
