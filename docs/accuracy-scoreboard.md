# PDF Parser Accuracy Scoreboard

**Suite:** `regression` · **Docs:** 33 · **ICDAR excluded** (competitive-only)

## Executive summary — table quality

Primary ranking for table-engine work (mean over docs that produce table metrics):

| Rank | Library | cell F1 | detect F1 | shape exact | overall | ms |
|-----:|---------|--------:|----------:|------------:|--------:|---:|
| 1 | **pdfparser** ← **ours** | 0.994 | 0.938 | 0.958 | 99.258 | 11.0 |
| 2 | **pdfplumber** | 0.860 | 0.919 | 0.812 | 89.149 | 60.3 |
| 3 | **camelot_auto** | 0.848 | 0.919 | 0.792 | 67.262 | 375.3 |
| 4 | **camelot_lattice** | 0.821 | 0.889 | 0.771 | 59.043 | 25.9 |
| 5 | **pymupdf** | 0.816 | 0.889 | 0.812 | 88.809 | 73.1 |
| 6 | **img2table** | 0.768 | 0.854 | 0.771 | 57.824 | 72.8 |
| 7 | **pdfminer.six** | 0.000 | 0.242 | 0.000 | 35.975 | 66.9 |
| 8 | **pypdf** | 0.000 | 0.242 | 0.000 | 39.394 | 7.5 |
| 9 | **pypdfium2** | 0.000 | 0.242 | 0.000 | 36.073 | 1.9 |

### Hard tier only (structure stress)

| Rank | Library | cell F1 | detect F1 | shape exact | overall |
|-----:|---------|--------:|----------:|------------:|--------:|
| 1 | **pdfparser** ← **ours** | 0.993 | 1.000 | 1.000 | 99.756 |
| 2 | **pdfplumber** | 0.889 | 0.897 | 0.885 | 90.741 |
| 3 | **camelot_auto** | 0.866 | 0.974 | 0.846 | 87.127 |
| 4 | **pymupdf** | 0.852 | 0.897 | 0.885 | 89.448 |
| 5 | **camelot_lattice** | 0.817 | 0.821 | 0.808 | 77.741 |
| 6 | **img2table** | 0.808 | 0.821 | 0.808 | 77.436 |
| 7 | **pdfminer.six** | 0.000 | 0.000 | 0.000 | 15.000 |
| 8 | **pypdf** | 0.000 | 0.000 | 0.000 | 15.000 |
| 9 | **pypdfium2** | 0.000 | 0.000 | 0.000 | 15.000 |

### Overall product score (text+tables+objects)

| Rank | Library | overall | text F1 | table cell F1 | ms |
|-----:|---------|--------:|--------:|--------------:|---:|
| 1 | **pdfparser** ← **ours** | 99.258 | 1.000 | 0.994 | 11.0 |
| 2 | **pdfplumber** | 89.149 | 0.992 | 0.860 | 60.3 |
| 3 | **pymupdf** | 88.809 | 0.992 | 0.816 | 73.1 |
| 4 | **camelot_auto** | 67.262 | 0.585 | 0.848 | 375.3 |
| 5 | **camelot_lattice** | 59.043 | 0.456 | 0.821 | 25.9 |
| 6 | **img2table** | 57.824 | 0.444 | 0.768 | 72.8 |
| 7 | **pypdf** | 39.394 | 1.000 | 0.000 | 7.5 |
| 8 | **pypdfium2** | 36.073 | 0.992 | 0.000 | 1.9 |
| 9 | **pdfminer.six** | 35.975 | 0.985 | 0.000 | 66.9 |

> Table-primary tools (`camelot_*`, `img2table`) get lower **overall** when outside-table tokens are missing — judge them on **cell F1 / detect F1**.

---

**Generated:** auto · **Docs scored:** 33 · **Libraries:** camelot_auto, camelot_lattice, img2table, pdfminer.six, pdfparser, pdfplumber, pymupdf, pypdf, pypdfium2

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
| pdfparser | 99.258 | 100.000 | 1.000 | 0.000 | 92.781 | 0.938 | 0.994 | 100.000 | 11.0 |
| pdfplumber | 89.149 | 98.432 | 0.992 | 0.267 | 89.582 | 0.919 | 0.860 | 98.485 | 60.3 |
| pymupdf | 88.809 | 99.242 | 0.992 | 0.000 | 86.695 | 0.889 | 0.816 | 100.000 | 73.1 |
| camelot_auto | 67.262 | 57.662 | 0.585 | 0.770 | 84.897 | 0.919 | 0.848 | 0.000 | 375.3 |
| camelot_lattice | 59.043 | 45.552 | 0.456 | 1.000 | 86.626 | 0.889 | 0.821 | 0.000 | 25.9 |
| img2table | 57.824 | 44.371 | 0.444 | 1.000 | 82.978 | 0.854 | 0.768 | 0.000 | 72.8 |
| pypdf | 39.394 | 100.000 | 1.000 | 0.000 | 24.242 | 0.242 | 0.000 | 100.000 | 7.5 |
| pypdfium2 | 36.073 | 99.179 | 0.992 | 0.021 | 24.242 | 0.242 | 0.000 | 0.000 | 1.9 |
| pdfminer.six | 35.975 | 97.638 | 0.985 | 0.279 | 24.242 | 0.242 | 0.000 | 98.485 | 66.9 |

## Grid-gold subset (synthetic tables with full cell grids)
Best apples-to-apples table quality comparison.

| Library | n | detect F1 | shape exact | cell F1 | table score |
|---------|--:|----------:|------------:|--------:|------------:|
| pdfparser | 24 | 1.000 | 0.958 | 0.994 | 98.708 |
| pdfplumber | 24 | 0.903 | 0.812 | 0.860 | 86.324 |
| pymupdf | 24 | 0.861 | 0.812 | 0.816 | 83.094 |
| camelot_auto | 24 | 0.986 | 0.792 | 0.848 | 88.215 |
| camelot_lattice | 24 | 0.861 | 0.771 | 0.821 | 82.259 |
| img2table | 24 | 0.819 | 0.771 | 0.768 | 78.692 |
| pypdf | 24 | 0.000 | 0.000 | 0.000 | 0.000 |
| pypdfium2 | 24 | 0.000 | 0.000 | 0.000 | 0.000 |
| pdfminer.six | 24 | 0.000 | 0.000 | 0.000 | 0.000 |

## By tier

### basic

| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|
| pdfparser | 11 | 100.000 | 100.000 | 90.909 | 0.909 | 1.000 | 1.000 | 100.000 |
| pdfplumber | 12 | 84.913 | 97.772 | 86.744 | 0.917 | 0.630 | 0.600 | 95.833 |
| pymupdf | 12 | 87.460 | 100.000 | 83.284 | 0.833 | 0.597 | 0.600 | 100.000 |
| camelot_auto | 12 | 38.048 | 41.636 | 81.327 | 0.917 | 0.630 | 0.600 | 0.000 |
| camelot_lattice | 12 | 27.697 | 27.222 | 86.744 | 0.917 | 0.630 | 0.600 | 0.000 |
| img2table | 11 | 25.488 | 22.121 | 81.766 | 0.818 | 0.597 | 0.600 | 0.000 |
| pypdf | 12 | 70.417 | 100.000 | 58.333 | 0.583 | 0.000 | 0.000 | 100.000 |
| pypdfium2 | 12 | 63.367 | 99.825 | 58.333 | 0.583 | 0.000 | 0.000 | 0.000 |
| pdfminer.six | 12 | 61.016 | 93.506 | 58.333 | 0.583 | 0.000 | 0.000 | 95.833 |

### stress

| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|
| pdfparser | 8 | 97.428 | 100.000 | 84.090 | 0.875 | 0.991 | 0.833 | 100.000 |
| pdfplumber | 8 | 92.915 | 96.875 | 94.611 | 0.958 | 0.989 | 0.833 | 100.000 |
| pymupdf | 8 | 89.792 | 96.875 | 90.364 | 0.958 | 0.922 | 0.833 | 100.000 |
| camelot_auto | 8 | 78.803 | 59.420 | 82.111 | 0.833 | 0.989 | 0.833 | — |
| camelot_lattice | 8 | 75.678 | 56.295 | 94.611 | 0.958 | 0.989 | 0.833 | — |
| img2table | 8 | 70.415 | 56.295 | 87.462 | 0.958 | 0.825 | 0.833 | — |
| pypdf | 8 | 32.500 | 100.000 | 12.500 | 0.125 | 0.000 | 0.000 | 100.000 |
| pypdfium2 | 8 | 29.375 | 96.875 | 12.500 | 0.125 | 0.000 | 0.000 | — |
| pdfminer.six | 8 | 32.500 | 100.000 | 12.500 | 0.125 | 0.000 | 0.000 | 100.000 |

### hard

| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|
| pdfparser | 13 | 99.756 | 100.000 | 99.713 | 1.000 | 0.993 | 1.000 | 100.000 |
| pdfplumber | 13 | 90.741 | 100.000 | 89.107 | 0.897 | 0.889 | 0.885 | 100.000 |
| pymupdf | 13 | 89.448 | 100.000 | 87.586 | 0.897 | 0.852 | 0.885 | 100.000 |
| camelot_auto | 13 | 87.127 | 71.374 | 89.907 | 0.974 | 0.866 | 0.846 | — |
| camelot_lattice | 13 | 77.741 | 55.861 | 81.603 | 0.821 | 0.817 | 0.808 | — |
| img2table | 13 | 77.436 | 55.861 | 81.244 | 0.821 | 0.808 | 0.808 | — |
| pypdf | 13 | 15.000 | 100.000 | 0.000 | 0.000 | 0.000 | 0.000 | 100.000 |
| pypdfium2 | 13 | 15.000 | 100.000 | 0.000 | 0.000 | 0.000 | 0.000 | — |
| pdfminer.six | 13 | 15.000 | 100.000 | 0.000 | 0.000 | 0.000 | 0.000 | 100.000 |

### real

| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|

## Per-document matrix (overall score)

| Doc | camelot_auto | camelot_lattice | img2table | pdfminer.six | pdfparser | pdfplumber | pymupdf | pypdf | pypdfium2 |
|-----|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|
| `01_simple_text` | 0.0 | 0.0 | 0.0 | 99.5 | 100.0 | 100.0 | 100.0 | 100.0 | 99.4 |
| `02_multi_column` | 73.0 | 0.0 | 0.0 | 72.7 | 100.0 | 73.3 | 100.0 | 100.0 | 98.5 |
| `03_large_multipage` | 0.0 | 0.0 | 0.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |
| `04_image_heavy` | 0.0 | 0.0 | 0.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |
| `05_special_objects` | 0.0 | 0.0 | 0.0 | 65.0 | 100.0 | 65.0 | 100.0 | 100.0 | 30.0 |
| `06_table_lattice` | 95.8 | 95.8 | 95.8 | 25.0 | 100.0 | 100.0 | 100.0 | 25.0 | 25.0 |
| `07_table_stream` | 51.2 | 0.0 | 0.0 | 25.0 | 100.0 | 25.0 | 25.0 | 25.0 | 25.0 |
| `08_table_partial_border` | 51.5 | 51.5 | 0.0 | 25.0 | 100.0 | 55.7 | 25.0 | 25.0 | 25.0 |
| `09_table_complex` | 100.0 | 100.0 | 99.5 | 20.0 | 100.0 | 100.0 | 99.5 | 20.0 | 20.0 |
| `10_mixed_document` | 85.0 | 85.0 | 85.0 | 50.0 | 100.0 | 100.0 | 100.0 | 50.0 | 37.5 |
| `11_rotated_page` | 0.0 | 0.0 | 0.0 | 50.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |
| `12_encrypted_password` | 0.0 | 0.0 | FAIL | 100.0 | FAIL | 100.0 | 100.0 | 100.0 | 100.0 |
| `20_bank_statement_multipage` | 72.0 | 72.0 | 59.5 | 30.0 | 100.0 | 89.1 | 76.7 | 30.0 | 30.0 |
| `21_table_overflow_cells` | 94.4 | 94.4 | 94.4 | 20.0 | 100.0 | 98.4 | 93.7 | 20.0 | 20.0 |
| `22_table_merged_headers` | 97.1 | 97.1 | 95.5 | 20.0 | 99.1 | 99.6 | 97.7 | 20.0 | 20.0 |
| `23_side_by_side_tables` | 95.0 | 95.0 | 95.0 | 20.0 | 99.1 | 100.0 | 98.1 | 20.0 | 20.0 |
| `24_invoice_line_items` | 68.8 | 68.8 | 40.7 | 25.0 | 81.2 | 81.2 | 79.1 | 25.0 | 25.0 |
| `25_dense_numeric_grid` | 96.2 | 96.2 | 96.2 | 15.0 | 100.0 | 100.0 | 100.0 | 15.0 | 15.0 |
| `26_watermark_overlap` | 25.0 | 0.0 | 0.0 | 100.0 | 100.0 | 75.0 | 75.0 | 100.0 | 75.0 |
| `27_table_with_footnotes` | 82.0 | 82.0 | 82.0 | 30.0 | 100.0 | 100.0 | 98.0 | 30.0 | 30.0 |
| `50_multi_table_stacked_page` | 96.2 | 96.2 | 96.2 | 15.0 | 100.0 | 100.0 | 99.0 | 15.0 | 15.0 |
| `51_multi_table_multipage` | 93.6 | 93.6 | 93.6 | 15.0 | 100.0 | 99.7 | 98.4 | 15.0 | 15.0 |
| `52_page_border_noise` | 44.8 | 90.0 | 90.0 | 15.0 | 100.0 | 100.0 | 99.1 | 15.0 | 15.0 |
| `53_column_span_header` | 97.5 | 97.5 | 95.8 | 15.0 | 99.1 | 100.0 | 97.6 | 15.0 | 15.0 |
| `54_row_span_categories` | 97.5 | 97.5 | 95.2 | 15.0 | 98.1 | 100.0 | 96.7 | 15.0 | 15.0 |
| `55_gap_broken_corners` | 96.2 | 0.0 | 0.0 | 15.0 | 100.0 | 100.0 | 98.2 | 15.0 | 15.0 |
| `56_stacked_uneven_tables` | 95.0 | 95.0 | 95.0 | 15.0 | 100.0 | 99.5 | 98.4 | 15.0 | 15.0 |
| `57_wide_statistical_grid` | 97.9 | 97.9 | 97.9 | 15.0 | 99.7 | 100.0 | 99.7 | 15.0 | 15.0 |
| `58_multipage_continued_table` | 95.0 | 95.0 | 95.0 | 15.0 | 100.0 | 100.0 | 99.6 | 15.0 | 15.0 |
| `59_stream_multi_region` | 71.0 | 0.0 | 0.0 | 15.0 | 100.0 | 15.0 | 15.0 | 15.0 | 15.0 |
| `60_mixed_lattice_stream_page` | 56.7 | 56.7 | 56.7 | 15.0 | 100.0 | 66.7 | 65.7 | 15.0 | 15.0 |
| `61_decorative_hline_noise` | 95.0 | 95.0 | 95.0 | 15.0 | 100.0 | 100.0 | 98.0 | 15.0 | 15.0 |
| `62_two_close_grids` | 96.2 | 96.2 | 96.2 | 15.0 | 100.0 | 98.7 | 97.4 | 15.0 | 15.0 |

## Per-document table cell F1 (grid gold only)

| Doc | camelot_auto | camelot_lattice | img2table | pdfminer.six | pdfparser | pdfplumber | pymupdf | pypdf | pypdfium2 |
|-----|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|
| `06_table_lattice` | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 1.000 | 0.000 | 0.000 |
| `07_table_stream` | 0.000 | 0.000 | 0.000 | 0.000 | 1.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| `08_table_partial_border` | 0.148 | 0.148 | 0.000 | 0.000 | 1.000 | 0.148 | 0.000 | 0.000 | 0.000 |
| `09_table_complex` | 1.000 | 1.000 | 0.986 | 0.000 | 1.000 | 1.000 | 0.985 | 0.000 | 0.000 |
| `10_mixed_document` | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 1.000 | 0.000 | 0.000 |
| `21_table_overflow_cells` | 0.950 | 0.950 | 0.950 | 0.000 | 1.000 | 0.950 | 0.805 | 0.000 | 0.000 |
| `22_table_merged_headers` | 0.987 | 0.987 | 0.937 | 0.000 | 0.973 | 0.987 | 0.930 | 0.000 | 0.000 |
| `23_side_by_side_tables` | 1.000 | 1.000 | 1.000 | 0.000 | 0.971 | 1.000 | 0.941 | 0.000 | 0.000 |
| `24_invoice_line_items` | 1.000 | 1.000 | 0.065 | 0.000 | 1.000 | 1.000 | 0.929 | 0.000 | 0.000 |
| `25_dense_numeric_grid` | 1.000 | 1.000 | 1.000 | 0.000 | 0.999 | 1.000 | 0.999 | 0.000 | 0.000 |
| `27_table_with_footnotes` | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.929 | 0.000 | 0.000 |
| `50_multi_table_stacked_page` | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.972 | 0.000 | 0.000 |
| `51_multi_table_multipage` | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 0.991 | 0.953 | 0.000 | 0.000 |
| `52_page_border_noise` | 0.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.974 | 0.000 | 0.000 |
| `53_column_span_header` | 1.000 | 1.000 | 0.950 | 0.000 | 0.973 | 1.000 | 0.930 | 0.000 | 0.000 |
| `54_row_span_categories` | 1.000 | 1.000 | 0.933 | 0.000 | 0.943 | 1.000 | 0.902 | 0.000 | 0.000 |
| `55_gap_broken_corners` | 1.000 | 0.000 | 0.000 | 0.000 | 1.000 | 1.000 | 0.947 | 0.000 | 0.000 |
| `56_stacked_uneven_tables` | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 0.985 | 0.954 | 0.000 | 0.000 |
| `57_wide_statistical_grid` | 1.000 | 1.000 | 1.000 | 0.000 | 0.990 | 1.000 | 0.990 | 0.000 | 0.000 |
| `58_multipage_continued_table` | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.987 | 0.000 | 0.000 |
| `59_stream_multi_region` | 0.636 | 0.000 | 0.000 | 0.000 | 1.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| `60_mixed_lattice_stream_page` | 0.625 | 0.625 | 0.625 | 0.000 | 1.000 | 0.625 | 0.596 | 0.000 | 0.000 |
| `61_decorative_hline_noise` | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.941 | 0.000 | 0.000 |
| `62_two_close_grids` | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 0.963 | 0.923 | 0.000 | 0.000 |

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

- **Suite:** `regression` (ICDAR never included in regression)
- **Documents scored:** 33
- **Libraries:** pdfparser, camelot_lattice, camelot_auto, img2table, pdfplumber, pymupdf, pdfminer.six, pypdf, pypdfium2
- **Note:** `camelot_*` / `img2table` / `tabula` are table-primary; text scores use cell-concatenated text only (outside-table tokens may miss).
- **Tabula:** skipped when no working JRE is installed.

## Table-quality leaderboard (mean over docs with table gold)

| Rank | Library | detect F1 | shape exact | cell F1 | table score | ms |
|-----:|---------|----------:|------------:|--------:|------------:|---:|
| 1 | **pdfparser** | 0.938 | 0.958 | 0.994 | 92.781 | 11.0 |
| 2 | **pdfplumber** | 0.919 | 0.812 | 0.860 | 89.582 | 60.3 |
| 3 | **camelot_auto** | 0.919 | 0.792 | 0.848 | 84.897 | 375.3 |
| 4 | **camelot_lattice** | 0.889 | 0.771 | 0.821 | 86.626 | 25.9 |
| 5 | **pymupdf** | 0.889 | 0.812 | 0.816 | 86.695 | 73.1 |
| 6 | **img2table** | 0.854 | 0.771 | 0.768 | 82.978 | 72.8 |
| 7 | **pdfminer.six** | 0.242 | 0.000 | 0.000 | 24.242 | 66.9 |
| 8 | **pypdf** | 0.242 | 0.000 | 0.000 | 24.242 | 7.5 |
| 9 | **pypdfium2** | 0.242 | 0.000 | 0.000 | 24.242 | 1.9 |

## Hard tier (structure stress 50–62)

| Library | n | detect F1 | shape exact | cell F1 | table score | overall |
|---------|--:|----------:|------------:|--------:|------------:|--------:|
| pdfparser | 13 | 1.000 | 1.000 | 0.993 | 99.713 | 99.756 |
| pdfplumber | 13 | 0.897 | 0.885 | 0.889 | 89.107 | 90.741 |
| camelot_auto | 13 | 0.974 | 0.846 | 0.866 | 89.907 | 87.127 |
| camelot_lattice | 13 | 0.821 | 0.808 | 0.817 | 81.603 | 77.741 |
| pymupdf | 13 | 0.897 | 0.885 | 0.852 | 87.586 | 89.448 |
| img2table | 13 | 0.821 | 0.808 | 0.808 | 81.244 | 77.436 |
| pdfminer.six | 13 | 0.000 | 0.000 | 0.000 | 0.000 | 15.000 |
| pypdf | 13 | 0.000 | 0.000 | 0.000 | 0.000 | 15.000 |
| pypdfium2 | 13 | 0.000 | 0.000 | 0.000 | 0.000 | 15.000 |

## Hard suite — per-doc table detect F1

| Doc | pdfparser | camelot_lattice | camelot_auto | img2table | pdfplumber | pymupdf | pdfminer.six | pypdf | pypdfium2 |
|-----|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|
| `50_multi_table_stacked_page` | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `51_multi_table_multipage` | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `52_page_border_noise` | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `53_column_span_header` | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `54_row_span_categories` | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `55_gap_broken_corners` | 1.00 | 0.00 | 1.00 | 0.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `56_stacked_uneven_tables` | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `57_wide_statistical_grid` | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `58_multipage_continued_table` | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `59_stream_multi_region` | 1.00 | 0.00 | 1.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 |
| `60_mixed_lattice_stream_page` | 1.00 | 0.67 | 0.67 | 0.67 | 0.67 | 0.67 | 0.00 | 0.00 | 0.00 |
| `61_decorative_hline_noise` | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |
| `62_two_close_grids` | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 0.00 | 0.00 | 0.00 |

## Hard suite — per-doc table cell F1

| Doc | pdfparser | camelot_lattice | camelot_auto | img2table | pdfplumber | pymupdf | pdfminer.six | pypdf | pypdfium2 |
|-----|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|
| `50_multi_table_stacked_page` | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0.972 | 0.000 | 0.000 | 0.000 |
| `51_multi_table_multipage` | 1.000 | 1.000 | 1.000 | 1.000 | 0.991 | 0.953 | 0.000 | 0.000 | 0.000 |
| `52_page_border_noise` | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.974 | 0.000 | 0.000 | 0.000 |
| `53_column_span_header` | 0.973 | 1.000 | 1.000 | 0.950 | 1.000 | 0.930 | 0.000 | 0.000 | 0.000 |
| `54_row_span_categories` | 0.943 | 1.000 | 1.000 | 0.933 | 1.000 | 0.902 | 0.000 | 0.000 | 0.000 |
| `55_gap_broken_corners` | 1.000 | 0.000 | 1.000 | 0.000 | 1.000 | 0.947 | 0.000 | 0.000 | 0.000 |
| `56_stacked_uneven_tables` | 1.000 | 1.000 | 1.000 | 1.000 | 0.985 | 0.954 | 0.000 | 0.000 | 0.000 |
| `57_wide_statistical_grid` | 0.990 | 1.000 | 1.000 | 1.000 | 1.000 | 0.990 | 0.000 | 0.000 | 0.000 |
| `58_multipage_continued_table` | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0.987 | 0.000 | 0.000 | 0.000 |
| `59_stream_multi_region` | 1.000 | 0.000 | 0.636 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| `60_mixed_lattice_stream_page` | 1.000 | 0.625 | 0.625 | 0.625 | 0.625 | 0.596 | 0.000 | 0.000 | 0.000 |
| `61_decorative_hline_noise` | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0.941 | 0.000 | 0.000 | 0.000 |
| `62_two_close_grids` | 1.000 | 1.000 | 1.000 | 1.000 | 0.963 | 0.923 | 0.000 | 0.000 | 0.000 |

## How to re-run

```bash
cargo build --release -p pdfparser-cli
source .venv/bin/activate
python benchmark/scripts/run_accuracy_benchmark.py --suite regression
python benchmark/scripts/run_accuracy_benchmark.py --suite regression_hard
```

Competitive ICDAR (external, not this scoreboard): see `docs/camelot-comparison-replication.md`.
