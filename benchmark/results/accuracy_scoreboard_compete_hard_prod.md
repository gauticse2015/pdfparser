# Compete-hard accuracy scoreboard (post-geometry re-baseline)

**Suite:** `regression_compete_hard` · **Docs:** 81 · **ICDAR excluded**  
**Freeze:** `benchmark/results/compete_hard_baseline_post_geometry.json`  
**Prior freezes:** pre-algorithm `compete_hard_baseline_frozen.json` · post-raster `compete_hard_baseline_post_raster.json`

## Re-baseline summary (pdfparser)

| Metric | Pre-algorithm | Post-raster | **Post-geometry** |
|--------|-------------:|------------:|------------------:|
| overall | 50.4 | 61.5 | **88.2** |
| cell F1 | 0.383 | 0.445 | **0.830** |
| detect F1 | 0.795 | 0.884 | **0.961** |
| shape exact | 0.204 | 0.444 | **0.846** |
| row accuracy | 0.426 | 0.667 | **0.846** |
| col accuracy | 0.596 | 0.608 | **0.966** |
| imperfect rate | 87.7% | 74.1% | **35.8%** |

### Geometry lanes landed (parallel)

| Lane | Failure mode | Result |
|------|--------------|--------|
| A | partial-V densify_x (C116–C133) | **solved** cell/shape 1.0 |
| B | sparse densify_y (C142–C149) | **solved** cell/shape 1.0 |
| C | header-slice network regioning (C108–C115) | det 1.0, col 1.0, cell **0.63**, shape 0.50 |
| D | mixed lattice+stream (via C network bridge) | cell **0.98**, shape 0.92 |

### By struggle mode

| Mode | n | det F1 | shape | row | col | cell F1 | imperfect | overall |
|------|--:|-------:|------:|----:|----:|--------:|----------:|--------:|
| `image_painted_miss_all` | 8 | 1.000 | 1.000 | 1.000 | 1.000 | 0.000 | 100% | 62.0 |
| `complex_spans` | 4 | 0.833 | 0.250 | 0.250 | 0.500 | 0.474 | 75% | 56.7 |
| `header_slice_fragmentation` | 8 | 1.000 | 0.500 | 0.500 | 1.000 | 0.632 | 100% | 74.2 |
| `multi_table_close_chrome` | 5 | 0.880 | 0.800 | 0.800 | 0.850 | 0.887 | 20% | 87.0 |
| `mixed_stream_miss` | 6 | 1.000 | 0.917 | 0.917 | 1.000 | 0.977 | 17% | 93.8 |
| `invoice_fees_inside_grid` | 4 | 1.000 | 0.000 | 0.000 | 1.000 | 1.000 | 100% | 76.2 |
| `cell_content_assign` | 4 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0% | 97.5 |
| `false_underline_row_overcount` | 8 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0% | 100.0 |
| `multipage_underline_overcount` | 4 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0% | 100.0 |
| `partial_v_col_undercount` | 18 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0% | 100.0 |
| `severe_minigrid_overdetect` | 4 | 0.528 | 1.000 | 1.000 | 1.000 | 1.000 | 100% | 83.7 |
| `sparse_densify_row_undercount` | 8 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0% | 100.0 |

---

## Executive summary — table quality

Primary ranking for table-engine work (mean over docs that produce table metrics):

| Rank | Library | cell F1 | detect F1 | shape exact | overall | ms |
|-----:|---------|--------:|----------:|------------:|--------:|---:|
| 1 | **pdfparser** ← **ours** | 0.830 | 0.961 | 0.846 | 88.189 | 10.9 |

### Overall product score (text+tables+objects)

| Rank | Library | overall | text F1 | table cell F1 | ms |
|-----:|---------|--------:|--------:|--------------:|---:|
| 1 | **pdfparser** ← **ours** | 88.189 | 0.920 | 0.830 | 10.9 |

> Table-primary tools (`camelot_*`, `img2table`) get lower **overall** when outside-table tokens are missing — judge them on **cell F1 / detect F1**.

---

**Generated:** auto · **Docs scored:** 81 · **Libraries:** pdfparser

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
| pdfparser | 88.189 | 91.975 | 0.920 | — | 87.990 | 0.961 | 0.830 | 90.124 | 10.9 |

## Grid-gold subset (synthetic tables with full cell grids)
Best apples-to-apples table quality comparison.

| Library | n | detect F1 | shape exact | cell F1 | table score |
|---------|--:|----------:|------------:|--------:|------------:|
| pdfparser | 81 | 0.961 | 0.846 | 0.830 | 87.990 |

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
| `C100_img_rules_8x4` | 62.0 |
| `C101_img_rules_10x5` | 62.0 |
| `C102_img_rules_12x6` | 62.0 |
| `C103_img_rules_15x6` | 62.0 |
| `C104_img_rules_21x6` | 62.0 |
| `C105_img_rules_12x8` | 62.0 |
| `C106_img_rules_18x5` | 62.0 |
| `C107_img_rules_9x7` | 62.0 |
| `C108_hdr_slice_31x10` | 38.5 |
| `C109_hdr_slice_44x12` | 73.1 |
| `C110_hdr_slice_58x8` | 99.6 |
| `C111_hdr_slice_35x11` | 96.9 |
| `C112_hdr_slice_40x9` | 99.2 |
| `C113_hdr_slice_50x10` | 38.4 |
| `C114_hdr_slice_28x14` | 97.6 |
| `C115_hdr_slice_36x8` | 49.8 |
| `C116_partial_v_s2_12x10` | 100.0 |
| `C117_partial_v_s2_15x12` | 100.0 |
| `C118_partial_v_s2_20x12` | 100.0 |
| `C119_partial_v_s2_15x14` | 100.0 |
| `C120_partial_v_s2_10x16` | 100.0 |
| `C121_partial_v_s2_18x10` | 100.0 |
| `C122_partial_v_s3_12x10` | 100.0 |
| `C123_partial_v_s3_15x12` | 100.0 |
| `C124_partial_v_s3_20x12` | 100.0 |
| `C125_partial_v_s3_15x14` | 100.0 |
| `C126_partial_v_s3_10x16` | 100.0 |
| `C127_partial_v_s3_18x10` | 100.0 |
| `C128_partial_v_s4_12x10` | 100.0 |
| `C129_partial_v_s4_15x12` | 100.0 |
| `C130_partial_v_s4_20x12` | 100.0 |
| `C131_partial_v_s4_15x14` | 100.0 |
| `C132_partial_v_s4_10x16` | 100.0 |
| `C133_partial_v_s4_18x10` | 100.0 |
| `C134_cell_underlines_6x4` | 100.0 |
| `C135_cell_underlines_8x5` | 100.0 |
| `C136_cell_underlines_10x5` | 100.0 |
| `C137_cell_underlines_12x6` | 100.0 |
| `C138_cell_underlines_7x4` | 100.0 |
| `C139_cell_underlines_9x5` | 100.0 |
| `C140_cell_underlines_14x4` | 100.0 |
| `C141_cell_underlines_11x6` | 100.0 |
| `C142_sparse_f25_20x6` | 100.0 |
| `C143_sparse_f25_25x8` | 100.0 |
| `C144_sparse_f35_20x6` | 100.0 |
| `C145_sparse_f35_25x8` | 100.0 |
| `C146_sparse_f50_20x6` | 100.0 |
| `C147_sparse_f50_28x7` | 100.0 |
| `C148_sparse_f30_30x5` | 100.0 |
| `C149_sparse_f40_18x9` | 100.0 |
| `C150_mix_irreg_str20x6` | 96.7 |
| `C151_mix_irreg_str30x6` | 79.6 |
| `C152_mix_irreg_str40x7` | 96.7 |
| `C153_mix_irreg_str25x8` | 96.7 |
| `C154_mix_irreg_str35x5` | 96.7 |
| `C155_mix_irreg_str22x9` | 96.7 |
| `C156_multi_close_n3_g6` | 100.0 |
| `C157_multi_close_n4_g8` | 100.0 |
| `C158_multi_close_n5_g10` | 100.0 |
| `C159_multi_close_n4_g5` | 34.8 |
| `C160_multi_close_n6_g12` | 100.0 |
| `C161_mp_chaos_p2_s3` | 100.0 |
| `C162_mp_chaos_p3_s3` | 100.0 |
| `C163_mp_chaos_p4_s4` | 100.0 |
| `C164_mp_chaos_p3_s2` | 100.0 |
| `C165_span_hard_v1e0` | 72.3 |
| `C166_span_hard_v1e1` | 27.2 |
| `C167_span_hard_v2e0` | 100.0 |
| `C168_span_hard_v2e1` | 27.2 |
| `C169_invoice_hard_i5_f4` | 76.2 |
| `C170_invoice_hard_i8_f5` | 76.2 |
| `C171_invoice_hard_i12_f6` | 76.2 |
| `C172_invoice_hard_i6_f3` | 76.2 |
| `C173_severe_overdetect_n8` | 88.9 |
| `C174_severe_overdetect_n16` | 88.9 |
| `C175_severe_overdetect_n24` | 88.9 |
| `C176_severe_overdetect_n12` | 67.9 |
| `C177_cell_assign_hard_v1` | 97.5 |
| `C178_cell_assign_hard_v2` | 97.5 |
| `C179_cell_assign_hard_v3` | 97.5 |
| `C180_cell_assign_hard_v4` | 97.5 |

## Per-document table cell F1 (grid gold only)

| Doc | pdfparser |
|-----|-----:|
| `C100_img_rules_8x4` | 0.000 |
| `C101_img_rules_10x5` | 0.000 |
| `C102_img_rules_12x6` | 0.000 |
| `C103_img_rules_15x6` | 0.000 |
| `C104_img_rules_21x6` | 0.000 |
| `C105_img_rules_12x8` | 0.000 |
| `C106_img_rules_18x5` | 0.000 |
| `C107_img_rules_9x7` | 0.000 |
| `C108_hdr_slice_31x10` | 0.007 |
| `C109_hdr_slice_44x12` | 0.917 |
| `C110_hdr_slice_58x8` | 0.990 |
| `C111_hdr_slice_35x11` | 0.919 |
| `C112_hdr_slice_40x9` | 0.980 |
| `C113_hdr_slice_50x10` | 0.004 |
| `C114_hdr_slice_28x14` | 0.938 |
| `C115_hdr_slice_36x8` | 0.303 |
| `C116_partial_v_s2_12x10` | 1.000 |
| `C117_partial_v_s2_15x12` | 1.000 |
| `C118_partial_v_s2_20x12` | 1.000 |
| `C119_partial_v_s2_15x14` | 1.000 |
| `C120_partial_v_s2_10x16` | 1.000 |
| `C121_partial_v_s2_18x10` | 1.000 |
| `C122_partial_v_s3_12x10` | 1.000 |
| `C123_partial_v_s3_15x12` | 1.000 |
| `C124_partial_v_s3_20x12` | 1.000 |
| `C125_partial_v_s3_15x14` | 1.000 |
| `C126_partial_v_s3_10x16` | 1.000 |
| `C127_partial_v_s3_18x10` | 1.000 |
| `C128_partial_v_s4_12x10` | 1.000 |
| `C129_partial_v_s4_15x12` | 1.000 |
| `C130_partial_v_s4_20x12` | 1.000 |
| `C131_partial_v_s4_15x14` | 1.000 |
| `C132_partial_v_s4_10x16` | 1.000 |
| `C133_partial_v_s4_18x10` | 1.000 |
| `C134_cell_underlines_6x4` | 1.000 |
| `C135_cell_underlines_8x5` | 1.000 |
| `C136_cell_underlines_10x5` | 1.000 |
| `C137_cell_underlines_12x6` | 1.000 |
| `C138_cell_underlines_7x4` | 1.000 |
| `C139_cell_underlines_9x5` | 1.000 |
| `C140_cell_underlines_14x4` | 1.000 |
| `C141_cell_underlines_11x6` | 1.000 |
| `C142_sparse_f25_20x6` | 1.000 |
| `C143_sparse_f25_25x8` | 1.000 |
| `C144_sparse_f35_20x6` | 1.000 |
| `C145_sparse_f35_25x8` | 1.000 |
| `C146_sparse_f50_20x6` | 1.000 |
| `C147_sparse_f50_28x7` | 1.000 |
| `C148_sparse_f30_30x5` | 1.000 |
| `C149_sparse_f40_18x9` | 1.000 |
| `C150_mix_irreg_str20x6` | 1.000 |
| `C151_mix_irreg_str30x6` | 0.865 |
| `C152_mix_irreg_str40x7` | 1.000 |
| `C153_mix_irreg_str25x8` | 1.000 |
| `C154_mix_irreg_str35x5` | 1.000 |
| `C155_mix_irreg_str22x9` | 1.000 |
| `C156_multi_close_n3_g6` | 1.000 |
| `C157_multi_close_n4_g8` | 1.000 |
| `C158_multi_close_n5_g10` | 1.000 |
| `C159_multi_close_n4_g5` | 0.435 |
| `C160_multi_close_n6_g12` | 1.000 |
| `C161_mp_chaos_p2_s3` | 1.000 |
| `C162_mp_chaos_p3_s3` | 1.000 |
| `C163_mp_chaos_p4_s4` | 1.000 |
| `C164_mp_chaos_p3_s2` | 1.000 |
| `C165_span_hard_v1e0` | 0.895 |
| `C166_span_hard_v1e1` | 0.000 |
| `C167_span_hard_v2e0` | 1.000 |
| `C168_span_hard_v2e1` | 0.000 |
| `C169_invoice_hard_i5_f4` | 1.000 |
| `C170_invoice_hard_i8_f5` | 1.000 |
| `C171_invoice_hard_i12_f6` | 1.000 |
| `C172_invoice_hard_i6_f3` | 1.000 |
| `C173_severe_overdetect_n8` | 1.000 |
| `C174_severe_overdetect_n16` | 1.000 |
| `C175_severe_overdetect_n24` | 1.000 |
| `C176_severe_overdetect_n12` | 1.000 |
| `C177_cell_assign_hard_v1` | 1.000 |
| `C178_cell_assign_hard_v2` | 1.000 |
| `C179_cell_assign_hard_v3` | 1.000 |
| `C180_cell_assign_hard_v4` | 1.000 |

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

- **Suite:** `regression_compete_hard` (ICDAR never included in regression)
- **Documents scored:** 81
- **Libraries:** pdfparser
- **Note:** `camelot_*` / `img2table` / `tabula` are table-primary; text scores use cell-concatenated text only (outside-table tokens may miss).
- **Tabula:** skipped when no working JRE is installed.

## Table-quality leaderboard (mean over docs with table gold)

| Rank | Library | detect F1 | shape exact | cell F1 | table score | ms |
|-----:|---------|----------:|------------:|--------:|------------:|---:|
| 1 | **pdfparser** | 0.961 | 0.846 | 0.830 | 87.990 | 10.9 |

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
