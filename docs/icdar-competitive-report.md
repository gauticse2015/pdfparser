# ICDAR-2013 Competitive Analysis (external)

**Policy:** ICDAR is **not** part of the regression corpus, not CI, not a tuning target. This report is for competitive measurement only.

**Docs:** 67 · **Gold:** ICDAR `*-str.xml` · **Metrics:** Camelot `bench/_metrics.score` (F1, TEDS proxy, row/col)

**Incomplete board:** this dump is **pdfparser-only** (**peers=1**). A solo board is **not** a multi-peer rank. Do **not** claim ICDAR #1 from this file. The honest multi-peer table is in the repo README (pdfparser **#2** on detection F1 vs Camelot auto). Gate labels: [`STATUS.md`](STATUS.md).

## Leaderboard (peers=1 - not a rank)

| Rank | Tool | F1 | TEDS | row | col | time (s) |
|-----:|------|---:|-----:|----:|----:|---------:|
| n/a (solo) | **pdfparser** | 0.826 | 0.459 | 0.500 | 0.535 | 27.25 |

## pdfparser vs Camelot (headline)

| Metric | pdfparser | camelot lattice/vector | camelot auto | Δ vs lattice |
|--------|----------:|-----------------------:|-------------:|-------------:|
| f1 | 0.826 | — | — | — |
| teds | 0.459 | — | — | — |
| row | 0.500 | — | — | — |
| col | 0.535 | — | — | — |

## Improvement vs previous ICDAR run

| Metric | Previous | Now | Δ |
|--------|---------:|----:|--:|
| f1 | 0.822 | 0.826 | +0.003 |
| teds | 0.461 | 0.459 | -0.003 |
| row | 0.504 | 0.500 | -0.004 |
| col | 0.539 | 0.535 | -0.004 |

## Failure mode histogram (pdfparser)

| Mode | Docs |
|------|-----:|
| WRONG_SHAPE | 47 |
| ROW_MISCOUNT | 41 |
| BAD_STRUCTURE | 39 |
| COL_MISCOUNT | 33 |
| MULTI_PAGE_DOC | 28 |
| OVER_DETECT | 22 |
| MULTI_TABLE_PAGE | 17 |
| UNDER_DETECT | 7 |
| MISS_ALL | 1 |

### Buckets

- **miss_all:** 1
- **under:** 6
- **over:** 22
- **bad_struct_ok_count:** 15
- **good:** 19

## Multi-table vs single-table

- Multi-table docs (n=33): mean F1 us=0.843, camelot=0.843; TEDS us=0.399, camelot=0.399
- Single-table docs (n=34): mean F1 us=0.804, camelot=0.804; TEDS us=0.541, camelot=0.541

## Worst TEDS gap vs Camelot lattice (top 15)

| Doc | ΔTEDS | F1 us/c | TEDS us/c | n_gt/us/c | modes |
|------|------:|--------:|----------:|----------:|-------|
| `eu-001.pdf` | +0.000 | 1.00/1.00 | 0.720/0.720 | 7/7/7 | MULTI_TABLE_PAGE, MULTI_PAGE_DOC, BAD_STRUCTURE |
| `eu-002.pdf` | +0.000 | 1.00/1.00 | 1.000/1.000 | 1/1/1 |  |
| `eu-003.pdf` | +0.000 | 1.00/1.00 | 0.631/0.631 | 3/3/3 | MULTI_TABLE_PAGE, ROW_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `eu-004.pdf` | +0.000 | 1.00/1.00 | 0.662/0.662 | 12/12/12 | MULTI_TABLE_PAGE, MULTI_PAGE_DOC, COL_MISCOUNT, WRONG_SHAPE |
| `eu-005.pdf` | +0.000 | 1.00/1.00 | 0.740/0.740 | 2/2/2 | MULTI_TABLE_PAGE, BAD_STRUCTURE |
| `eu-006.pdf` | +0.000 | 1.00/1.00 | 0.750/0.750 | 4/4/4 | MULTI_TABLE_PAGE, MULTI_PAGE_DOC, BAD_STRUCTURE |
| `eu-007.pdf` | +0.000 | 1.00/1.00 | 0.573/0.573 | 6/6/6 | MULTI_TABLE_PAGE, MULTI_PAGE_DOC, COL_MISCOUNT, WRONG_SHAPE |
| `eu-008.pdf` | +0.000 | 1.00/1.00 | 1.000/1.000 | 1/1/1 |  |
| `eu-009a.pdf` | +0.000 | 1.00/1.00 | 0.917/0.917 | 1/1/1 |  |
| `eu-010.pdf` | +0.000 | 1.00/1.00 | 1.000/1.000 | 1/1/1 |  |
| `eu-011.pdf` | +0.000 | 0.67/0.67 | 0.716/0.716 | 1/2/2 | OVER_DETECT, ROW_MISCOUNT, WRONG_SHAPE |
| `eu-012.pdf` | +0.000 | 0.83/0.83 | 0.365/0.365 | 5/7/7 | OVER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC, COL_MISCOUNT |
| `eu-013.pdf` | +0.000 | 0.67/0.67 | 0.130/0.130 | 4/5/5 | OVER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT |
| `eu-014.pdf` | +0.000 | 0.00/0.00 | 0.000/0.000 | 1/0/0 | MISS_ALL, UNDER_DETECT |
| `eu-015.pdf` | +0.000 | 0.57/0.57 | 0.424/0.424 | 5/2/2 | UNDER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT |

## Detect OK, structure bad (F1≥0.9, TEDS&lt;0.35) — n=12

- `us-003.pdf`: shapes us=[[[6, 4]]] gt=[[[5, 4]]] TEDS=0.043
- `us-011a.pdf`: shapes us=[[[3, 2]], [[3, 2]]] gt=[[[13, 2]], [[7, 2]]] TEDS=0.016
- `us-012.pdf`: shapes us=[[[23, 6]]] gt=[[[21, 6]]] TEDS=0.095
- `us-013.pdf`: shapes us=[[[16, 5]]] gt=[[[4, 5]]] TEDS=0.074
- `us-014.pdf`: shapes us=[[[8, 3]], [[8, 3]]] gt=[[[6, 3]], [[6, 3]]] TEDS=0.345
- `us-015.pdf`: shapes us=[[[6, 4]], [[32, 4]]] gt=[[[10, 2]], [[7, 4]]] TEDS=0.095
- `us-018.pdf`: shapes us=[[[48, 17]], [[48, 19]], [[33, 2]], [[4, 2]], [[7, 2]], [[32, 10]], [[13, 10], [14, 8]]] gt=[[[58, 11]], [[58, 10]], [[59, 5]], [[32, 7]], [[29, 4]], [[32, 6]], [[32, 6]]] TEDS=0.066
- `us-021.pdf`: shapes us=[[[8, 10], [5, 6]]] gt=[[[12, 8], [5, 4]]] TEDS=0.254
- `us-025.pdf`: shapes us=[[[6, 9], [13, 24]], [[10, 24], [14, 20], [10, 18]]] gt=[[[14, 7], [17, 13]], [[9, 13], [17, 13], [9, 13]], [[53, 7]]] TEDS=0.070
- `us-026.pdf`: shapes us=[[[17, 10]]] gt=[[[18, 6]]] TEDS=0.182
- `us-032.pdf`: shapes us=[[[18, 3]]] gt=[[[8, 4]]] TEDS=0.252
- `us-038.pdf`: shapes us=[[[8, 2]]] gt=[[[9, 3]]] TEDS=0.212

## MISS_ALL (n=1)

- `eu-014.pdf`: gt=1 camelot=0 camelot TEDS=0.000

## Gap analysis (where we still lack)

pdfparser F1=0.826 TEDS=0.459 row=0.500 col=0.535. Camelot columns in this dump are empty (peers=1; not a head-to-head). Do not read F1=0.000 as Camelot scoring zero.

### Primary remaining gaps

1. **Structure quality (TEDS / row / col)** — Detection has improved more than content alignment. ROW_MISCOUNT=41, COL_MISCOUNT=33, WRONG_SHAPE=47, BAD_STRUCTURE=39.
2. **MISS_ALL / UNDER_DETECT** — MISS_ALL=1, UNDER_DETECT=7. Often stream-only or faint/incomplete rules where lattice CC has too few joints; Camelot raster/auto recovers some of these.
3. **MULTI_TABLE_PAGE** — 17 docs. Multi-region CC helps; residual fusion or order-mismatch vs gold still hurts F1/TEDS (order-based matching).
4. **Spans & partial rules** — High F1 / low TEDS cases usually have wrong row/col counts from extra decorative lines or missing span merge on real competition layouts.
5. **Metric sensitivity** — ICDAR matching is **page order**, not IoU. Correct tables in wrong order look like structure failures. TEDS is a difflib proxy, not tree-edit TEDS.
6. **No raster line engine** — Camelot `auto`/`combined` can find painted/faint rules; we are vector-only.

---
*Generated by `benchmark/scripts/run_icdar_competitive.py`. ICDAR files remain external; never copied into `benchmark/corpus/`.*
