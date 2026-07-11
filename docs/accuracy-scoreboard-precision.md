# PDF Parser Accuracy Scoreboard

**Suite:** `regression_precision` · **Docs:** 13 · **ICDAR excluded** (competitive-only)

## Executive summary — table quality

Primary ranking for table-engine work (mean over docs that produce table metrics):

| Rank | Library | cell F1 | detect F1 | shape exact | overall | ms |
|-----:|---------|--------:|----------:|------------:|--------:|---:|
| 1 | **pdfparser** ← **ours** | 1.000 | 1.000 | 1.000 | 99.423 | 7.4 |
| 2 | **camelot_lattice** | 0.900 | 0.897 | 0.833 | 83.233 | 34.0 |
| 3 | **camelot_auto** | 0.900 | 0.897 | 0.833 | 81.291 | 392.3 |
| 4 | **pdfplumber** | 0.894 | 0.897 | 0.833 | 89.780 | 7.9 |
| 5 | **pymupdf** | 0.835 | 0.897 | 0.833 | 87.919 | 9.9 |
| 6 | **img2table** | 0.800 | 0.821 | 0.833 | 77.026 | 69.6 |
| 7 | **camelot_stream** | 0.675 | 0.833 | 0.667 | 69.067 | 2.5 |
| 8 | **pdfminer.six** | 0.000 | 0.077 | 0.000 | 20.962 | 6.8 |
| 9 | **pypdf** | 0.000 | 0.077 | 0.000 | 20.962 | 2.9 |
| 10 | **pypdfium2** | 0.000 | 0.077 | 0.000 | 20.962 | 0.8 |

### Overall product score (text+tables+objects)

| Rank | Library | overall | text F1 | table cell F1 | ms |
|-----:|---------|--------:|--------:|--------------:|---:|
| 1 | **pdfparser** ← **ours** | 99.423 | 0.962 | 1.000 | 7.4 |
| 2 | **pdfplumber** | 89.780 | 0.962 | 0.894 | 7.9 |
| 3 | **pymupdf** | 87.919 | 0.962 | 0.835 | 9.9 |
| 4 | **camelot_lattice** | 83.233 | 0.513 | 0.900 | 34.0 |
| 5 | **camelot_auto** | 81.291 | 0.667 | 0.900 | 392.3 |
| 6 | **img2table** | 77.026 | 0.462 | 0.800 | 69.6 |
| 7 | **camelot_stream** | 69.067 | 0.667 | 0.675 | 2.5 |
| 8 | **pdfminer.six** | 20.962 | 0.962 | 0.000 | 6.8 |
| 9 | **pypdf** | 20.962 | 0.962 | 0.000 | 2.9 |
| 10 | **pypdfium2** | 20.962 | 0.962 | 0.000 | 0.8 |

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
| pdfparser | 99.423 | 96.154 | 0.962 | — | 100.000 | 1.000 | 1.000 | 100.000 | 7.4 |
| pdfplumber | 89.780 | 96.154 | 0.962 | — | 88.655 | 0.897 | 0.894 | 100.000 | 7.9 |
| pymupdf | 87.919 | 96.154 | 0.962 | — | 86.465 | 0.897 | 0.835 | 100.000 | 9.9 |
| camelot_lattice | 83.233 | 51.282 | 0.513 | — | 88.872 | 0.897 | 0.900 | — | 34.0 |
| camelot_auto | 81.291 | 66.667 | 0.667 | — | 83.872 | 0.897 | 0.900 | — | 392.3 |
| img2table | 77.026 | 46.154 | 0.462 | — | 82.474 | 0.821 | 0.800 | — | 69.6 |
| camelot_stream | 69.067 | 66.667 | 0.667 | — | 69.490 | 0.833 | 0.675 | — | 2.5 |
| pdfminer.six | 20.962 | 96.154 | 0.962 | — | 7.692 | 0.077 | 0.000 | 100.000 | 6.8 |
| pypdf | 20.962 | 96.154 | 0.962 | — | 7.692 | 0.077 | 0.000 | 100.000 | 2.9 |
| pypdfium2 | 20.962 | 96.154 | 0.962 | — | 7.692 | 0.077 | 0.000 | — | 0.8 |

## Grid-gold subset (synthetic tables with full cell grids)
Best apples-to-apples table quality comparison.

| Library | n | detect F1 | shape exact | cell F1 | table score |
|---------|--:|----------:|------------:|--------:|------------:|
| pdfparser | 12 | 1.000 | 1.000 | 1.000 | 100.000 |
| pdfplumber | 12 | 0.889 | 0.833 | 0.894 | 87.709 |
| pymupdf | 12 | 0.889 | 0.833 | 0.835 | 85.338 |
| camelot_lattice | 12 | 0.889 | 0.833 | 0.900 | 87.944 |
| camelot_auto | 12 | 0.972 | 0.833 | 0.900 | 90.861 |
| img2table | 12 | 0.806 | 0.833 | 0.800 | 81.013 |
| camelot_stream | 12 | 0.903 | 0.667 | 0.675 | 75.281 |
| pdfminer.six | 12 | 0.000 | 0.000 | 0.000 | 0.000 |
| pypdf | 12 | 0.000 | 0.000 | 0.000 | 0.000 |
| pypdfium2 | 12 | 0.000 | 0.000 | 0.000 | 0.000 |

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

### real

| Library | n | overall | text | table | detect F1 | cell F1 | shape exact | objects |
|---------|--:|--------:|-----:|------:|----------:|--------:|------------:|--------:|

## Per-document matrix (overall score)

| Doc | camelot_auto | camelot_lattice | camelot_stream | img2table | pdfminer.six | pdfparser | pdfplumber | pymupdf | pypdf | pypdfium2 |
|-----|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|
| `70_deco_rules_one_table` | 95.0 | 95.0 | 95.0 | 95.0 | 15.0 | 100.0 | 100.0 | 97.6 | 15.0 | 15.0 |
| `71_frame_and_table` | 44.8 | 92.5 | 92.5 | 92.5 | 15.0 | 100.0 | 100.0 | 98.8 | 15.0 | 15.0 |
| `72_two_tables_margin_ticks` | 95.0 | 95.0 | 34.8 | 95.0 | 15.0 | 100.0 | 98.5 | 96.8 | 15.0 | 15.0 |
| `73_false_row_underlines` | 95.0 | 95.0 | 95.0 | 95.0 | 15.0 | 100.0 | 100.0 | 95.1 | 15.0 | 15.0 |
| `74_sparse_data_grid` | 85.0 | 85.0 | 85.0 | 74.2 | 7.5 | 92.5 | 92.5 | 92.5 | 7.5 | 7.5 |
| `75_prose_not_stream` | 15.0 | 85.0 | 15.0 | 85.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |
| `76_caption_not_table` | 85.1 | 85.1 | 90.0 | 85.1 | 15.0 | 100.0 | 90.1 | 87.8 | 15.0 | 15.0 |
| `77_three_stacked_disjoint` | 96.2 | 96.2 | 29.9 | 96.2 | 15.0 | 100.0 | 99.4 | 97.3 | 15.0 | 15.0 |
| `78_multipage_footer_rules` | 95.0 | 95.0 | 95.0 | 94.8 | 15.0 | 100.0 | 99.8 | 99.4 | 15.0 | 15.0 |
| `79_span_header_precision` | 96.2 | 96.2 | 48.3 | 93.5 | 15.0 | 100.0 | 100.0 | 96.7 | 15.0 | 15.0 |
| `80_form_boxes_above_table` | 95.0 | 95.0 | 95.0 | 95.0 | 15.0 | 100.0 | 100.0 | 97.6 | 15.0 | 15.0 |
| `81_phantom_verticals` | 67.0 | 67.0 | 95.0 | 0.0 | 15.0 | 100.0 | 72.0 | 68.4 | 15.0 | 15.0 |
| `82_stream_gap_boundary` | 92.5 | 0.0 | 27.3 | 0.0 | 15.0 | 100.0 | 15.0 | 15.0 | 15.0 | 15.0 |

## Per-document table cell F1 (grid gold only)

| Doc | camelot_auto | camelot_lattice | camelot_stream | img2table | pdfminer.six | pdfparser | pdfplumber | pymupdf | pypdf | pypdfium2 |
|-----|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|-----:|
| `70_deco_rules_one_table` | 1.000 | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.929 | 0.000 | 0.000 |
| `71_frame_and_table` | 0.000 | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.966 | 0.000 | 0.000 |
| `72_two_tables_margin_ticks` | 1.000 | 1.000 | 0.000 | 1.000 | 0.000 | 1.000 | 0.955 | 0.905 | 0.000 | 0.000 |
| `73_false_row_underlines` | 1.000 | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.857 | 0.000 | 0.000 |
| `74_sparse_data_grid` | 1.000 | 1.000 | 1.000 | 0.683 | 0.000 | 1.000 | 1.000 | 1.000 | 0.000 | 0.000 |
| `76_caption_not_table` | 1.000 | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.933 | 0.000 | 0.000 |
| `77_three_stacked_disjoint` | 1.000 | 1.000 | 0.000 | 1.000 | 0.000 | 1.000 | 0.981 | 0.920 | 0.000 | 0.000 |
| `78_multipage_footer_rules` | 1.000 | 1.000 | 1.000 | 0.994 | 0.000 | 1.000 | 0.994 | 0.981 | 0.000 | 0.000 |
| `79_span_header_precision` | 1.000 | 1.000 | 0.105 | 0.919 | 0.000 | 1.000 | 1.000 | 0.903 | 0.000 | 0.000 |
| `80_form_boxes_above_table` | 1.000 | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.929 | 0.000 | 0.000 |
| `81_phantom_verticals` | 0.800 | 0.800 | 1.000 | 0.000 | 0.000 | 1.000 | 0.800 | 0.696 | 0.000 | 0.000 |
| `82_stream_gap_boundary` | 1.000 | 0.000 | 0.000 | 0.000 | 0.000 | 1.000 | 0.000 | 0.000 | 0.000 | 0.000 |

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

- **Suite:** `regression_precision` (ICDAR never included in regression)
- **Documents scored:** 13
- **Libraries:** pdfparser, camelot_lattice, camelot_stream, camelot_auto, img2table, pdfplumber, pymupdf, pdfminer.six, pypdf, pypdfium2
- **Note:** `camelot_*` / `img2table` / `tabula` are table-primary; text scores use cell-concatenated text only (outside-table tokens may miss).
- **Tabula:** skipped when no working JRE is installed.

## Table-quality leaderboard (mean over docs with table gold)

| Rank | Library | detect F1 | shape exact | cell F1 | table score | ms |
|-----:|---------|----------:|------------:|--------:|------------:|---:|
| 1 | **pdfparser** | 1.000 | 1.000 | 1.000 | 100.000 | 7.4 |
| 2 | **camelot_lattice** | 0.897 | 0.833 | 0.900 | 88.872 | 34.0 |
| 3 | **camelot_auto** | 0.897 | 0.833 | 0.900 | 83.872 | 392.3 |
| 4 | **pdfplumber** | 0.897 | 0.833 | 0.894 | 88.655 | 7.9 |
| 5 | **pymupdf** | 0.897 | 0.833 | 0.835 | 86.465 | 9.9 |
| 6 | **img2table** | 0.821 | 0.833 | 0.800 | 82.474 | 69.6 |
| 7 | **camelot_stream** | 0.833 | 0.667 | 0.675 | 69.490 | 2.5 |
| 8 | **pdfminer.six** | 0.077 | 0.000 | 0.000 | 7.692 | 6.8 |
| 9 | **pypdf** | 0.077 | 0.000 | 0.000 | 7.692 | 2.9 |
| 10 | **pypdfium2** | 0.077 | 0.000 | 0.000 | 7.692 | 0.8 |

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
