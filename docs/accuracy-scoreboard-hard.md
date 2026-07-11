# PDF Parser Accuracy Scoreboard

**Suite:** `regression_hard` · **Docs:** 13 · **ICDAR excluded** (competitive-only)

## Executive summary — table quality

Primary ranking for table-engine work (mean over docs that produce table metrics):

| Rank | Library | cell F1 | detect F1 | shape exact | overall | ms |
|-----:|---------|--------:|----------:|------------:|--------:|---:|
| 1 | **pdfparser** ← **ours** | 0.993 | 1.000 | 1.000 | 99.756 | 6.0 |
| 2 | **pdfplumber** | 0.889 | 0.897 | 0.885 | 90.741 | 12.7 |
| 3 | **camelot_lattice** | 0.817 | 0.821 | 0.808 | 77.741 | 32.8 |

### Hard tier only (structure stress)

| Rank | Library | cell F1 | detect F1 | shape exact | overall |
|-----:|---------|--------:|----------:|------------:|--------:|
| 1 | **pdfparser** ← **ours** | 0.993 | 1.000 | 1.000 | 99.756 |
| 2 | **pdfplumber** | 0.889 | 0.897 | 0.885 | 90.741 |
| 3 | **camelot_lattice** | 0.817 | 0.821 | 0.808 | 77.741 |

### Overall product score (text+tables+objects)

| Rank | Library | overall | text F1 | table cell F1 | ms |
|-----:|---------|--------:|--------:|--------------:|---:|
| 1 | **pdfparser** ← **ours** | 99.756 | 1.000 | 0.993 | 6.0 |
| 2 | **pdfplumber** | 90.741 | 1.000 | 0.889 | 12.7 |
| 3 | **camelot_lattice** | 77.741 | 0.559 | 0.817 | 32.8 |

> Table-primary tools (`camelot_*`, `img2table`) get lower **overall** when outside-table tokens are missing — judge them on **cell F1 / detect F1**.

---

**Generated:** auto · **Docs scored:** 13 · **Libraries:** camelot_lattice, pdfparser, pdfplumber

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
| pdfparser | 99.756 | 100.000 | 1.000 | — | 99.713 | 1.000 | 0.993 | 100.000 | 6.0 |
| pdfplumber | 90.741 | 100.000 | 1.000 | — | 89.107 | 0.897 | 0.889 | 100.000 | 12.7 |
| camelot_lattice | 77.741 | 55.861 | 0.559 | — | 81.603 | 0.821 | 0.817 | — | 32.8 |

## Grid-gold subset (synthetic tables with full cell grids)
Best apples-to-apples table quality comparison.

| Library | n | detect F1 | shape exact | cell F1 | table score |
|---------|--:|----------:|------------:|--------:|------------:|
| pdfparser | 13 | 1.000 | 1.000 | 0.993 | 99.713 |
| pdfplumber | 13 | 0.897 | 0.885 | 0.889 | 89.107 |
| camelot_lattice | 13 | 0.821 | 0.808 | 0.817 | 81.603 |

## By tier

### basic

| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|

### stress

| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|

### hard

| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|
| pdfparser | 13 | 99.756 | 100.000 | 99.713 | 1.000 | 0.993 | 1.000 | 100.000 |
| pdfplumber | 13 | 90.741 | 100.000 | 89.107 | 0.897 | 0.889 | 0.885 | 100.000 |
| camelot_lattice | 13 | 77.741 | 55.861 | 81.603 | 0.821 | 0.817 | 0.808 | — |

### real

| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|

## Per-document matrix (overall score)

| Doc | camelot_lattice | pdfparser | pdfplumber |
|-----|-----:|-----:|-----:|
| `50_multi_table_stacked_page` | 96.2 | 100.0 | 100.0 |
| `51_multi_table_multipage` | 93.6 | 100.0 | 99.7 |
| `52_page_border_noise` | 90.0 | 100.0 | 100.0 |
| `53_column_span_header` | 97.5 | 99.1 | 100.0 |
| `54_row_span_categories` | 97.5 | 98.1 | 100.0 |
| `55_gap_broken_corners` | 0.0 | 100.0 | 100.0 |
| `56_stacked_uneven_tables` | 95.0 | 100.0 | 99.5 |
| `57_wide_statistical_grid` | 97.9 | 99.7 | 100.0 |
| `58_multipage_continued_table` | 95.0 | 100.0 | 100.0 |
| `59_stream_multi_region` | 0.0 | 100.0 | 15.0 |
| `60_mixed_lattice_stream_page` | 56.7 | 100.0 | 66.7 |
| `61_decorative_hline_noise` | 95.0 | 100.0 | 100.0 |
| `62_two_close_grids` | 96.2 | 100.0 | 98.7 |

## Per-document table cell F1 (grid gold only)

| Doc | camelot_lattice | pdfparser | pdfplumber |
|-----|-----:|-----:|-----:|
| `50_multi_table_stacked_page` | 1.000 | 1.000 | 1.000 |
| `51_multi_table_multipage` | 1.000 | 1.000 | 0.991 |
| `52_page_border_noise` | 1.000 | 1.000 | 1.000 |
| `53_column_span_header` | 1.000 | 0.973 | 1.000 |
| `54_row_span_categories` | 1.000 | 0.943 | 1.000 |
| `55_gap_broken_corners` | 0.000 | 1.000 | 1.000 |
| `56_stacked_uneven_tables` | 1.000 | 1.000 | 0.985 |
| `57_wide_statistical_grid` | 1.000 | 0.990 | 1.000 |
| `58_multipage_continued_table` | 1.000 | 1.000 | 1.000 |
| `59_stream_multi_region` | 0.000 | 1.000 | 0.000 |
| `60_mixed_lattice_stream_page` | 0.625 | 1.000 | 0.625 |
| `61_decorative_hline_noise` | 1.000 | 1.000 | 1.000 |
| `62_two_close_grids` | 1.000 | 1.000 | 0.963 |

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

- **Suite:** `regression_hard` (ICDAR never included in regression)
- **Documents scored:** 13
- **Libraries:** pdfparser, pdfplumber, camelot_lattice
- **Note:** `camelot_*` / `img2table` / `tabula` are table-primary; text scores use cell-concatenated text only (outside-table tokens may miss).
- **Tabula:** skipped when no working JRE is installed.

## Table-quality leaderboard (mean over docs with table gold)

| Rank | Library | detect F1 | shape exact | cell F1 | table score | ms |
|-----:|---------|----------:|------------:|--------:|------------:|---:|
| 1 | **pdfparser** | 1.000 | 1.000 | 0.993 | 99.713 | 6.0 |
| 2 | **pdfplumber** | 0.897 | 0.885 | 0.889 | 89.107 | 12.7 |
| 3 | **camelot_lattice** | 0.821 | 0.808 | 0.817 | 81.603 | 32.8 |

## Hard tier (structure stress 50–62)

| Library | n | detect F1 | shape exact | cell F1 | table score | overall |
|---------|--:|----------:|------------:|--------:|------------:|--------:|
| pdfparser | 13 | 1.000 | 1.000 | 0.993 | 99.713 | 99.756 |
| pdfplumber | 13 | 0.897 | 0.885 | 0.889 | 89.107 | 90.741 |
| camelot_lattice | 13 | 0.821 | 0.808 | 0.817 | 81.603 | 77.741 |

## Hard suite — per-doc table detect F1

| Doc | pdfparser | pdfplumber | camelot_lattice |
|-----|-----:|-----:|-----:|
| `50_multi_table_stacked_page` | 1.00 | 1.00 | 1.00 |
| `51_multi_table_multipage` | 1.00 | 1.00 | 1.00 |
| `52_page_border_noise` | 1.00 | 1.00 | 1.00 |
| `53_column_span_header` | 1.00 | 1.00 | 1.00 |
| `54_row_span_categories` | 1.00 | 1.00 | 1.00 |
| `55_gap_broken_corners` | 1.00 | 1.00 | 0.00 |
| `56_stacked_uneven_tables` | 1.00 | 1.00 | 1.00 |
| `57_wide_statistical_grid` | 1.00 | 1.00 | 1.00 |
| `58_multipage_continued_table` | 1.00 | 1.00 | 1.00 |
| `59_stream_multi_region` | 1.00 | 0.00 | 0.00 |
| `60_mixed_lattice_stream_page` | 1.00 | 0.67 | 0.67 |
| `61_decorative_hline_noise` | 1.00 | 1.00 | 1.00 |
| `62_two_close_grids` | 1.00 | 1.00 | 1.00 |

## Hard suite — per-doc table cell F1

| Doc | pdfparser | pdfplumber | camelot_lattice |
|-----|-----:|-----:|-----:|
| `50_multi_table_stacked_page` | 1.000 | 1.000 | 1.000 |
| `51_multi_table_multipage` | 1.000 | 0.991 | 1.000 |
| `52_page_border_noise` | 1.000 | 1.000 | 1.000 |
| `53_column_span_header` | 0.973 | 1.000 | 1.000 |
| `54_row_span_categories` | 0.943 | 1.000 | 1.000 |
| `55_gap_broken_corners` | 1.000 | 1.000 | 0.000 |
| `56_stacked_uneven_tables` | 1.000 | 0.985 | 1.000 |
| `57_wide_statistical_grid` | 0.990 | 1.000 | 1.000 |
| `58_multipage_continued_table` | 1.000 | 1.000 | 1.000 |
| `59_stream_multi_region` | 1.000 | 0.000 | 0.000 |
| `60_mixed_lattice_stream_page` | 1.000 | 0.625 | 0.625 |
| `61_decorative_hline_noise` | 1.000 | 1.000 | 1.000 |
| `62_two_close_grids` | 1.000 | 0.963 | 1.000 |

## How to re-run

```bash
cargo build --release -p pdfparser-cli
source .venv/bin/activate
python benchmark/scripts/run_accuracy_benchmark.py --suite regression
python benchmark/scripts/run_accuracy_benchmark.py --suite regression_hard
```

Competitive ICDAR (external, not this scoreboard): see `docs/camelot-comparison-replication.md`.
