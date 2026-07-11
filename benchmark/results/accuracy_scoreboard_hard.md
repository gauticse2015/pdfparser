# PDF Parser Accuracy Scoreboard

**Suite:** `regression_hard` · **Docs:** 13 · **ICDAR excluded** (competitive-only)

## Executive summary — table quality

Primary ranking for table-engine work (mean over docs that produce table metrics):

| Rank | Library | cell F1 | detect F1 | shape exact | overall | ms |
|-----:|---------|--------:|----------:|------------:|--------:|---:|
| 1 | **pdfparser** ← **ours** | 1.000 | 1.000 | 1.000 | 100.000 | 7.8 |
| 2 | **pdfplumber** | 0.889 | 0.897 | 0.885 | 90.741 | 13.9 |
| 3 | **camelot_auto** | 0.866 | 0.974 | 0.846 | 87.127 | 429.6 |
| 4 | **pymupdf** | 0.852 | 0.897 | 0.885 | 89.448 | 18.5 |
| 5 | **camelot_lattice** | 0.817 | 0.821 | 0.808 | 77.741 | 40.1 |
| 6 | **img2table** | 0.808 | 0.821 | 0.808 | 77.436 | 77.4 |
| 7 | **camelot_stream** | 0.418 | 0.859 | 0.404 | 58.665 | 7.5 |
| 8 | **pdfminer.six** | 0.000 | 0.000 | 0.000 | 15.000 | 20.3 |
| 9 | **pypdf** | 0.000 | 0.000 | 0.000 | 15.000 | 4.3 |
| 10 | **pypdfium2** | 0.000 | 0.000 | 0.000 | 15.000 | 1.0 |

### Hard tier only (structure stress)

| Rank | Library | cell F1 | detect F1 | shape exact | overall |
|-----:|---------|--------:|----------:|------------:|--------:|
| 1 | **pdfparser** ← **ours** | 1.000 | 1.000 | 1.000 | 100.000 |
| 2 | **pdfplumber** | 0.889 | 0.897 | 0.885 | 90.741 |
| 3 | **camelot_auto** | 0.866 | 0.974 | 0.846 | 87.127 |
| 4 | **pymupdf** | 0.852 | 0.897 | 0.885 | 89.448 |
| 5 | **camelot_lattice** | 0.817 | 0.821 | 0.808 | 77.741 |
| 6 | **img2table** | 0.808 | 0.821 | 0.808 | 77.436 |
| 7 | **camelot_stream** | 0.418 | 0.859 | 0.404 | 58.665 |
| 8 | **pdfminer.six** | 0.000 | 0.000 | 0.000 | 15.000 |
| 9 | **pypdf** | 0.000 | 0.000 | 0.000 | 15.000 |
| 10 | **pypdfium2** | 0.000 | 0.000 | 0.000 | 15.000 |

### Overall product score (text+tables+objects)

| Rank | Library | overall | text F1 | table cell F1 | ms |
|-----:|---------|--------:|--------:|--------------:|---:|
| 1 | **pdfparser** ← **ours** | 100.000 | 1.000 | 1.000 | 7.8 |
| 2 | **pdfplumber** | 90.741 | 1.000 | 0.889 | 13.9 |
| 3 | **pymupdf** | 89.448 | 1.000 | 0.852 | 18.5 |
| 4 | **camelot_auto** | 87.127 | 0.714 | 0.866 | 429.6 |
| 5 | **camelot_lattice** | 77.741 | 0.559 | 0.817 | 40.1 |
| 6 | **img2table** | 77.436 | 0.559 | 0.808 | 77.4 |
| 7 | **camelot_stream** | 58.665 | 0.689 | 0.418 | 7.5 |
| 8 | **pdfminer.six** | 15.000 | 1.000 | 0.000 | 20.3 |
| 9 | **pypdf** | 15.000 | 1.000 | 0.000 | 4.3 |
| 10 | **pypdfium2** | 15.000 | 1.000 | 0.000 | 1.0 |

> Table-primary tools (`camelot_*`, `img2table`) get lower **overall** when outside-table tokens are missing — judge them on **cell F1 / detect F1**.

---

**Generated:** auto · **Docs scored:** 13 · **Libraries:** camelot_auto, camelot_lattice, camelot_stream, img2table, pdfminer.six, pdfparser, pdfplumber, pymupdf, pypdf, pypdfium2

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
| pdfparser | 100.000 | 100.000 | 1.000 | — | 100.000 | 1.000 | 1.000 | 100.000 | 7.8 |
| pdfplumber | 90.741 | 100.000 | 1.000 | — | 89.107 | 0.897 | 0.889 | 100.000 | 13.9 |
| pymupdf | 89.448 | 100.000 | 1.000 | — | 87.586 | 0.897 | 0.852 | 100.000 | 18.5 |
| camelot_auto | 87.127 | 71.374 | 0.714 | — | 89.907 | 0.974 | 0.866 | — | 429.6 |
| camelot_lattice | 77.741 | 55.861 | 0.559 | — | 81.603 | 0.821 | 0.817 | — | 40.1 |
| img2table | 77.436 | 55.861 | 0.559 | — | 81.244 | 0.821 | 0.808 | — | 77.4 |
| camelot_stream | 58.665 | 68.865 | 0.689 | — | 56.865 | 0.859 | 0.418 | — | 7.5 |
| pdfminer.six | 15.000 | 100.000 | 1.000 | — | 0.000 | 0.000 | 0.000 | 100.000 | 20.3 |
| pypdf | 15.000 | 100.000 | 1.000 | — | 0.000 | 0.000 | 0.000 | 100.000 | 4.3 |
| pypdfium2 | 15.000 | 100.000 | 1.000 | — | 0.000 | 0.000 | 0.000 | — | 1.0 |

## Grid-gold subset (synthetic tables with full cell grids)
Best apples-to-apples table quality comparison.

| Library | n | detect F1 | shape exact | cell F1 | table score |
|---------|--:|----------:|------------:|--------:|------------:|
| pdfparser | 13 | 1.000 | 1.000 | 1.000 | 100.000 |
| pdfplumber | 13 | 0.897 | 0.885 | 0.889 | 89.107 |
| pymupdf | 13 | 0.897 | 0.885 | 0.852 | 87.586 |
| camelot_auto | 13 | 0.974 | 0.846 | 0.866 | 89.907 |
| camelot_lattice | 13 | 0.821 | 0.808 | 0.817 | 81.603 |
| img2table | 13 | 0.821 | 0.808 | 0.808 | 81.244 |
| camelot_stream | 13 | 0.859 | 0.404 | 0.418 | 56.865 |
| pdfminer.six | 13 | 0.000 | 0.000 | 0.000 | 0.000 |
| pypdf | 13 | 0.000 | 0.000 | 0.000 | 0.000 |
| pypdfium2 | 13 | 0.000 | 0.000 | 0.000 | 0.000 |

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
| pdfparser | 13 | 100.000 | 100.000 | 100.000 | 1.000 | 1.000 | 1.000 | 100.000 |
| pdfplumber | 13 | 90.741 | 100.000 | 89.107 | 0.897 | 0.889 | 0.885 | 100.000 |
| pymupdf | 13 | 89.448 | 100.000 | 87.586 | 0.897 | 0.852 | 0.885 | 100.000 |
| camelot_auto | 13 | 87.127 | 71.374 | 89.907 | 0.974 | 0.866 | 0.846 | — |
| camelot_lattice | 13 | 77.741 | 55.861 | 81.603 | 0.821 | 0.817 | 0.808 | — |
| img2table | 13 | 77.436 | 55.861 | 81.244 | 0.821 | 0.808 | 0.808 | — |
| camelot_stream | 13 | 58.665 | 68.865 | 56.865 | 0.859 | 0.418 | 0.404 | — |
| pdfminer.six | 13 | 15.000 | 100.000 | 0.000 | 0.000 | 0.000 | 0.000 | 100.000 |
| pypdf | 13 | 15.000 | 100.000 | 0.000 | 0.000 | 0.000 | 0.000 | 100.000 |
| pypdfium2 | 13 | 15.000 | 100.000 | 0.000 | 0.000 | 0.000 | 0.000 | — |

### real

| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|

## Per-document matrix (overall score)

| Doc | camelot_auto | camelot_lattice | camelot_stream | img2table | pdfminer.six | pdfparser | pdfplumber | pymupdf | pypdf | pypdfium2 |
|-----|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|
| `50_multi_table_stacked_page` | 96.2 | 96.2 | 96.2 | 96.2 | 15.0 | 100.0 | 100.0 | 99.0 | 15.0 | 15.0 |
| `51_multi_table_multipage` | 93.6 | 93.6 | 41.4 | 93.6 | 15.0 | 100.0 | 99.7 | 98.4 | 15.0 | 15.0 |
| `52_page_border_noise` | 44.8 | 90.0 | 90.0 | 90.0 | 15.0 | 100.0 | 100.0 | 99.1 | 15.0 | 15.0 |
| `53_column_span_header` | 97.5 | 97.5 | 42.2 | 95.8 | 15.0 | 100.0 | 100.0 | 97.6 | 15.0 | 15.0 |
| `54_row_span_categories` | 97.5 | 97.5 | 47.0 | 95.2 | 15.0 | 100.0 | 100.0 | 96.7 | 15.0 | 15.0 |
| `55_gap_broken_corners` | 96.2 | 0.0 | 96.2 | 0.0 | 15.0 | 100.0 | 100.0 | 98.2 | 15.0 | 15.0 |
| `56_stacked_uneven_tables` | 95.0 | 95.0 | 24.8 | 95.0 | 15.0 | 100.0 | 99.5 | 98.4 | 15.0 | 15.0 |
| `57_wide_statistical_grid` | 97.9 | 97.9 | 45.1 | 97.9 | 15.0 | 100.0 | 100.0 | 99.7 | 15.0 | 15.0 |
| `58_multipage_continued_table` | 95.0 | 95.0 | 95.0 | 95.0 | 15.0 | 100.0 | 100.0 | 99.6 | 15.0 | 15.0 |
| `59_stream_multi_region` | 71.0 | 0.0 | 34.8 | 0.0 | 15.0 | 100.0 | 15.0 | 15.0 | 15.0 | 15.0 |
| `60_mixed_lattice_stream_page` | 56.7 | 56.7 | 24.8 | 56.7 | 15.0 | 100.0 | 66.7 | 65.7 | 15.0 | 15.0 |
| `61_decorative_hline_noise` | 95.0 | 95.0 | 95.0 | 95.0 | 15.0 | 100.0 | 100.0 | 98.0 | 15.0 | 15.0 |
| `62_two_close_grids` | 96.2 | 96.2 | 29.9 | 96.2 | 15.0 | 100.0 | 98.7 | 97.4 | 15.0 | 15.0 |

## Per-document table cell F1 (grid gold only)

| Doc | camelot_auto | camelot_lattice | camelot_stream | img2table | pdfminer.six | pdfparser | pdfplumber | pymupdf | pypdf | pypdfium2 |
|-----|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|
| `50_multi_table_stacked_page` | 1.000 | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.972 | 0.000 | 0.000 |
| `51_multi_table_multipage` | 1.000 | 1.000 | 0.353 | 1.000 | 0.000 | 1.000 | 0.991 | 0.953 | 0.000 | 0.000 |
| `52_page_border_noise` | 0.000 | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.974 | 0.000 | 0.000 |
| `53_column_span_header` | 1.000 | 1.000 | 0.000 | 0.950 | 0.000 | 1.000 | 1.000 | 0.930 | 0.000 | 0.000 |
| `54_row_span_categories` | 1.000 | 1.000 | 0.067 | 0.933 | 0.000 | 1.000 | 1.000 | 0.902 | 0.000 | 0.000 |
| `55_gap_broken_corners` | 1.000 | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 | 1.000 | 0.947 | 0.000 | 0.000 |
| `56_stacked_uneven_tables` | 1.000 | 1.000 | 0.000 | 1.000 | 0.000 | 1.000 | 0.985 | 0.954 | 0.000 | 0.000 |
| `57_wide_statistical_grid` | 1.000 | 1.000 | 0.010 | 1.000 | 0.000 | 1.000 | 1.000 | 0.990 | 0.000 | 0.000 |
| `58_multipage_continued_table` | 1.000 | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.987 | 0.000 | 0.000 |
| `59_stream_multi_region` | 0.636 | 0.000 | 0.000 | 0.000 | 0.000 | 1.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| `60_mixed_lattice_stream_page` | 0.625 | 0.625 | 0.000 | 0.625 | 0.000 | 1.000 | 0.625 | 0.596 | 0.000 | 0.000 |
| `61_decorative_hline_noise` | 1.000 | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.941 | 0.000 | 0.000 |
| `62_two_close_grids` | 1.000 | 1.000 | 0.000 | 1.000 | 0.000 | 1.000 | 0.963 | 0.923 | 0.000 | 0.000 |

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
- **Libraries:** pdfparser, camelot_lattice, camelot_stream, camelot_auto, img2table, pdfplumber, pymupdf, pdfminer.six, pypdf, pypdfium2
- **Note:** `camelot_*` / `img2table` / `tabula` are table-primary; text scores use cell-concatenated text only (outside-table tokens may miss).
- **Tabula:** skipped when no working JRE is installed.

## Table-quality leaderboard (mean over docs with table gold)

| Rank | Library | detect F1 | shape exact | cell F1 | table score | ms |
|-----:|---------|----------:|------------:|--------:|------------:|---:|
| 1 | **pdfparser** | 1.000 | 1.000 | 1.000 | 100.000 | 7.8 |
| 2 | **pdfplumber** | 0.897 | 0.885 | 0.889 | 89.107 | 13.9 |
| 3 | **camelot_auto** | 0.974 | 0.846 | 0.866 | 89.907 | 429.6 |
| 4 | **pymupdf** | 0.897 | 0.885 | 0.852 | 87.586 | 18.5 |
| 5 | **camelot_lattice** | 0.821 | 0.808 | 0.817 | 81.603 | 40.1 |
| 6 | **img2table** | 0.821 | 0.808 | 0.808 | 81.244 | 77.4 |
| 7 | **camelot_stream** | 0.859 | 0.404 | 0.418 | 56.865 | 7.5 |
| 8 | **pdfminer.six** | 0.000 | 0.000 | 0.000 | 0.000 | 20.3 |
| 9 | **pypdf** | 0.000 | 0.000 | 0.000 | 0.000 | 4.3 |
| 10 | **pypdfium2** | 0.000 | 0.000 | 0.000 | 0.000 | 1.0 |

## Hard tier (structure stress 50–62)

| Library | n | detect F1 | shape exact | cell F1 | table score | overall |
|---------|--:|----------:|------------:|--------:|------------:|--------:|
| pdfparser | 13 | 1.000 | 1.000 | 1.000 | 100.000 | 100.000 |
| pdfplumber | 13 | 0.897 | 0.885 | 0.889 | 89.107 | 90.741 |
| camelot_auto | 13 | 0.974 | 0.846 | 0.866 | 89.907 | 87.127 |
| pymupdf | 13 | 0.897 | 0.885 | 0.852 | 87.586 | 89.448 |
| camelot_lattice | 13 | 0.821 | 0.808 | 0.817 | 81.603 | 77.741 |
| img2table | 13 | 0.821 | 0.808 | 0.808 | 81.244 | 77.436 |
| camelot_stream | 13 | 0.859 | 0.404 | 0.418 | 56.865 | 58.665 |
| pdfminer.six | 13 | 0.000 | 0.000 | 0.000 | 0.000 | 15.000 |
| pypdf | 13 | 0.000 | 0.000 | 0.000 | 0.000 | 15.000 |
| pypdfium2 | 13 | 0.000 | 0.000 | 0.000 | 0.000 | 15.000 |

## Hard suite — per-doc table detect F1

| Doc | pdfparser | camelot_lattice | camelot_stream | camelot_auto | img2table | pdfplumber | pymupdf | pdfminer.six | pypdf | pypdfium2 |
|-----|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|
| `50_multi_table_stacked_page` | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `51_multi_table_multipage` | 1.00 | 1.00 | 0.67 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `52_page_border_noise` | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `53_column_span_header` | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `54_row_span_categories` | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `55_gap_broken_corners` | 1.00 | 0.00 | 1.00 | 1.00 | 0.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `56_stacked_uneven_tables` | 1.00 | 1.00 | 0.67 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `57_wide_statistical_grid` | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `58_multipage_continued_table` | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `59_stream_multi_region` | 1.00 | 0.00 | 0.67 | 1.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 |
| `60_mixed_lattice_stream_page` | 1.00 | 0.67 | 0.67 | 0.67 | 0.67 | 0.67 | 0.67 | 0.00 | 0.00 | 0.00 |
| `61_decorative_hline_noise` | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `62_two_close_grids` | 1.00 | 1.00 | 0.50 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |

## Hard suite — per-doc table cell F1

| Doc | pdfparser | camelot_lattice | camelot_stream | camelot_auto | img2table | pdfplumber | pymupdf | pdfminer.six | pypdf | pypdfium2 |
|-----|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|
| `50_multi_table_stacked_page` | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0.972 | 0.000 | 0.000 | 0.000 |
| `51_multi_table_multipage` | 1.000 | 1.000 | 0.353 | 1.000 | 1.000 | 0.991 | 0.953 | 0.000 | 0.000 | 0.000 |
| `52_page_border_noise` | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.974 | 0.000 | 0.000 | 0.000 |
| `53_column_span_header` | 1.000 | 1.000 | 0.000 | 1.000 | 0.950 | 1.000 | 0.930 | 0.000 | 0.000 | 0.000 |
| `54_row_span_categories` | 1.000 | 1.000 | 0.067 | 1.000 | 0.933 | 1.000 | 0.902 | 0.000 | 0.000 | 0.000 |
| `55_gap_broken_corners` | 1.000 | 0.000 | 1.000 | 1.000 | 0.000 | 1.000 | 0.947 | 0.000 | 0.000 | 0.000 |
| `56_stacked_uneven_tables` | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.985 | 0.954 | 0.000 | 0.000 | 0.000 |
| `57_wide_statistical_grid` | 1.000 | 1.000 | 0.010 | 1.000 | 1.000 | 1.000 | 0.990 | 0.000 | 0.000 | 0.000 |
| `58_multipage_continued_table` | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0.987 | 0.000 | 0.000 | 0.000 |
| `59_stream_multi_region` | 1.000 | 0.000 | 0.000 | 0.636 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| `60_mixed_lattice_stream_page` | 1.000 | 0.625 | 0.000 | 0.625 | 0.625 | 0.625 | 0.596 | 0.000 | 0.000 | 0.000 |
| `61_decorative_hline_noise` | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0.941 | 0.000 | 0.000 | 0.000 |
| `62_two_close_grids` | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.963 | 0.923 | 0.000 | 0.000 | 0.000 |

## How to re-run

```bash
cargo build --release -p pdfparser-cli
source .venv/bin/activate
python benchmark/scripts/run_accuracy_benchmark.py --suite regression
python benchmark/scripts/run_accuracy_benchmark.py --suite regression_hard
```

Competitive ICDAR (external, not this scoreboard): see `docs/camelot-comparison-replication.md`.
