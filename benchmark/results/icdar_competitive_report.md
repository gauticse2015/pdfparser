# ICDAR-2013 Competitive Analysis (external)

**Policy:** ICDAR is **not** part of the regression corpus. This report is for competitive measurement only.

**Docs:** 67 · **Gold:** ICDAR `*-str.xml` · **Metrics:** Camelot `bench/_metrics.score` (F1, TEDS proxy, row/col)

## Leaderboard

| Rank | Tool | F1 | TEDS | row | col | time (s) |
|-----:|------|---:|-----:|----:|----:|---------:|
| 1 | **camelot_auto** | 0.864 | 0.786 | 0.564 | 0.792 | 72.30 |
| 2 | **pymupdf** | 0.776 | 0.674 | 0.578 | 0.642 | 10.76 |
| 3 | **camelot_lattice_vector** | 0.766 | 0.784 | 0.748 | 0.806 | 3.55 |
| 4 | **pdfparser** ← **ours** | 0.672 | 0.331 | 0.382 | 0.489 | 0.58 |
| 5 | **pdfplumber** | 0.662 | 0.650 | 0.571 | 0.533 | 8.42 |

## pdfparser vs Camelot (headline)

| Metric | pdfparser | camelot lattice/vector | camelot auto | Δ vs lattice |
|--------|----------:|-----------------------:|-------------:|-------------:|
| f1 | 0.672 | 0.766 | 0.864 | -0.094 |
| teds | 0.331 | 0.784 | 0.786 | -0.453 |
| row | 0.382 | 0.748 | 0.564 | -0.366 |
| col | 0.489 | 0.806 | 0.792 | -0.317 |

## Improvement vs previous ICDAR run

| Metric | Previous | Now | Δ |
|--------|---------:|----:|--:|
| f1 | 0.605 | 0.672 | +0.067 |
| teds | 0.226 | 0.331 | +0.105 |
| row | 0.136 | 0.382 | +0.246 |
| col | 0.169 | 0.489 | +0.319 |

## Failure mode histogram (pdfparser)

| Mode | Docs |
|------|-----:|
| WRONG_SHAPE | 47 |
| BAD_STRUCTURE | 44 |
| ROW_MISCOUNT | 40 |
| OVER_DETECT | 36 |
| COL_MISCOUNT | 36 |
| MULTI_PAGE_DOC | 28 |
| MULTI_TABLE_PAGE | 17 |
| UNDER_DETECT | 11 |
| MISS_ALL | 4 |

### Buckets

- **miss_all:** 4
- **under:** 7
- **over:** 36
- **bad_struct_ok_count:** 13
- **good:** 4

## Multi-table vs single-table

- Multi-table docs (n=33): mean F1 us=0.628, camelot=0.631; TEDS us=0.271, camelot=0.476
- Single-table docs (n=34): mean F1 us=0.661, camelot=0.775; TEDS us=0.404, camelot=0.519

## Worst TEDS gap vs Camelot lattice (top 15)

| Doc | ΔTEDS | F1 us/c | TEDS us/c | n_gt/us/c | modes |
|------|------:|--------:|----------:|----------:|-------|
| `us-012.pdf` | -0.906 | 0.00/1.00 | 0.000/0.906 | 1/0/1 | MISS_ALL, UNDER_DETECT |
| `eu-014.pdf` | -0.897 | 0.40/1.00 | 0.103/1.000 | 1/4/1 | OVER_DETECT, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE |
| `eu-012.pdf` | -0.888 | 0.53/1.00 | 0.112/1.000 | 5/14/5 | OVER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT |
| `eu-001.pdf` | -0.875 | 0.93/1.00 | 0.125/1.000 | 7/8/7 | OVER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT |
| `eu-007.pdf` | -0.865 | 1.00/1.00 | 0.135/1.000 | 6/6/6 | MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT, WRONG_SHAPE |
| `us-005.pdf` | -0.800 | 1.00/1.00 | 0.000/0.800 | 1/1/1 | BAD_STRUCTURE |
| `us-016.pdf` | -0.745 | 0.50/1.00 | 0.130/0.875 | 1/3/1 | OVER_DETECT, ROW_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `us-014.pdf` | -0.724 | 0.80/1.00 | 0.051/0.776 | 2/3/2 | OVER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, WRONG_SHAPE |
| `us-013.pdf` | -0.692 | 0.50/1.00 | 0.031/0.724 | 1/3/1 | OVER_DETECT, ROW_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `eu-013.pdf` | -0.672 | 0.38/0.80 | 0.083/0.755 | 4/12/6 | OVER_DETECT, MULTI_PAGE_DOC, COL_MISCOUNT, WRONG_SHAPE |
| `us-004.pdf` | -0.552 | 0.50/1.00 | 0.210/0.762 | 1/3/1 | OVER_DETECT, BAD_STRUCTURE |
| `eu-025.pdf` | -0.522 | 1.00/1.00 | 0.478/1.000 | 5/5/5 | MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT, WRONG_SHAPE |
| `us-015.pdf` | -0.494 | 0.67/1.00 | 0.156/0.650 | 2/4/2 | OVER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, WRONG_SHAPE |
| `us-031a.pdf` | -0.471 | 0.33/1.00 | 0.316/0.787 | 1/5/1 | OVER_DETECT, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE |
| `us-009.pdf` | -0.455 | 1.00/1.00 | 0.227/0.682 | 1/1/1 | COL_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |

## Detect OK, structure bad (F1≥0.9, TEDS&lt;0.35) — n=10

- `eu-001.pdf`: shapes us=[[[8, 4], [10, 4], [13, 4], [4, 4]], [[23, 4], [24, 4]], [[9, 4], [27, 4]]] gt=[[[8, 4], [13, 4], [10, 4]], [[24, 4], [23, 4]], [[18, 4], [9, 4]]] TEDS=0.125
- `eu-003.pdf`: shapes us=[[[3, 3], [10, 6], [7, 5]]] gt=[[[3, 3], [7, 5], [4, 6]]] TEDS=0.182
- `eu-007.pdf`: shapes us=[[[5, 4]], [[2, 7]], [[11, 3], [2, 3]], [[16, 4], [2, 4]]] gt=[[[5, 4]], [[2, 7]], [[2, 3], [11, 3]], [[2, 4], [9, 4]]] TEDS=0.135
- `us-003.pdf`: shapes us=[[[4, 14]]] gt=[[[5, 4]]] TEDS=0.032
- `us-005.pdf`: shapes us=[[[5, 2]]] gt=[[[5, 2]]] TEDS=0.000
- `us-009.pdf`: shapes us=[[[22, 6]]] gt=[[[22, 7]]] TEDS=0.227
- `us-017.pdf`: shapes us=[[[3, 12]], [[3, 3]], [[3, 7]], [[3, 10]], [[3, 10]], [[3, 10]]] gt=[[[31, 10]], [[31, 10]], [[31, 9]], [[31, 8]], [[31, 8]], [[31, 8]]] TEDS=0.028
- `us-018.pdf`: shapes us=[[[3, 8]], [[3, 5]], [[3, 8]], [[4, 7]], [[29, 3]], [[4, 13]], [[4, 8]]] gt=[[[58, 11]], [[58, 10]], [[59, 5]], [[32, 7]], [[29, 4]], [[32, 6]], [[32, 6]]] TEDS=0.048
- `us-023.pdf`: shapes us=[[[11, 14]]] gt=[[[10, 13]]] TEDS=0.223
- `us-032.pdf`: shapes us=[[[15, 3]]] gt=[[[8, 4]]] TEDS=0.231

## MISS_ALL (n=4)

- `eu-018.pdf`: gt=2 camelot=2 camelot TEDS=0.215
- `us-011a.pdf`: gt=2 camelot=2 camelot TEDS=0.025
- `us-012.pdf`: gt=1 camelot=1 camelot TEDS=0.906
- `us-026.pdf`: gt=1 camelot=0 camelot TEDS=0.000

## Gap analysis (where we still lack)

pdfparser F1=0.672 TEDS=0.331 row=0.382 col=0.489 vs camelot lattice F1=0.766 TEDS=0.784.

### Primary remaining gaps

1. **Structure quality (TEDS / row / col)** — Detection has improved more than content alignment. ROW_MISCOUNT=40, COL_MISCOUNT=36, WRONG_SHAPE=47, BAD_STRUCTURE=44.
2. **MISS_ALL / UNDER_DETECT** — MISS_ALL=4, UNDER_DETECT=11. Often stream-only or faint/incomplete rules where lattice CC has too few joints; Camelot raster/auto recovers some of these.
3. **MULTI_TABLE_PAGE** — 17 docs. Multi-region CC helps; residual fusion or order-mismatch vs gold still hurts F1/TEDS (order-based matching).
4. **Spans & partial rules** — High F1 / low TEDS cases usually have wrong row/col counts from extra decorative lines or missing span merge on real competition layouts.
5. **Metric sensitivity** — ICDAR matching is **page order**, not IoU. Correct tables in wrong order look like structure failures. TEDS is a difflib proxy, not tree-edit TEDS.
6. **No raster line engine** — Camelot `auto`/`combined` can find painted/faint rules; we are vector-only.

---
*Generated by `benchmark/scripts/run_icdar_competitive.py`. ICDAR files remain external; never copied into `benchmark/corpus/`.*
