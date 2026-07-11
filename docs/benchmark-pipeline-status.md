# Benchmark pipeline status

**Captured:** `2026-07-11T08:39:13.807081+00:00`

Regression suites use **owned synthetic** corpus only. ICDAR-2013 is **external competitive** (not a regression gate).

## Executive summary — pdfparser

| Suite | cell F1 | detect F1 | overall | ms | Rank vs peers |
|-------|--------:|----------:|--------:|---:|---------------|
| **hard** | **1.0000** | **1.0000** | **100.00** | 51.9 | **#1** |
| **precision** | **1.0000** | **1.0000** | **99.42** | 6.8 | **#1** |
| **sensing** | **0.9996** | **1.0000** | **99.99** | 7.7 | **#1** |
| **basic_stress** | **0.9916** | **0.8947** | **94.85** | 10.1 | **#1** |

## Full leaderboards by suite

### Hard structure (50–62)

| Rank | Library | cell F1 | detect F1 | overall | ms |
|-----:|---------|--------:|----------:|--------:|---:|
| 1 | **pdfparser** ← **ours** | 1.0000 | 1.0000 | 100.00 | 51.9 |
| 2 | **pdfplumber** | 0.8895 | 0.8974 | 90.74 | 16.4 |
| 3 | **camelot_auto** | 0.8663 | 0.9744 | 87.13 | 439.1 |
| 4 | **camelot_lattice** | 0.8173 | 0.8205 | 77.74 | 55.9 |

### Precision FP (70–82)

| Rank | Library | cell F1 | detect F1 | overall | ms |
|-----:|---------|--------:|----------:|--------:|---:|
| 1 | **pdfparser** ← **ours** | 1.0000 | 1.0000 | 99.42 | 6.8 |
| 2 | **camelot_lattice** | 0.9000 | 0.8974 | 83.23 | 31.8 |
| 3 | **camelot_auto** | 0.9000 | 0.8974 | 81.29 | 383.6 |
| 4 | **pdfplumber** | 0.8941 | 0.8974 | 89.78 | 7.2 |

### Sensing / capability (90–95)

| Rank | Library | cell F1 | detect F1 | overall | ms |
|-----:|---------|--------:|----------:|--------:|---:|
| 1 | **pdfparser** ← **ours** | 0.9996 | 1.0000 | 99.99 | 7.7 |
| 2 | **camelot_auto** | 0.6993 | 0.9167 | 71.66 | 353.1 |
| 3 | **pdfplumber** | 0.6774 | 0.8333 | 71.89 | 15.9 |
| 4 | **camelot_lattice** | 0.6623 | 0.8333 | 66.70 | 72.9 |

### Basic + stress (01–12, 20–27)

| Rank | Library | cell F1 | detect F1 | overall | ms |
|-----:|---------|--------:|----------:|--------:|---:|
| 1 | **pdfparser** ← **ours** | 0.9916 | 0.8947 | 94.85 | 10.1 |
| 2 | **pdfplumber** | 0.8258 | 0.9333 | 88.11 | 94.2 |
| 3 | **camelot_lattice** | 0.8258 | 0.9333 | 46.89 | 32.0 |
| 4 | **camelot_auto** | 0.8258 | 0.8833 | 54.35 | 334.0 |

## Sensing suite per-document (pdfparser)

| Doc | cell F1 | detect F1 | overall |
|------|--------:|----------:|--------:|
| `90_painted_thin_rect_rules` | 1.0000 | 1.0000 | 100.00 |
| `91_two_large_stacked_grids` | 1.0000 | 1.0000 | 100.00 |
| `92_large_borderless_prose_gap` | 0.9978 | 1.0000 | 99.92 |
| `93_partial_body_hlines` | 1.0000 | 1.0000 | 100.00 |
| `94_invoice_totals_under_grid` | 1.0000 | 1.0000 | 100.00 |
| `95_multicolumn_prose_not_table` | 1.0000 | 1.0000 | 100.00 |

## ICDAR-2013 competitive (external)

**Docs:** 67 · Metrics: Camelot `bench/_metrics.score`

| Rank | Tool | F1 | TEDS | row | col | time (s) |
|-----:|------|---:|-----:|----:|----:|---------:|
| 1 | **camelot_auto** | 0.864 | 0.786 | 0.564 | 0.792 | 72.3 |
| 2 | **pymupdf** | 0.776 | 0.674 | 0.578 | 0.642 | 10.8 |
| 3 | **camelot_lattice_vector** | 0.766 | 0.784 | 0.748 | 0.806 | 3.6 |
| 4 | **pdfparser** ← **ours** | 0.672 | 0.331 | 0.382 | 0.489 | 0.6 |
| 5 | **pdfplumber** | 0.662 | 0.650 | 0.571 | 0.533 | 8.4 |

### pdfparser ICDAR trajectory

| Metric | Pre-improvement | Previous mid | **Now** |
|--------|----------------:|-------------:|--------:|
| F1 | ~0.601 | 0.605 | **0.672** |
| TEDS | ~0.210 | 0.226 | **0.331** |
| row | ~0.134 | 0.136 | **0.382** |
| col | ~0.193 | 0.169 | **0.489** |
| Rank | 5–6 | 5 | **4** |
| time | ~0.5 s | 0.5 s | **0.6 s** |

Still behind Camelot auto/lattice and PyMuPDF on ICDAR; **ahead of pdfplumber** on F1; fastest runtime.

## Artifacts

| Output | Path |
|--------|------|
| Hard scoreboard | `benchmark/results/accuracy_scoreboard_hard.md` |
| Precision scoreboard | `benchmark/results/accuracy_scoreboard_regression_precision.md` |
| Sensing scoreboard | `benchmark/results/accuracy_scoreboard_sensing.md` |
| Basic+stress scoreboard | `benchmark/results/accuracy_scoreboard_basic_stress.md` |
| ICDAR report | `docs/icdar-competitive-report.md` |
| Machine snapshot | `benchmark/results/pipeline_status_snapshot.json` |

---

## Compete hard suite (2026-07-11 dataset freeze)

**Before algorithm work.** Analysis: `docs/compete-struggle-analysis.md`.

| Artifact | Path |
|----------|------|
| Hard fixtures | `benchmark/corpus/compete_synthetic/C100–C180` (81 PDFs) |
| Gold | `benchmark/ground_truth/C1*.json` full grids |
| Real public | 14 PDFs under `compete_real/` |
| Frozen baseline | `benchmark/results/compete_hard_baseline_frozen.json` |
| Scoreboard | `docs/accuracy-scoreboard-compete-hard.md` |
| Integrity gate | `benchmark/scripts/check_compete_suite.py` |

| Metric (pdfparser hard wave) | Value |
|------------------------------|------:|
| n | 81 |
| imperfect rate | 87.7% |
| detect F1 | 0.795 |
| row accuracy | 0.426 |
| col accuracy | 0.596 |
| shape exact | 0.204 |
| cell F1 (TEDS-like) | **0.383** |

Coverage wave C001–C068 remains mostly solved (~0.94 cell F1) — do not use it alone for TEDS/row/col work.
