# Competitive-gap dataset design (owned regression)

## Why previous suites were not enough

| Suite | Size | Strength | Blind spots vs ICDAR |
|-------|-----:|----------|----------------------|
| hard 50–62 | 13 | multi-table, spans, noise | few large borderless; little severe over-detect; no multipage chaos |
| precision 70–82 | 13 | FP chrome, phantom lines | not row-fragment 30+; not multi-table count chaos |
| sensing 90–95 | 6 | painted rules, routing, stream FP | single instance per mode; not a coverage matrix |

ICDAR shows **32/67 docs with TEDS&lt;0.2**, **36 OVER_DETECT**, **only 4 “good”**. A handful of fixtures cannot cover that space.

## Policy

1. **No ICDAR PDF/XML in corpus** (competitive external only).
2. **Synthetic** = primary, parametric, full cell gold.
3. **Real public** = secondary, count/shape/token gold where manual grid is feasible; else detect+tokens.
4. Every fixture maps to ≥1 **failure class** below with `failure_classes` in GT.
5. Baseline frozen **before** algorithm work on this suite.

## Failure-class taxonomy (from ICDAR analysis)

| ID | Class | ICDAR n | Dataset need |
|----|-------|--------:|--------------|
| C1 | OVER_DETECT / chrome soup | 36 | page border + footer + ticks + 1 real table; multi false stream |
| C2 | SEVERE_OVER_DETECT (n_pred≫n_gt) | 12 | 1 table + many chrome “grids”; multi-region false splits |
| C3 | UNDER_DETECT / MISS_ALL | 11+4 | painted rules, thin fill, faint line simulation, no-rule borderless only |
| C4 | ROW_FRAGMENT_LARGE | 8 | 25–60 row borderless or partial-rule tables; mid prose gaps |
| C5 | HEADER_ONLY_SLICE | 4 | multi-level header + dense body; sparse H |
| C6 | COL_SKEW_WIDE | 7 | 10–20 column statistical grids |
| C7 | ROW_OVERCOUNT | 8 | false underlines every text line; deco H through body |
| C8 | MULTI_TABLE_COUNT_ERROR | 13 | 3–6 tables/page heterogeneous shapes + noise |
| C9 | MULTIPAGE | 28 | 2–4 pages continued + new tables |
| C10 | COUNT_OK_STRUCT_BAD | 10 | right n_tables, wrong RxC (partial H/V, merge errors) |
| C11 | SAME_SHAPE_ZERO_TEDS | 2 | correct shape, bad cell assign / reading order |
| C12 | ORDER_SENSITIVE | — | multiple tables where y/x order matters |
| C13 | MIXED_LATTICE_STREAM | — | ruled + borderless same page |
| C14 | INVOICE_FOOTER | — | totals rows inside grid |
| C15 | SPAN_COMPLEX | — | multi colspan/rowspan financial |

## Target inventory

| Bucket | Target n | Gold type |
|--------|---------:|-----------|
| Synthetic compete | **≥ 60** | full `expected_tables` grids |
| Real public compete | **≥ 12** | count + tokens + shape when clear |
| **Total** | **≥ 72** | |

Parametric synthesis: each class ≥ 3–6 variants (size, density, noise seed).

## Metrics harness

Suite `regression_compete` reports:

- table_detect_f1 (F1 proxy)
- table_row_accuracy / table_col_accuracy  
- table_cell_f1 (content; TEDS-like when grids exist)
- shape exact rate
- per-class breakdown via `failure_classes` tags

## Success of the *dataset* (before algo)

1. Current pdfparser scores **clearly poorly** on majority of open classes (not 1.0 everywhere).
2. Peers (camelot/plumber) show **differentiated** strengths (some classes they win).
3. Every ICDAR taxonomy row has **≥3 owned fixtures**.
4. Reproducible generation + pinned real downloads with license notes.

## What this dataset is NOT

- Not a copy of ICDAR.
- Not a victory suite (if we score 1.0 immediately, fixtures are too weak—revise).
- Not for algorithm tuning against external competition files.


## Status update (2026-07-11)

### Inventory achieved

| Bucket | n | Notes |
|--------|--:|-------|
| C001–C068 coverage | 64 | Mostly solved; keep as non-regression |
| C100–C180 hard struggle | 81 | Probe-validated open_struggle |
| Real public R001+ | ≥14 | Soft gold |
| **Grid-gold hard** | **81** | Full cells |

### Dataset success (target)

Hard suite must keep pdfparser **well below** 1.0 on cell F1 / row / col until algorithms land.
See `docs/compete-struggle-analysis.md`.
