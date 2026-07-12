# PDF Parser Accuracy Scoreboard

**Suite:** `regression_basic_stress` · **Docs:** 20 · **ICDAR excluded** (competitive-only)

## Executive summary — table quality

Primary ranking for table-engine work (mean over docs that produce table metrics):

| Rank | Library | cell F1 | detect F1 | shape exact | overall | ms |
|-----:|---------|--------:|----------:|------------:|--------:|---:|
| 1 | **pdfparser** ← **ours** | 0.950 | 0.947 | 0.909 | 98.126 | 10.6 |
| 2 | **camelot_lattice** | 0.826 | 0.933 | 0.727 | 46.889 | 35.5 |
| 3 | **pdfplumber** | 0.826 | 0.933 | 0.727 | 88.114 | 93.0 |
| 4 | **camelot_auto** | 0.826 | 0.883 | 0.727 | 54.350 | 338.4 |
| 5 | **pymupdf** | 0.774 | 0.883 | 0.727 | 88.393 | 111.2 |
| 6 | **img2table** | 0.722 | 0.877 | 0.727 | 44.405 | 69.8 |
| 7 | **camelot_stream** | 0.233 | 0.608 | 0.182 | 61.521 | 12.9 |
| 8 | **pdfminer.six** | 0.000 | 0.400 | 0.000 | 49.609 | 96.3 |
| 9 | **pypdf** | 0.000 | 0.400 | 0.000 | 55.250 | 9.8 |
| 10 | **pypdfium2** | 0.000 | 0.400 | 0.000 | 49.770 | 2.5 |

### Overall product score (text+tables+objects)

| Rank | Library | overall | text F1 | table cell F1 | ms |
|-----:|---------|--------:|--------:|--------------:|---:|
| 1 | **pdfparser** ← **ours** | 98.126 | 1.000 | 0.950 | 10.6 |
| 2 | **pymupdf** | 88.393 | 0.988 | 0.774 | 111.2 |
| 3 | **pdfplumber** | 88.114 | 0.988 | 0.826 | 93.0 |
| 4 | **camelot_stream** | 61.521 | 0.813 | 0.233 | 12.9 |
| 5 | **pypdf** | 55.250 | 1.000 | 0.000 | 9.8 |
| 6 | **camelot_auto** | 54.350 | 0.501 | 0.826 | 338.4 |
| 7 | **pypdfium2** | 49.770 | 0.988 | 0.000 | 2.5 |
| 8 | **pdfminer.six** | 49.609 | 0.975 | 0.000 | 96.3 |
| 9 | **camelot_lattice** | 46.889 | 0.389 | 0.826 | 35.5 |
| 10 | **img2table** | 44.405 | 0.365 | 0.722 | 69.8 |

> Table-primary tools (`camelot_*`, `img2table`) get lower **overall** when outside-table tokens are missing — judge them on **cell F1 / detect F1**.

---

**Generated:** auto · **Docs scored:** 20 · **Libraries:** camelot_auto, camelot_lattice, camelot_stream, img2table, pdfminer.six, pdfparser, pdfplumber, pymupdf, pypdf, pypdfium2

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
| pdfparser | 98.126 | 100.000 | 1.000 | 0.000 | 92.251 | 0.947 | 0.950 | 100.000 | 10.6 |
| pymupdf | 88.393 | 98.750 | 0.988 | 0.000 | 86.116 | 0.883 | 0.774 | 100.000 | 111.2 |
| pdfplumber | 88.114 | 97.413 | 0.988 | 0.267 | 89.891 | 0.933 | 0.826 | 97.500 | 93.0 |
| camelot_stream | 61.521 | 79.946 | 0.813 | 0.276 | 35.120 | 0.608 | 0.233 | 0.000 | 12.9 |
| pypdf | 55.250 | 100.000 | 1.000 | 0.000 | 40.000 | 0.400 | 0.000 | 100.000 | 9.8 |
| camelot_auto | 54.350 | 48.749 | 0.501 | 0.770 | 81.641 | 0.883 | 0.826 | 0.000 | 338.4 |
| pypdfium2 | 49.770 | 98.645 | 0.988 | 0.021 | 40.000 | 0.400 | 0.000 | 0.000 | 2.5 |
| pdfminer.six | 49.609 | 96.104 | 0.975 | 0.279 | 40.000 | 0.400 | 0.000 | 97.500 | 96.3 |
| camelot_lattice | 46.889 | 38.851 | 0.389 | 1.000 | 89.891 | 0.933 | 0.826 | 0.000 | 35.5 |
| img2table | 44.405 | 36.510 | 0.365 | 1.000 | 84.164 | 0.877 | 0.722 | 0.000 | 69.8 |

## Grid-gold subset (synthetic tables with full cell grids)
Best apples-to-apples table quality comparison.

| Library | n | detect F1 | shape exact | cell F1 | table score |
|---------|--:|----------:|------------:|--------:|------------:|
| pdfparser | 11 | 1.000 | 0.909 | 0.950 | 95.706 |
| pymupdf | 11 | 0.818 | 0.727 | 0.774 | 77.786 |
| pdfplumber | 11 | 0.909 | 0.727 | 0.826 | 83.034 |
| camelot_stream | 11 | 0.970 | 0.182 | 0.233 | 47.794 |
| pypdf | 11 | 0.000 | 0.000 | 0.000 | 0.000 |
| camelot_auto | 11 | 1.000 | 0.727 | 0.826 | 86.216 |
| pypdfium2 | 11 | 0.000 | 0.000 | 0.000 | 0.000 |
| pdfminer.six | 11 | 0.000 | 0.000 | 0.000 | 0.000 |
| camelot_lattice | 11 | 0.909 | 0.727 | 0.826 | 83.034 |
| img2table | 11 | 0.818 | 0.727 | 0.722 | 75.677 |

## By tier

### basic

| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|
| pdfparser | 11 | 100.000 | 100.000 | 100.000 | 1.000 | 1.000 | 1.000 | 100.000 |
| pymupdf | 12 | 87.460 | 100.000 | 83.284 | 0.833 | 0.597 | 0.600 | 100.000 |
| pdfplumber | 12 | 84.913 | 97.772 | 86.744 | 0.917 | 0.630 | 0.600 | 95.833 |
| camelot_stream | 12 | 69.476 | 84.642 | 33.750 | 0.500 | 0.400 | 0.400 | 0.000 |
| pypdf | 12 | 70.417 | 100.000 | 58.333 | 0.583 | 0.000 | 0.000 | 100.000 |
| camelot_auto | 12 | 38.048 | 41.636 | 81.327 | 0.917 | 0.630 | 0.600 | 0.000 |
| pypdfium2 | 12 | 63.367 | 99.825 | 58.333 | 0.583 | 0.000 | 0.000 | 0.000 |
| pdfminer.six | 12 | 61.016 | 93.506 | 58.333 | 0.583 | 0.000 | 0.000 | 95.833 |
| camelot_lattice | 12 | 27.697 | 27.222 | 86.744 | 0.917 | 0.630 | 0.600 | 0.000 |
| img2table | 11 | 25.488 | 22.121 | 81.766 | 0.818 | 0.597 | 0.600 | 0.000 |

### stress

| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|
| pdfparser | 8 | 95.549 | 100.000 | 81.596 | 0.875 | 0.907 | 0.833 | 100.000 |
| pymupdf | 8 | 89.792 | 96.875 | 90.364 | 0.958 | 0.922 | 0.833 | 100.000 |
| pdfplumber | 8 | 92.915 | 96.875 | 94.611 | 0.958 | 0.989 | 0.833 | 100.000 |
| camelot_stream | 8 | 49.590 | 72.902 | 37.175 | 0.771 | 0.093 | 0.000 | — |
| pypdf | 8 | 32.500 | 100.000 | 12.500 | 0.125 | 0.000 | 0.000 | 100.000 |
| camelot_auto | 8 | 78.803 | 59.420 | 82.111 | 0.833 | 0.989 | 0.833 | — |
| pypdfium2 | 8 | 29.375 | 96.875 | 12.500 | 0.125 | 0.000 | 0.000 | — |
| pdfminer.six | 8 | 32.500 | 100.000 | 12.500 | 0.125 | 0.000 | 0.000 | 100.000 |
| camelot_lattice | 8 | 75.678 | 56.295 | 94.611 | 0.958 | 0.989 | 0.833 | — |
| img2table | 8 | 70.415 | 56.295 | 87.462 | 0.958 | 0.825 | 0.833 | — |

### hard

| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|

### real

| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|

## Per-document matrix (overall score)

| Doc | camelot_auto | camelot_lattice | camelot_stream | img2table | pdfminer.six | pdfparser | pdfplumber | pymupdf | pypdf | pypdfium2 |
|-----|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|
| `01_simple_text` | 0.0 | 0.0 | 99.4 | 0.0 | 99.5 | 100.0 | 100.0 | 100.0 | 100.0 | 99.4 |
| `02_multi_column` | 73.0 | 0.0 | 73.0 | 0.0 | 72.7 | 100.0 | 73.3 | 100.0 | 100.0 | 98.5 |
| `03_large_multipage` | 0.0 | 0.0 | 0.0 | 0.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |
| `04_image_heavy` | 0.0 | 0.0 | 100.0 | 0.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |
| `05_special_objects` | 0.0 | 0.0 | 30.0 | 0.0 | 65.0 | 100.0 | 65.0 | 100.0 | 100.0 | 30.0 |
| `06_table_lattice` | 95.8 | 95.8 | 51.2 | 95.8 | 25.0 | 100.0 | 100.0 | 100.0 | 25.0 | 25.0 |
| `07_table_stream` | 51.2 | 0.0 | 51.2 | 0.0 | 25.0 | 100.0 | 25.0 | 25.0 | 25.0 | 25.0 |
| `08_table_partial_border` | 51.5 | 51.5 | 95.8 | 0.0 | 25.0 | 100.0 | 55.7 | 25.0 | 25.0 | 25.0 |
| `09_table_complex` | 100.0 | 100.0 | 48.0 | 99.5 | 20.0 | 100.0 | 100.0 | 99.5 | 20.0 | 20.0 |
| `10_mixed_document` | 85.0 | 85.0 | 85.0 | 85.0 | 50.0 | 100.0 | 100.0 | 100.0 | 50.0 | 37.5 |
| `11_rotated_page` | 0.0 | 0.0 | 100.0 | 0.0 | 50.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |
| `12_encrypted_password` | 0.0 | 0.0 | 100.0 | FAIL | 100.0 | FAIL | 100.0 | 100.0 | 100.0 | 100.0 |
| `20_bank_statement_multipage` | 72.0 | 72.0 | 79.4 | 59.5 | 30.0 | 100.0 | 89.1 | 76.7 | 30.0 | 30.0 |
| `21_table_overflow_cells` | 94.4 | 94.4 | 48.0 | 94.4 | 20.0 | 98.4 | 98.4 | 93.7 | 20.0 | 20.0 |
| `22_table_merged_headers` | 97.1 | 97.1 | 45.5 | 95.5 | 20.0 | 99.6 | 99.6 | 97.7 | 20.0 | 20.0 |
| `23_side_by_side_tables` | 95.0 | 95.0 | 51.6 | 95.0 | 20.0 | 99.1 | 100.0 | 98.1 | 20.0 | 20.0 |
| `24_invoice_line_items` | 68.8 | 68.8 | 38.8 | 40.7 | 25.0 | 67.3 | 81.2 | 79.1 | 25.0 | 25.0 |
| `25_dense_numeric_grid` | 96.2 | 96.2 | 41.0 | 96.2 | 15.0 | 100.0 | 100.0 | 100.0 | 15.0 | 15.0 |
| `26_watermark_overlap` | 25.0 | 0.0 | 50.0 | 0.0 | 100.0 | 100.0 | 75.0 | 75.0 | 100.0 | 75.0 |
| `27_table_with_footnotes` | 82.0 | 82.0 | 42.5 | 82.0 | 30.0 | 100.0 | 100.0 | 98.0 | 30.0 | 30.0 |

## Per-document table cell F1 (grid gold only)

| Doc | camelot_auto | camelot_lattice | camelot_stream | img2table | pdfminer.six | pdfparser | pdfplumber | pymupdf | pypdf | pypdfium2 |
|-----|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|
| `06_table_lattice` | 1.000 | 1.000 | 0.000 | 1.000 | 0.000 | 1.000 | 1.000 | 1.000 | 0.000 | 0.000 |
| `07_table_stream` | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 1.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| `08_table_partial_border` | 0.148 | 0.148 | 1.000 | 0.000 | 0.000 | 1.000 | 0.148 | 0.000 | 0.000 | 0.000 |
| `09_table_complex` | 1.000 | 1.000 | 0.000 | 0.986 | 0.000 | 1.000 | 1.000 | 0.985 | 0.000 | 0.000 |
| `10_mixed_document` | 1.000 | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 1.000 | 0.000 | 0.000 |
| `21_table_overflow_cells` | 0.950 | 0.950 | 0.000 | 0.950 | 0.000 | 0.950 | 0.950 | 0.805 | 0.000 | 0.000 |
| `22_table_merged_headers` | 0.987 | 0.987 | 0.000 | 0.937 | 0.000 | 0.987 | 0.987 | 0.930 | 0.000 | 0.000 |
| `23_side_by_side_tables` | 1.000 | 1.000 | 0.560 | 1.000 | 0.000 | 0.971 | 1.000 | 0.941 | 0.000 | 0.000 |
| `24_invoice_line_items` | 1.000 | 1.000 | 0.000 | 0.065 | 0.000 | 0.537 | 1.000 | 0.929 | 0.000 | 0.000 |
| `25_dense_numeric_grid` | 1.000 | 1.000 | 0.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.999 | 0.000 | 0.000 |
| `27_table_with_footnotes` | 1.000 | 1.000 | 0.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.929 | 0.000 | 0.000 |

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

## Run metadata

- **Suite:** `regression_basic_stress` (ICDAR never included in regression)
- **Documents scored:** 20
- **Libraries:** pdfparser, camelot_lattice, camelot_stream, camelot_auto, img2table, pdfplumber, pymupdf, pdfminer.six, pypdf, pypdfium2
- **Note:** `camelot_*` / `img2table` / `tabula` are table-primary; text scores use cell-concatenated text only (outside-table tokens may miss).
- **Tabula:** skipped when no working JRE is installed.

## Table-quality leaderboard (mean over docs with table gold)

| Rank | Library | detect F1 | shape exact | cell F1 | table score | ms |
|-----:|---------|----------:|------------:|--------:|------------:|---:|
| 1 | **pdfparser** | 0.947 | 0.909 | 0.950 | 92.251 | 10.6 |
| 2 | **camelot_lattice** | 0.933 | 0.727 | 0.826 | 89.891 | 35.5 |
| 3 | **pdfplumber** | 0.933 | 0.727 | 0.826 | 89.891 | 93.0 |
| 4 | **camelot_auto** | 0.883 | 0.727 | 0.826 | 81.641 | 338.4 |
| 5 | **pymupdf** | 0.883 | 0.727 | 0.774 | 86.116 | 111.2 |
| 6 | **img2table** | 0.877 | 0.727 | 0.722 | 84.164 | 69.8 |
| 7 | **camelot_stream** | 0.608 | 0.182 | 0.233 | 35.120 | 12.9 |
| 8 | **pdfminer.six** | 0.400 | 0.000 | 0.000 | 40.000 | 96.3 |
| 9 | **pypdf** | 0.400 | 0.000 | 0.000 | 40.000 | 9.8 |
| 10 | **pypdfium2** | 0.400 | 0.000 | 0.000 | 40.000 | 2.5 |

## Hard tier (structure stress 50–62)

| Library | n | detect F1 | shape exact | cell F1 | table score | overall |
|---------|--:|----------:|------------:|--------:|------------:|--------:|

## How to re-run

```bash
cargo build --release -p pdfparser-cli
source .venv/bin/activate
python benchmark/scripts/run_accuracy_benchmark.py --suite regression
python benchmark/scripts/run_accuracy_benchmark.py --suite regression_hard
```

Competitive ICDAR (external, not this scoreboard): see `docs/camelot-comparison-replication.md`.
