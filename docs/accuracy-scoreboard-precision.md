# PDF Parser Accuracy Scoreboard

**Suite:** `regression_precision` · **Docs:** 13 · **ICDAR excluded** (competitive-only)

## Executive summary — table quality

Primary ranking for table-engine work (mean over docs that produce table metrics):

| Rank | Library | cell F1 | detect F1 | shape exact | overall | ms |
|-----:|---------|--------:|----------:|------------:|--------:|---:|
| 1 | **pdfparser** ← **ours** | 1.000 | 1.000 | 1.000 | 99.423 | 6.8 |
| 2 | **camelot_lattice** | 0.900 | 0.897 | 0.833 | 83.233 | 31.8 |
| 3 | **camelot_auto** | 0.900 | 0.897 | 0.833 | 81.291 | 383.6 |
| 4 | **pdfplumber** | 0.894 | 0.897 | 0.833 | 89.780 | 7.2 |

### Overall product score (text+tables+objects)

| Rank | Library | overall | text F1 | table cell F1 | ms |
|-----:|---------|--------:|--------:|--------------:|---:|
| 1 | **pdfparser** ← **ours** | 99.423 | 0.962 | 1.000 | 6.8 |
| 2 | **pdfplumber** | 89.780 | 0.962 | 0.894 | 7.2 |
| 3 | **camelot_lattice** | 83.233 | 0.513 | 0.900 | 31.8 |
| 4 | **camelot_auto** | 81.291 | 0.667 | 0.900 | 383.6 |

> Table-primary tools (`camelot_*`, `img2table`) get lower **overall** when outside-table tokens are missing — judge them on **cell F1 / detect F1**.

---

**Generated:** auto · **Docs scored:** 13 · **Libraries:** camelot_auto, camelot_lattice, pdfparser, pdfplumber

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
| pdfparser | 99.423 | 96.154 | 0.962 | — | 100.000 | 1.000 | 1.000 | 100.000 | 6.8 |
| pdfplumber | 89.780 | 96.154 | 0.962 | — | 88.655 | 0.897 | 0.894 | 100.000 | 7.2 |
| camelot_lattice | 83.233 | 51.282 | 0.513 | — | 88.872 | 0.897 | 0.900 | — | 31.8 |
| camelot_auto | 81.291 | 66.667 | 0.667 | — | 83.872 | 0.897 | 0.900 | — | 383.6 |

## Grid-gold subset (synthetic tables with full cell grids)
Best apples-to-apples table quality comparison.

| Library | n | detect F1 | shape exact | cell F1 | table score |
|---------|--:|----------:|------------:|--------:|------------:|
| pdfparser | 12 | 1.000 | 1.000 | 1.000 | 100.000 |
| pdfplumber | 12 | 0.889 | 0.833 | 0.894 | 87.709 |
| camelot_lattice | 12 | 0.889 | 0.833 | 0.900 | 87.944 |
| camelot_auto | 12 | 0.972 | 0.833 | 0.900 | 90.861 |

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

| Doc | camelot_auto | camelot_lattice | pdfparser | pdfplumber |
|-----|-----:|-----:|-----:|-----:|
| `70_deco_rules_one_table` | 95.0 | 95.0 | 100.0 | 100.0 |
| `71_frame_and_table` | 44.8 | 92.5 | 100.0 | 100.0 |
| `72_two_tables_margin_ticks` | 95.0 | 95.0 | 100.0 | 98.5 |
| `73_false_row_underlines` | 95.0 | 95.0 | 100.0 | 100.0 |
| `74_sparse_data_grid` | 85.0 | 85.0 | 92.5 | 92.5 |
| `75_prose_not_stream` | 15.0 | 85.0 | 100.0 | 100.0 |
| `76_caption_not_table` | 85.1 | 85.1 | 100.0 | 90.1 |
| `77_three_stacked_disjoint` | 96.2 | 96.2 | 100.0 | 99.4 |
| `78_multipage_footer_rules` | 95.0 | 95.0 | 100.0 | 99.8 |
| `79_span_header_precision` | 96.2 | 96.2 | 100.0 | 100.0 |
| `80_form_boxes_above_table` | 95.0 | 95.0 | 100.0 | 100.0 |
| `81_phantom_verticals` | 67.0 | 67.0 | 100.0 | 72.0 |
| `82_stream_gap_boundary` | 92.5 | 0.0 | 100.0 | 15.0 |

## Per-document table cell F1 (grid gold only)

| Doc | camelot_auto | camelot_lattice | pdfparser | pdfplumber |
|-----|-----:|-----:|-----:|-----:|
| `70_deco_rules_one_table` | 1.000 | 1.000 | 1.000 | 1.000 |
| `71_frame_and_table` | 0.000 | 1.000 | 1.000 | 1.000 |
| `72_two_tables_margin_ticks` | 1.000 | 1.000 | 1.000 | 0.955 |
| `73_false_row_underlines` | 1.000 | 1.000 | 1.000 | 1.000 |
| `74_sparse_data_grid` | 1.000 | 1.000 | 1.000 | 1.000 |
| `76_caption_not_table` | 1.000 | 1.000 | 1.000 | 1.000 |
| `77_three_stacked_disjoint` | 1.000 | 1.000 | 1.000 | 0.981 |
| `78_multipage_footer_rules` | 1.000 | 1.000 | 1.000 | 0.994 |
| `79_span_header_precision` | 1.000 | 1.000 | 1.000 | 1.000 |
| `80_form_boxes_above_table` | 1.000 | 1.000 | 1.000 | 1.000 |
| `81_phantom_verticals` | 0.800 | 0.800 | 1.000 | 0.800 |
| `82_stream_gap_boundary` | 1.000 | 0.000 | 1.000 | 0.000 |

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
- **Libraries:** pdfparser, pdfplumber, camelot_lattice, camelot_auto
- **Note:** `camelot_*` / `img2table` / `tabula` are table-primary; text scores use cell-concatenated text only (outside-table tokens may miss).
- **Tabula:** skipped when no working JRE is installed.

## Table-quality leaderboard (mean over docs with table gold)

| Rank | Library | detect F1 | shape exact | cell F1 | table score | ms |
|-----:|---------|----------:|------------:|--------:|------------:|---:|
| 1 | **pdfparser** | 1.000 | 1.000 | 1.000 | 100.000 | 6.8 |
| 2 | **camelot_lattice** | 0.897 | 0.833 | 0.900 | 88.872 | 31.8 |
| 3 | **camelot_auto** | 0.897 | 0.833 | 0.900 | 83.872 | 383.6 |
| 4 | **pdfplumber** | 0.897 | 0.833 | 0.894 | 88.655 | 7.2 |

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
