# PDF Parser Accuracy Scoreboard
**Generated:** auto · **Docs scored:** 33 · **Libraries:** pdfminer.six, pdfparser, pdfplumber, pymupdf, pypdf, pypdfium2

## Metric definitions
| Metric | Meaning | Range |
|--------|---------|-------|
| **overall_score** | Weighted blend of text / tables / objects (per-doc weights in GT) | 0–100 |
| **text_token_f1** | F1 on required substrings (`must_contain`) | 0–1 |
| **text_cer** | Character Error Rate vs `reference_text` (lower better) | 0–1+ |
| **table_detect_f1** | F1 on table *count* vs expected | 0–1 |
| **table_row/col_accuracy** | Fraction of gold tables with exact row/col counts | 0–1 |
| **table_cell_f1** | Micro F1 of normalized cell text after table alignment | 0–1 |
| **images/forms/links/outline** | Count or set accuracy vs gold | 0–1 |

Scores are only computed when gold exists for that component. Synthetic docs have full table grids; real docs often have count-only gold.

## Overall leaderboard (mean over successful runs)
| Library | overall | text | text F1 | CER↓ | table | detect F1 | cell F1 | objects | ms |
|---------|--------:|-----:|--------:|-----:|------:|----------:|--------:|--------:|---:|
| pymupdf | 84.481 | 99.242 | 0.992 | 0.000 | 69.700 | 0.710 | 0.774 | 100.000 | 387.6 |
| pdfplumber | 82.980 | 98.432 | 0.992 | 0.267 | 67.488 | 0.696 | 0.826 | 97.500 | 306.8 |
| pdfparser | 80.490 | 100.000 | 1.000 | 0.000 | 66.989 | 0.698 | 0.786 | 0.000 | 28.2 |
| pypdf | 56.515 | 100.000 | 1.000 | 0.000 | 39.394 | 0.394 | 0.000 | 100.000 | 71.0 |
| pypdfium2 | 53.194 | 99.179 | 0.992 | 0.021 | 39.394 | 0.394 | 0.000 | 0.000 | 12.8 |
| pdfminer.six | 53.097 | 97.638 | 0.984 | 0.279 | 39.394 | 0.394 | 0.000 | 97.500 | 361.5 |

## Grid-gold subset (synthetic tables with full cell grids)
Best apples-to-apples table quality comparison.

| Library | n | detect F1 | shape exact | cell F1 | table score |
|---------|--:|----------:|------------:|--------:|------------:|
| pymupdf | 11 | 0.818 | 0.727 | 0.774 | 77.786 |
| pdfplumber | 11 | 0.909 | 0.727 | 0.826 | 83.034 |
| pdfparser | 11 | 0.879 | 0.636 | 0.786 | 78.108 |
| pypdf | 11 | 0.000 | 0.000 | 0.000 | 0.000 |
| pypdfium2 | 11 | 0.000 | 0.000 | 0.000 | 0.000 |
| pdfminer.six | 11 | 0.000 | 0.000 | 0.000 | 0.000 |

## By tier

### basic

| Library | n | overall | text | table | detect F1 | cell F1 | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|--------:|
| pymupdf | 12 | 87.460 | 100.000 | 83.284 | 0.833 | 0.597 | 100.000 |
| pdfplumber | 12 | 84.913 | 97.772 | 86.744 | 0.917 | 0.630 | 95.833 |
| pdfparser | 11 | 82.790 | 100.000 | 85.539 | 0.909 | 0.630 | 0.000 |
| pypdf | 12 | 70.417 | 100.000 | 58.333 | 0.583 | 0.000 | 100.000 |
| pypdfium2 | 12 | 63.367 | 99.825 | 58.333 | 0.583 | 0.000 | 0.000 |
| pdfminer.six | 12 | 61.016 | 93.506 | 58.333 | 0.583 | 0.000 | 95.833 |

### stress

| Library | n | overall | text | table | detect F1 | cell F1 | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|--------:|
| pymupdf | 8 | 89.792 | 96.875 | 90.364 | 0.958 | 0.922 | 100.000 |
| pdfplumber | 8 | 92.915 | 96.875 | 94.611 | 0.958 | 0.989 | 100.000 |
| pdfparser | 8 | 90.621 | 100.000 | 87.838 | 0.917 | 0.916 | — |
| pypdf | 8 | 32.500 | 100.000 | 12.500 | 0.125 | 0.000 | 100.000 |
| pypdfium2 | 8 | 29.375 | 96.875 | 12.500 | 0.125 | 0.000 | — |
| pdfminer.six | 8 | 32.500 | 100.000 | 12.500 | 0.125 | 0.000 | 100.000 |

### real

| Library | n | overall | text | table | detect F1 | cell F1 | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|--------:|
| pymupdf | 13 | 78.462 | 100.000 | 44.444 | 0.444 | — | — |
| pdfplumber | 13 | 75.082 | 100.000 | 33.023 | 0.330 | — | — |
| pdfparser | 13 | 72.308 | 100.000 | 38.462 | 0.385 | — | — |
| pypdf | 13 | 58.462 | 100.000 | 38.462 | 0.385 | — | — |
| pypdfium2 | 13 | 58.462 | 100.000 | 38.462 | 0.385 | — | — |
| pdfminer.six | 13 | 58.462 | 100.000 | 38.462 | 0.385 | — | — |

## Per-document matrix (overall score)

| Doc | pdfminer.six | pdfparser | pdfplumber | pymupdf | pypdf | pypdfium2 |
|-----|-----:|-----:|-----:|-----:|-----:|-----:|
| `01_simple_text` | 99.5 | 100.0 | 100.0 | 100.0 | 100.0 | 99.4 |
| `02_multi_column` | 72.7 | 100.0 | 73.3 | 100.0 | 100.0 | 98.5 |
| `03_large_multipage` | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |
| `04_image_heavy` | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |
| `05_special_objects` | 65.0 | 30.0 | 65.0 | 100.0 | 100.0 | 30.0 |
| `06_table_lattice` | 25.0 | 100.0 | 100.0 | 100.0 | 25.0 | 25.0 |
| `07_table_stream` | 25.0 | 25.0 | 25.0 | 25.0 | 25.0 | 25.0 |
| `08_table_partial_border` | 25.0 | 55.7 | 55.7 | 25.0 | 25.0 | 25.0 |
| `09_table_complex` | 20.0 | 100.0 | 100.0 | 99.5 | 20.0 | 20.0 |
| `10_mixed_document` | 50.0 | 100.0 | 100.0 | 100.0 | 50.0 | 37.5 |
| `11_rotated_page` | 50.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |
| `12_encrypted_password` | 100.0 | FAIL | 100.0 | 100.0 | 100.0 | 100.0 |
| `20_bank_statement_multipage` | 30.0 | 89.1 | 89.1 | 76.7 | 30.0 | 30.0 |
| `21_table_overflow_cells` | 20.0 | 98.4 | 98.4 | 93.7 | 20.0 | 20.0 |
| `22_table_merged_headers` | 20.0 | 97.9 | 99.6 | 97.7 | 20.0 | 20.0 |
| `23_side_by_side_tables` | 20.0 | 58.4 | 100.0 | 98.1 | 20.0 | 20.0 |
| `24_invoice_line_items` | 25.0 | 81.2 | 81.2 | 79.1 | 25.0 | 25.0 |
| `25_dense_numeric_grid` | 15.0 | 100.0 | 100.0 | 100.0 | 15.0 | 15.0 |
| `26_watermark_overlap` | 100.0 | 100.0 | 75.0 | 75.0 | 100.0 | 75.0 |
| `27_table_with_footnotes` | 30.0 | 100.0 | 100.0 | 98.0 | 30.0 | 30.0 |
| `30_real_ca_warn_report` | 40.0 | 40.0 | 46.7 | 46.7 | 40.0 | 40.0 |
| `31_real_background_checks` | 30.0 | 30.0 | 100.0 | 100.0 | 30.0 | 30.0 |
| `32_real_census_table324` | 30.0 | 100.0 | 76.7 | 76.7 | 30.0 | 30.0 |
| `33_real_argentina_votes` | 30.0 | 100.0 | 100.0 | 76.7 | 30.0 | 30.0 |
| `34_real_schools_contributions` | 40.0 | 40.0 | 60.0 | 60.0 | 40.0 | 40.0 |
| `35_real_camelot_fuel` | 30.0 | 30.0 | 100.0 | 100.0 | 30.0 | 30.0 |
| `36_real_two_tables` | 30.0 | 30.0 | 42.7 | 100.0 | 30.0 | 30.0 |
| `37_real_liabilities_superscript` | 30.0 | 100.0 | 30.0 | 30.0 | 30.0 | 30.0 |
| `38_real_irs_f1040` | 100.0 | 100.0 | 60.0 | 60.0 | 100.0 | 100.0 |
| `39_real_fed_beigebook` | 100.0 | 100.0 | 90.0 | 100.0 | 100.0 | 100.0 |
| `40_real_arxiv_tensorflow` | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |
| `41_real_nist_withdrawn_notice` | 100.0 | 70.0 | 70.0 | 70.0 | 100.0 | 100.0 |
| `42_real_insurance_italian` | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |

## Per-document table cell F1 (grid gold only)

| Doc | pdfminer.six | pdfparser | pdfplumber | pymupdf | pypdf | pypdfium2 |
|-----|-----:|-----:|-----:|-----:|-----:|-----:|
| `06_table_lattice` | 0.000 | 1.000 | 1.000 | 1.000 | 0.000 | 0.000 |
| `07_table_stream` | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| `08_table_partial_border` | 0.000 | 0.148 | 0.148 | 0.000 | 0.000 | 0.000 |
| `09_table_complex` | 0.000 | 1.000 | 1.000 | 0.985 | 0.000 | 0.000 |
| `10_mixed_document` | 0.000 | 1.000 | 1.000 | 1.000 | 0.000 | 0.000 |
| `21_table_overflow_cells` | 0.000 | 0.950 | 0.950 | 0.805 | 0.000 | 0.000 |
| `22_table_merged_headers` | 0.000 | 0.933 | 0.987 | 0.930 | 0.000 | 0.000 |
| `23_side_by_side_tables` | 0.000 | 0.615 | 1.000 | 0.941 | 0.000 | 0.000 |
| `24_invoice_line_items` | 0.000 | 1.000 | 1.000 | 0.929 | 0.000 | 0.000 |
| `25_dense_numeric_grid` | 0.000 | 1.000 | 1.000 | 0.999 | 0.000 | 0.000 |
| `27_table_with_footnotes` | 0.000 | 1.000 | 1.000 | 0.929 | 0.000 | 0.000 |

## How to compare `pdfparser` later
1. Add an adapter returning the same extract fields.
2. Re-run `python benchmark/scripts/run_accuracy_benchmark.py`.
3. Read **overall_score**, **table_cell_f1** (grid subset), **table_detect_f1**, **text_cer**.
4. Target bars (from competitors on this harness):
   - text_token_f1 ≥ best competitor on basic/stress
   - table_cell_f1 ≥ pdfplumber on grid-gold subset
   - table_detect_f1 high on statements with multi-page tolerance
   - low false positives on IRS/NIST (detect F1 with expected 0)

---
*Machine-generated scoreboard. Re-run after corpus/gold changes.*
