# ICDAR-2013 Competitive Analysis (external)

**Policy:** ICDAR is **not** part of the regression corpus. This report is for competitive measurement only.

**Docs:** 67 · **Gold:** ICDAR `*-str.xml` · **Metrics:** Camelot `bench/_metrics.score` (F1, TEDS proxy, row/col)

## Leaderboard

| Rank | Tool | F1 | TEDS | row | col | time (s) |
|-----:|------|---:|-----:|----:|----:|---------:|
| 1 | **camelot_auto** | 0.864 | 0.786 | 0.564 | 0.792 | 72.40 |
| 2 | **pdfparser** ← **ours** | 0.825 | 0.475 | 0.489 | 0.547 | 25.43 |
| 3 | **pymupdf** | 0.776 | 0.674 | 0.578 | 0.642 | 10.86 |
| 4 | **camelot_lattice_vector** | 0.766 | 0.784 | 0.748 | 0.806 | 3.85 |
| 5 | **pdfplumber** | 0.662 | 0.650 | 0.571 | 0.533 | 8.73 |

## pdfparser vs Camelot (headline)

| Metric | pdfparser | camelot lattice/vector | camelot auto | Δ vs lattice |
|--------|----------:|-----------------------:|-------------:|-------------:|
| f1 | 0.825 | 0.766 | 0.864 | +0.060 |
| teds | 0.475 | 0.784 | 0.786 | -0.309 |
| row | 0.489 | 0.748 | 0.564 | -0.259 |
| col | 0.547 | 0.806 | 0.792 | -0.258 |

## Improvement vs previous ICDAR run

| Metric | Previous | Now | Δ |
|--------|---------:|----:|--:|
| f1 | 0.825 | 0.825 | +0.000 |
| teds | 0.475 | 0.475 | +0.000 |
| row | 0.489 | 0.489 | +0.000 |
| col | 0.547 | 0.547 | +0.000 |

## Failure mode histogram (pdfparser)

| Mode | Docs |
|------|-----:|
| WRONG_SHAPE | 47 |
| ROW_MISCOUNT | 42 |
| BAD_STRUCTURE | 37 |
| COL_MISCOUNT | 30 |
| MULTI_PAGE_DOC | 28 |
| MULTI_TABLE_PAGE | 17 |
| OVER_DETECT | 17 |
| UNDER_DETECT | 11 |
| MISS_ALL | 4 |

### Buckets

- **miss_all:** 4
- **under:** 7
- **over:** 17
- **bad_struct_ok_count:** 14
- **good:** 21

## Multi-table vs single-table

- Multi-table docs (n=33): mean F1 us=0.772, camelot=0.631; TEDS us=0.390, camelot=0.476
- Single-table docs (n=34): mean F1 us=0.858, camelot=0.775; TEDS us=0.557, camelot=0.519

## Worst TEDS gap vs Camelot lattice (top 15)

| Doc | ΔTEDS | F1 us/c | TEDS us/c | n_gt/us/c | modes |
|------|------:|--------:|----------:|----------:|-------|
| `eu-014.pdf` | -1.000 | 0.00/1.00 | 0.000/1.000 | 1/0/1 | MISS_ALL, UNDER_DETECT |
| `us-016.pdf` | -0.875 | 0.00/1.00 | 0.000/0.875 | 1/0/1 | MISS_ALL, UNDER_DETECT |
| `us-012.pdf` | -0.811 | 1.00/1.00 | 0.095/0.906 | 1/1/1 | ROW_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `us-013.pdf` | -0.650 | 1.00/1.00 | 0.074/0.724 | 1/1/1 | ROW_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `eu-012.pdf` | -0.635 | 0.83/1.00 | 0.365/1.000 | 5/7/5 | OVER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC, COL_MISCOUNT |
| `eu-013.pdf` | -0.624 | 0.67/0.80 | 0.130/0.755 | 4/5/6 | OVER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, COL_MISCOUNT |
| `us-015.pdf` | -0.595 | 0.67/1.00 | 0.055/0.650 | 2/1/2 | UNDER_DETECT, MULTI_PAGE_DOC, ROW_MISCOUNT, WRONG_SHAPE |
| `us-031a.pdf` | -0.572 | 0.67/1.00 | 0.215/0.787 | 1/2/1 | OVER_DETECT, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE |
| `us-004.pdf` | -0.524 | 1.00/1.00 | 0.238/0.762 | 1/1/1 | BAD_STRUCTURE |
| `eu-015.pdf` | -0.521 | 0.57/1.00 | 0.424/0.945 | 5/2/5 | UNDER_DETECT, MULTI_TABLE_PAGE, MULTI_PAGE_DOC, ROW_MISCOUNT |
| `us-038.pdf` | -0.467 | 1.00/1.00 | 0.212/0.679 | 1/1/1 | ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `us-014.pdf` | -0.431 | 1.00/1.00 | 0.345/0.776 | 2/2/2 | MULTI_PAGE_DOC, ROW_MISCOUNT, WRONG_SHAPE, BAD_STRUCTURE |
| `us-040.pdf` | -0.430 | 0.33/1.00 | 0.215/0.645 | 1/5/1 | OVER_DETECT, ROW_MISCOUNT, COL_MISCOUNT, WRONG_SHAPE |
| `eu-007.pdf` | -0.427 | 1.00/1.00 | 0.573/1.000 | 6/6/6 | MULTI_TABLE_PAGE, MULTI_PAGE_DOC, COL_MISCOUNT, WRONG_SHAPE |
| `eu-004.pdf` | -0.323 | 1.00/1.00 | 0.662/0.985 | 12/12/12 | MULTI_TABLE_PAGE, MULTI_PAGE_DOC, COL_MISCOUNT, WRONG_SHAPE |

## Detect OK, structure bad (F1≥0.9, TEDS&lt;0.35) — n=10

- `us-003.pdf`: shapes us=[[[9, 4]]] gt=[[[5, 4]]] TEDS=0.030
- `us-004.pdf`: shapes us=[[[15, 7]]] gt=[[[15, 7]]] TEDS=0.238
- `us-012.pdf`: shapes us=[[[23, 6]]] gt=[[[21, 6]]] TEDS=0.095
- `us-013.pdf`: shapes us=[[[16, 5]]] gt=[[[4, 5]]] TEDS=0.074
- `us-014.pdf`: shapes us=[[[8, 3]], [[8, 3]]] gt=[[[6, 3]], [[6, 3]]] TEDS=0.345
- `us-018.pdf`: shapes us=[[[48, 17]], [[48, 19]], [[32, 2]], [[4, 2]], [[7, 2]], [[32, 10]], [[13, 10], [14, 8]]] gt=[[[58, 11]], [[58, 10]], [[59, 5]], [[32, 7]], [[29, 4]], [[32, 6]], [[32, 6]]] TEDS=0.066
- `us-025.pdf`: shapes us=[[[6, 9], [13, 24]], [[10, 24], [14, 20], [10, 18]]] gt=[[[14, 7], [17, 13]], [[9, 13], [17, 13], [9, 13]], [[53, 7]]] TEDS=0.070
- `us-026.pdf`: shapes us=[[[16, 10]]] gt=[[[18, 6]]] TEDS=0.138
- `us-032.pdf`: shapes us=[[[18, 3]]] gt=[[[8, 4]]] TEDS=0.252
- `us-038.pdf`: shapes us=[[[8, 2]]] gt=[[[9, 3]]] TEDS=0.212

## MISS_ALL (n=4)

- `eu-014.pdf`: gt=1 camelot=1 camelot TEDS=1.000
- `us-011a.pdf`: gt=2 camelot=2 camelot TEDS=0.025
- `us-016.pdf`: gt=1 camelot=1 camelot TEDS=0.875
- `us-021.pdf`: gt=2 camelot=0 camelot TEDS=0.000

## Gap analysis (where we still lack)

pdfparser F1=0.825 TEDS=0.475 row=0.489 col=0.547 vs camelot lattice F1=0.766 TEDS=0.784.

### Primary remaining gaps

1. **Structure quality (TEDS / row / col)** — Detection has improved more than content alignment. ROW_MISCOUNT=42, COL_MISCOUNT=30, WRONG_SHAPE=47, BAD_STRUCTURE=37.
2. **MISS_ALL / UNDER_DETECT** — MISS_ALL=4, UNDER_DETECT=11. Often stream-only or faint/incomplete rules where lattice CC has too few joints; Camelot raster/auto recovers some of these.
3. **MULTI_TABLE_PAGE** — 17 docs. Multi-region CC helps; residual fusion or order-mismatch vs gold still hurts F1/TEDS (order-based matching).
4. **Spans & partial rules** — High F1 / low TEDS cases usually have wrong row/col counts from extra decorative lines or missing span merge on real competition layouts.
5. **Metric sensitivity** — ICDAR matching is **page order**, not IoU. Correct tables in wrong order look like structure failures. TEDS is a difflib proxy, not tree-edit TEDS.
6. **No raster line engine** — Camelot `auto`/`combined` can find painted/faint rules; we are vector-only.

---
*Generated by `benchmark/scripts/run_icdar_competitive.py`. ICDAR files remain external; never copied into `benchmark/corpus/`.*
