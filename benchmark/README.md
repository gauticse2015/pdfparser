# Competitive PDF Parser Benchmark

Reproducible market analysis harness for comparing popular PDF libraries
against the scenarios `pdfparser` intends to support.

## Suite policy (important)

| Suite | Purpose | Contents |
|-------|---------|----------|
| **`real_detect_smoke`** (primary direction) | Real public PDFs — detection smoke | `real_track/manifests/real_detect_smoke_v0.json` |
| **`real_structure`** (phased) | Real structure quality (Gate G1 at ≥15 cell grids) | `real_track/` — gold ramp 5→15→25 |
| **`regression`** (default multi-lib) | Synthetic regression | basic + stress + hard |
| **`regression_hard` / precision / compete** | Geometric unit & struggle modes | Synthetic only |
| **`competitive_icdar`** | Head-to-head vs peers | **External only** — never in corpus/CI |

**Hard rules:**
1. ICDAR-2013 PDFs/gold are **never** under `benchmark/corpus/`, `real_track/`, or `ground_truth` used for CI. Enforced by `scripts/assert_no_icdar.py`.
2. Primary product quality signal is moving to **real_track** (see `docs/design-table-engine-v2.md`). Synthetic multi-lib #1 is **not** a market SOTA claim.
3. No file-oriented or accuracy special-cases for named documents.

Details: [`real_track/README.md`](real_track/README.md).

Suite definitions: `corpus/suites.json`.

```bash
# Precision suite (must generate once)
python benchmark/scripts/generate_precision_corpus.py
python benchmark/scripts/run_accuracy_benchmark.py --suite regression_precision \
  --libs pdfparser,pdfplumber,camelot_lattice,camelot_auto
```

## Real track (primary quality signal)

| Item | Path |
|------|------|
| Detect smoke manifest | `real_track/manifests/real_detect_smoke_v0.json` |
| Sources / licenses | `real_track/SOURCES.md` |
| Demotion list (C0) | `real_track/manifests/demotion_list.json` |
| Coverage matrix | `real_track/manifests/coverage_matrix.json` |
| IoU unit tests | `python3 benchmark/scripts/test_iou_metrics.py` |
| Detect smoke runner | `python3 benchmark/scripts/run_real_detect_smoke.py [--dry-run]` |
| ICDAR refuse | `python3 benchmark/scripts/assert_no_icdar.py` |

**Stitch:** structure eval must use `stitch_multipage=false` when the CLI supports it; see `real_track/README.md`.

Synthetic multi-lib scoreboards remain **regression only** — not market SOTA.

## Quick start

```bash
cd /path/to/pdfParser
python3 -m venv .venv
source .venv/bin/activate
pip install -r benchmark/requirements.txt

# 1) Build multi-scenario corpus
python benchmark/scripts/generate_corpus.py
python benchmark/scripts/generate_complex_corpus.py
python benchmark/scripts/generate_hard_corpus.py   # hard regression (50–62)
python benchmark/scripts/generate_precision_corpus.py  # precision suite (70–82)

# 2) Run competitors (extract probe)
python benchmark/scripts/run_benchmark.py

# 3) Full accuracy scoreboard (pdfparser + Camelot + top peers; no ICDAR)
cargo build --release -p pdfparser-cli
python benchmark/scripts/run_accuracy_benchmark.py --suite regression \
  --libs pdfparser,camelot_lattice,camelot_stream,camelot_auto,img2table,pdfplumber,pymupdf,pdfminer.six,pypdf,pypdfium2
python benchmark/scripts/run_accuracy_benchmark.py --suite regression_hard

# 4) Read results
ls benchmark/results/accuracy_scoreboard*.md
# docs/accuracy-scoreboard.md  docs/accuracy-scoreboard-hard.md
```

### Libraries in the accuracy pipeline

| Library | Role |
|---------|------|
| **pdfparser** | Ours (CLI JSON + tables) |
| **camelot_lattice** | Camelot lattice / vector engine |
| **camelot_stream** | Camelot stream |
| **camelot_auto** | Camelot auto flavor pick |
| **img2table** | OpenCV table detector |
| **pdfplumber** | Lines/text tables |
| **pymupdf** | `find_tables` |
| pdfminer.six / pypdf / pypdfium2 | Text baselines (no table API) |
| tabula | Registered; **skipped** without a working JRE |

## Corpus scenarios

### Basic (01–12)

| ID | Category |
|----|----------|
| 01 | Simple digital text |
| 02 | Multi-column reading order |
| 03 | Large multipage (80 pages) |
| 04 | Image-heavy |
| 05 | Special objects (link, forms, outline) |
| 06 | Lattice / ruled table |
| 07 | Stream / whitespace table |
| 08 | Partial-border hybrid table |
| 09 | Complex financial lattice |
| 10 | Mixed text + image + table |
| 11 | Rotated page |
| 12 | Encrypted (password `benchpass`) |

### Hard synthetic (50–62) — regression improvement targets

Original PDFs (reportlab). Difficulty modes inspired by competitive gaps, **not** copies of ICDAR docs.

| ID | Challenge | Failure modes covered |
|----|-----------|------------------------|
| 50 | 3 stacked lattice tables (different shapes) | MULTI_TABLE, mega-grid |
| 51 | 2 pages × 2 tables | MULTI_TABLE + MULTI_PAGE |
| 52 | Page border / margin rules + 1 real table | noise lines, OVER_DETECT |
| 53 | Colspan group headers | WRONG_SHAPE, spans |
| 54 | Rowspan category column | rowspan |
| 55 | Broken corner gaps in rulings | MISS_ALL / gap-close |
| 56 | Tight-gap stacked uneven tables | mega-grid fusion |
| 57 | Wide 16×13 statistical grid | COL/ROW miscount |
| 58 | Multipage continued ledger (stitch) | MULTI_PAGE |
| 59 | Two stream tables + prose | stream multi-region |
| 60 | Lattice + stream on one page | mixed methods |
| 61 | Decorative H-lines around table | false rows |
| 62 | Side-by-side + third table below | multi-region NMS |

## Libraries under test

- pdfplumber
- pdfminer.six
- pypdf
- PyMuPDF (fitz)
- pypdfium2

## Outputs

- `results/benchmark_results.json` — full metrics
- `results/benchmark_results.tsv` — tabular
- `../docs/market-analysis-pdf-parsers.md` — deep-dive report

## Adding `pdfparser` later

Implement an adapter in `scripts/run_benchmark.py` that:

1. Invokes `pdfparser` CLI or FFI
2. Returns the same fields as other adapters (`text`, `tables`, `image_count`, …)
3. Re-run and compare token_recall / table_cell_recall / wall_ms

## License of fixtures

Synthetic PDFs generated by scripts (reportlab); free to use in this repo.


## Corpus v2 (extended)

### Generate everything

```bash
# basic fixtures
python benchmark/scripts/generate_corpus.py
# stress + real registration
python benchmark/scripts/generate_complex_corpus.py
# run all libraries
python benchmark/scripts/run_benchmark.py
```

### Layout

| Path | Content |
|------|---------|
| `corpus/*.pdf` | Basic scenarios (01–12) |
| `corpus/stress/` | Bank statements, overflow, merges, invoices, dense grids, watermarks |
| `corpus/hard/` | Hard synthetic structure suite (50–62) — **regression**, not ICDAR |
| `corpus/suites.json` | Suite membership + ICDAR-exclusion policy |
| `corpus/real/` | Real public PDFs (WARN, Census, IRS, Fed, arXiv, Camelot/Tabula fixtures) |
| `downloads/` | Original downloaded bytes |
| `sources.json` | Provenance |

### Why v2 / v3 exist

Basic PDFs do **not** expose production failures. v2 adds overflowing cells, multi-page ledgers, merged headers, side-by-side tables, and real government/scientific/form PDFs. **v3** adds the **hard** synthetic tier so multi-table / span / noise-line gaps can be fixed without using ICDAR competition files as regression targets.

See `docs/market-analysis-pdf-parsers.md` **Part II** and `corpus/suites.json`.

## Quantitative accuracy scoreboard

Accuracy is computed separately from raw extract probes:

```bash
python benchmark/scripts/build_gold_standards.py   # enrich GT with grids / counts
python benchmark/scripts/run_accuracy_benchmark.py  # score all libraries
```

| Output | Description |
|--------|-------------|
| `results/accuracy_results.json` | Full per-doc text/table/object metrics |
| `results/accuracy_scoreboard.tsv` | Flat spreadsheet |
| `results/accuracy_scoreboard.md` | Human leaderboard |
| `../docs/accuracy-scoreboard.md` | Same, under docs/ |

### Metrics (compare `pdfparser` later on these)

| Metric | What it measures |
|--------|------------------|
| `text_token_f1` | Required tokens found in extracted text |
| `text_cer` / `text_wer` | Error rates vs `reference_text` (synthetic) |
| `table_detect_f1` | Correct **# of tables** (precision/recall on count) |
| `table_row_accuracy` / `table_col_accuracy` | Exact row/col count match rate |
| `table_cell_f1` | Cell text correctness after table alignment (grid gold) |
| `images_score` / `links_f1` / `forms_f1` / `outline_f1` | Other objects |
| `overall_score` (0–100) | Weighted blend per document gold weights |

Module: `benchmark/scripts/metrics.py`
