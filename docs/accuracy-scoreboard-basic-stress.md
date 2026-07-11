# PDF Parser Accuracy Scoreboard

**Suite:** `regression_basic_stress` · **Docs:** 20 · **ICDAR excluded** (competitive-only)

## Executive summary — table quality

Primary ranking for table-engine work (mean over docs that produce table metrics):

| Rank | Library | cell F1 | detect F1 | shape exact | overall | ms |
|-----:|---------|--------:|----------:|------------:|--------:|---:|
| 1 | **pdfparser** ← **ours** | 0.992 | 0.895 | 1.000 | 99.844 | 10.6 |
| 2 | **pdfplumber** | 0.826 | 0.933 | 0.727 | 88.114 | 94.2 |
| 3 | **camelot_lattice** | 0.826 | 0.933 | 0.727 | 46.889 | 32.0 |
| 4 | **camelot_auto** | 0.826 | 0.883 | 0.727 | 54.350 | 333.9 |

### Overall product score (text+tables+objects)

| Rank | Library | overall | text F1 | table cell F1 | ms |
|-----:|---------|--------:|--------:|--------------:|---:|
| 1 | **pdfparser** ← **ours** | 99.844 | 1.000 | 0.992 | 10.6 |
| 2 | **pdfplumber** | 88.114 | 0.988 | 0.826 | 94.2 |
| 3 | **camelot_auto** | 54.350 | 0.501 | 0.826 | 333.9 |
| 4 | **camelot_lattice** | 46.889 | 0.389 | 0.826 | 32.0 |

> Table-primary tools (`camelot_*`, `img2table`) get lower **overall** when outside-table tokens are missing — judge them on **cell F1 / detect F1**.

---

**Generated:** auto · **Docs scored:** 20 · **Libraries:** camelot_auto, camelot_lattice, pdfparser, pdfplumber

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
| pdfparser | 99.844 | 100.000 | 1.000 | 0.000 | 89.279 | 0.895 | 0.992 | 100.000 | 10.6 |
| pdfplumber | 88.114 | 97.413 | 0.988 | 0.267 | 89.891 | 0.933 | 0.826 | 97.500 | 94.2 |
| camelot_auto | 54.350 | 48.749 | 0.501 | 0.770 | 81.641 | 0.883 | 0.826 | 0.000 | 333.9 |
| camelot_lattice | 46.889 | 38.851 | 0.389 | 1.000 | 89.891 | 0.933 | 0.826 | 0.000 | 32.0 |

## Grid-gold subset (synthetic tables with full cell grids)
Best apples-to-apples table quality comparison.

| Library | n | detect F1 | shape exact | cell F1 | table score |
|---------|--:|----------:|------------:|--------:|------------:|
| pdfparser | 11 | 1.000 | 1.000 | 0.992 | 99.664 |
| pdfplumber | 11 | 0.909 | 0.727 | 0.826 | 83.034 |
| camelot_auto | 11 | 1.000 | 0.727 | 0.826 | 86.216 |
| camelot_lattice | 11 | 0.909 | 0.727 | 0.826 | 83.034 |

## By tier

### basic

| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|
| pdfparser | 11 | 100.000 | 100.000 | 90.909 | 0.909 | 1.000 | 1.000 | 100.000 |
| pdfplumber | 12 | 84.913 | 97.772 | 86.744 | 0.917 | 0.630 | 0.600 | 95.833 |
| camelot_auto | 12 | 38.048 | 41.636 | 81.327 | 0.917 | 0.630 | 0.600 | 0.000 |
| camelot_lattice | 12 | 27.697 | 27.222 | 86.744 | 0.917 | 0.630 | 0.600 | 0.000 |

### stress

| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|
| pdfparser | 8 | 99.630 | 100.000 | 87.038 | 0.875 | 0.985 | 1.000 | 100.000 |
| pdfplumber | 8 | 92.915 | 96.875 | 94.611 | 0.958 | 0.989 | 0.833 | 100.000 |
| camelot_auto | 8 | 78.803 | 59.420 | 82.111 | 0.833 | 0.989 | 0.833 | — |
| camelot_lattice | 8 | 75.678 | 56.295 | 94.611 | 0.958 | 0.989 | 0.833 | — |

### hard

| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|

### real

| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|

## Per-document matrix (overall score)

| Doc | camelot_auto | camelot_lattice | pdfparser | pdfplumber |
|-----|-----:|-----:|-----:|-----:|
| `01_simple_text` | 0.0 | 0.0 | 100.0 | 100.0 |
| `02_multi_column` | 73.0 | 0.0 | 100.0 | 73.3 |
| `03_large_multipage` | 0.0 | 0.0 | 100.0 | 100.0 |
| `04_image_heavy` | 0.0 | 0.0 | 100.0 | 100.0 |
| `05_special_objects` | 0.0 | 0.0 | 100.0 | 65.0 |
| `06_table_lattice` | 95.8 | 95.8 | 100.0 | 100.0 |
| `07_table_stream` | 51.2 | 0.0 | 100.0 | 25.0 |
| `08_table_partial_border` | 51.5 | 51.5 | 100.0 | 55.7 |
| `09_table_complex` | 100.0 | 100.0 | 100.0 | 100.0 |
| `10_mixed_document` | 85.0 | 85.0 | 100.0 | 100.0 |
| `11_rotated_page` | 0.0 | 0.0 | 100.0 | 100.0 |
| `12_encrypted_password` | 0.0 | 0.0 | FAIL | 100.0 |
| `20_bank_statement_multipage` | 72.0 | 72.0 | 100.0 | 89.1 |
| `21_table_overflow_cells` | 94.4 | 94.4 | 98.4 | 98.4 |
| `22_table_merged_headers` | 97.1 | 97.1 | 99.6 | 99.6 |
| `23_side_by_side_tables` | 95.0 | 95.0 | 99.1 | 100.0 |
| `24_invoice_line_items` | 68.8 | 68.8 | 100.0 | 81.2 |
| `25_dense_numeric_grid` | 96.2 | 96.2 | 100.0 | 100.0 |
| `26_watermark_overlap` | 25.0 | 0.0 | 100.0 | 75.0 |
| `27_table_with_footnotes` | 82.0 | 82.0 | 100.0 | 100.0 |

## Per-document table cell F1 (grid gold only)

| Doc | camelot_auto | camelot_lattice | pdfparser | pdfplumber |
|-----|-----:|-----:|-----:|-----:|
| `06_table_lattice` | 1.000 | 1.000 | 1.000 | 1.000 |
| `07_table_stream` | 0.000 | 0.000 | 1.000 | 0.000 |
| `08_table_partial_border` | 0.148 | 0.148 | 1.000 | 0.148 |
| `09_table_complex` | 1.000 | 1.000 | 1.000 | 1.000 |
| `10_mixed_document` | 1.000 | 1.000 | 1.000 | 1.000 |
| `21_table_overflow_cells` | 0.950 | 0.950 | 0.950 | 0.950 |
| `22_table_merged_headers` | 0.987 | 0.987 | 0.987 | 0.987 |
| `23_side_by_side_tables` | 1.000 | 1.000 | 0.971 | 1.000 |
| `24_invoice_line_items` | 1.000 | 1.000 | 1.000 | 1.000 |
| `25_dense_numeric_grid` | 1.000 | 1.000 | 1.000 | 1.000 |
| `27_table_with_footnotes` | 1.000 | 1.000 | 1.000 | 1.000 |

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
- **Libraries:** pdfparser, pdfplumber, camelot_lattice, camelot_auto
- **Note:** `camelot_*` / `img2table` / `tabula` are table-primary; text scores use cell-concatenated text only (outside-table tokens may miss).
- **Tabula:** skipped when no working JRE is installed.

## Table-quality leaderboard (mean over docs with table gold)

| Rank | Library | detect F1 | shape exact | cell F1 | table score | ms |
|-----:|---------|----------:|------------:|--------:|------------:|---:|
| 1 | **pdfparser** | 0.895 | 1.000 | 0.992 | 89.279 | 10.6 |
| 2 | **pdfplumber** | 0.933 | 0.727 | 0.826 | 89.891 | 94.2 |
| 3 | **camelot_lattice** | 0.933 | 0.727 | 0.826 | 89.891 | 32.0 |
| 4 | **camelot_auto** | 0.883 | 0.727 | 0.826 | 81.641 | 333.9 |

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
