# PDF Parser Accuracy Scoreboard

**Suite:** `regression_compete` · **Docs:** 65 · **ICDAR excluded** (competitive-only)

## Executive summary — table quality

Primary ranking for table-engine work (mean over docs that produce table metrics):

| Rank | Library | cell F1 | detect F1 | shape exact | overall | ms |
|-----:|---------|--------:|----------:|------------:|--------:|---:|
| 1 | **pdfparser** ← **ours** | 0.908 | 0.951 | 0.908 | 92.509 | 4.9 |

### Overall product score (text+tables+objects)

| Rank | Library | overall | text F1 | table cell F1 | ms |
|-----:|---------|--------:|--------:|--------------:|---:|
| 1 | **pdfparser** ← **ours** | 92.509 | 0.946 | 0.908 | 4.9 |

> Table-primary tools (`camelot_*`, `img2table`) get lower **overall** when outside-table tokens are missing — judge them on **cell F1 / detect F1**.

---

**Generated:** auto · **Docs scored:** 65 · **Libraries:** pdfparser

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
| pdfparser | 92.509 | 94.615 | 0.946 | — | 92.275 | 0.951 | 0.908 | 100.000 | 4.9 |

## Grid-gold subset (synthetic tables with full cell grids)
Best apples-to-apples table quality comparison.

| Library | n | detect F1 | shape exact | cell F1 | table score |
|---------|--:|----------:|------------:|--------:|------------:|
| pdfparser | 65 | 0.951 | 0.908 | 0.908 | 92.275 |

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

| Doc | pdfparser |
|-----|-----:|
| `C001_overdetect_deco_h3` | 100.0 |
| `C002_overdetect_deco_h6` | 100.0 |
| `C003_overdetect_deco_h10` | 100.0 |
| `C004_overdetect_deco_h15` | 100.0 |
| `C009_overdetect_stream_bait_n8` | 100.0 |
| `C010_overdetect_stream_bait_n12` | 100.0 |
| `C011_overdetect_stream_bait_n16` | 100.0 |
| `C012_overdetect_stream_bait_n20` | 100.0 |
| `C013_painted_rect_5x4` | 100.0 |
| `C014_painted_rect_6x5` | 100.0 |
| `C015_painted_rect_8x4` | 100.0 |
| `C016_painted_rect_10x6` | 100.0 |
| `C017_painted_rect_7x3` | 100.0 |
| `C018_painted_rect_12x5` | 100.0 |
| `C019_partial_h_step2_12x5` | 100.0 |
| `C020_partial_h_step3_12x6` | 100.0 |
| `C021_partial_h_step4_12x5` | 100.0 |
| `C022_partial_h_step2_12x6` | 100.0 |
| `C023_partial_h_step3_12x5` | 100.0 |
| `C024_partial_h_step4_12x6` | 100.0 |
| `C025_borderless_20x6` | 96.7 |
| `C026_borderless_25x8` | 96.7 |
| `C027_borderless_30x8_gap` | 97.5 |
| `C028_borderless_35x10_gap` | 97.5 |
| `C029_borderless_40x6` | 96.7 |
| `C030_borderless_28x12_gap` | 97.4 |
| `C031_borderless_22x5` | 96.7 |
| `C032_borderless_32x7_gap` | 97.5 |
| `C033_borderless_18x9` | 96.7 |
| `C034_borderless_45x5_gap` | 97.5 |
| `C035_borderless_26x8` | 96.7 |
| `C036_borderless_38x8_gap` | 97.5 |
| `C037_wide_8x10` | 100.0 |
| `C038_wide_10x12` | 100.0 |
| `C039_wide_12x14` | 100.0 |
| `C040_wide_6x16` | 100.0 |
| `C041_wide_15x10` | 100.0 |
| `C042_wide_20x8` | 100.0 |
| `C043_false_underlines_x5` | 100.0 |
| `C044_false_underlines_x10` | 100.0 |
| `C045_false_underlines_x15` | 100.0 |
| `C046_false_underlines_x20` | 100.0 |
| `C047_multitable_n2` | 31.0 |
| `C048_multitable_n3` | 25.8 |
| `C049_multitable_n4` | 22.6 |
| `C050_multitable_n5` | 20.5 |
| `C051_multitable_n3` | 25.8 |
| `C052_multitable_n4` | 22.6 |
| `C053_multipage_p2` | 100.0 |
| `C054_multipage_p3` | 100.0 |
| `C055_multipage_p4` | 100.0 |
| `C056_multipage_p2` | 100.0 |
| `C057_invoice_items3` | 100.0 |
| `C058_invoice_items5` | 100.0 |
| `C059_invoice_items8` | 100.0 |
| `C060_invoice_items4` | 100.0 |
| `C061_mixed_lat5_str6` | 100.0 |
| `C062_mixed_lat6_str8` | 100.0 |
| `C063_mixed_lat4_str10` | 100.0 |
| `C064_mixed_lat7_str5` | 100.0 |
| `C065_span_header_v1` | 100.0 |
| `C066_span_header_v2` | 100.0 |
| `C067_span_header_v3` | 100.0 |
| `C068_span_header_v4` | 100.0 |
| `C069_partial_h_sparse_fill_15x5` | 100.0 |

## Per-document table cell F1 (grid gold only)

| Doc | pdfparser |
|-----|-----:|
| `C001_overdetect_deco_h3` | 1.000 |
| `C002_overdetect_deco_h6` | 1.000 |
| `C003_overdetect_deco_h10` | 1.000 |
| `C004_overdetect_deco_h15` | 1.000 |
| `C009_overdetect_stream_bait_n8` | 1.000 |
| `C010_overdetect_stream_bait_n12` | 1.000 |
| `C011_overdetect_stream_bait_n16` | 1.000 |
| `C012_overdetect_stream_bait_n20` | 1.000 |
| `C013_painted_rect_5x4` | 1.000 |
| `C014_painted_rect_6x5` | 1.000 |
| `C015_painted_rect_8x4` | 1.000 |
| `C016_painted_rect_10x6` | 1.000 |
| `C017_painted_rect_7x3` | 1.000 |
| `C018_painted_rect_12x5` | 1.000 |
| `C019_partial_h_step2_12x5` | 1.000 |
| `C020_partial_h_step3_12x6` | 1.000 |
| `C021_partial_h_step4_12x5` | 1.000 |
| `C022_partial_h_step2_12x6` | 1.000 |
| `C023_partial_h_step3_12x5` | 1.000 |
| `C024_partial_h_step4_12x6` | 1.000 |
| `C025_borderless_20x6` | 1.000 |
| `C026_borderless_25x8` | 1.000 |
| `C027_borderless_30x8_gap` | 1.000 |
| `C028_borderless_35x10_gap` | 1.000 |
| `C029_borderless_40x6` | 1.000 |
| `C030_borderless_28x12_gap` | 0.997 |
| `C031_borderless_22x5` | 1.000 |
| `C032_borderless_32x7_gap` | 1.000 |
| `C033_borderless_18x9` | 1.000 |
| `C034_borderless_45x5_gap` | 1.000 |
| `C035_borderless_26x8` | 1.000 |
| `C036_borderless_38x8_gap` | 1.000 |
| `C037_wide_8x10` | 1.000 |
| `C038_wide_10x12` | 1.000 |
| `C039_wide_12x14` | 1.000 |
| `C040_wide_6x16` | 1.000 |
| `C041_wide_15x10` | 1.000 |
| `C042_wide_20x8` | 1.000 |
| `C043_false_underlines_x5` | 1.000 |
| `C044_false_underlines_x10` | 1.000 |
| `C045_false_underlines_x15` | 1.000 |
| `C046_false_underlines_x20` | 1.000 |
| `C047_multitable_n2` | 0.000 |
| `C048_multitable_n3` | 0.000 |
| `C049_multitable_n4` | 0.000 |
| `C050_multitable_n5` | 0.000 |
| `C051_multitable_n3` | 0.000 |
| `C052_multitable_n4` | 0.000 |
| `C053_multipage_p2` | 1.000 |
| `C054_multipage_p3` | 1.000 |
| `C055_multipage_p4` | 1.000 |
| `C056_multipage_p2` | 1.000 |
| `C057_invoice_items3` | 1.000 |
| `C058_invoice_items5` | 1.000 |
| `C059_invoice_items8` | 1.000 |
| `C060_invoice_items4` | 1.000 |
| `C061_mixed_lat5_str6` | 1.000 |
| `C062_mixed_lat6_str8` | 1.000 |
| `C063_mixed_lat4_str10` | 1.000 |
| `C064_mixed_lat7_str5` | 1.000 |
| `C065_span_header_v1` | 1.000 |
| `C066_span_header_v2` | 1.000 |
| `C067_span_header_v3` | 1.000 |
| `C068_span_header_v4` | 1.000 |
| `C069_partial_h_sparse_fill_15x5` | 1.000 |

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

- **Suite:** `regression_compete` (ICDAR never included in regression)
- **Documents scored:** 65
- **Libraries:** pdfparser
- **Note:** `camelot_*` / `img2table` / `tabula` are table-primary; text scores use cell-concatenated text only (outside-table tokens may miss).
- **Tabula:** skipped when no working JRE is installed.

## Table-quality leaderboard (mean over docs with table gold)

| Rank | Library | detect F1 | shape exact | cell F1 | table score | ms |
|-----:|---------|----------:|------------:|--------:|------------:|---:|
| 1 | **pdfparser** | 0.951 | 0.908 | 0.908 | 92.275 | 4.9 |

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
