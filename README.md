# pdfparser

[![CI](https://github.com/gauticse2015/pdfparser/actions/workflows/ci.yml/badge.svg)](https://github.com/gauticse2015/pdfparser/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

**Native, library-first PDF parser in Rust** for born-digital (vector/text) PDFs.

Extracts **text**, **tables**, and common **document objects** with security budgets (stream limits; encrypted PDFs hard-error until crypto ships). Tables are **opt-in**.

> Not a full-page OCR engine. Scan-to-image OCR is out of scope. **Embedded image-painted grids** are recovered via morphology on Image XObjects; optional **external** full-page render (pdftoppm/mutool/gs) is fail-soft for line sensing only.

---

## Architecture (current)

Product default: **`TablePreset::Auto` = Engine V2 exclusive router**.

```text
PDF page
  → content VM (text runs + vector rules; Form XObjects; thin-fill rect rules)
  → embedded Image XObject raster morph (optional full-page render: HQ or K25 opportunistic)
  → lattice / hybrid / network proposals
  → exclusive AutoRouter
        · K26 vertical merge (header/body)
        · partition + ownership
        · nested multi-table keep (outer form + inner rate grid)
        · stream-under-ruled + weak 2-col prose cleanup
  → form scrub
  → optional multipage stitch
```

| Preset | Role |
|--------|------|
| **`Auto` / `Full`** | Production default — Engine V2 router |
| **`EngineV2`** | Same router + shadow diagnostics |
| **`HighQuality`** | Engine V2 + explicit full-page render request |
| **`LatticeOnly`** | Ruled grids only |
| Rollback | `TableOptions.legacy_router = true` → soup NMS path |

Steady quality freeze: [`benchmark/real_track/freezes/g2.json`](benchmark/real_track/freezes/g2.json).  
Design: [`docs/design-table-engine-v2.md`](docs/design-table-engine-v2.md).

---

## Capabilities

| Area | Status | Notes |
|------|--------|--------|
| **Text** | Production | Reading-order sort, `/Rotate`, WinAnsi / MacRoman / Differences / ToUnicode, space insertion |
| **Ruled tables (lattice)** | Production | Vector H/V + thin-fill painted rules, multi-region CC, spans, densify X/Y, empty-column cleanup, exterior stub expansion |
| **Raster line sensing** | Production | Camelot-class morph on **embedded** images; optional external full-page gray render |
| **Partial borders (hybrid)** | Production | Incomplete frames + text columns/rows |
| **Borderless (network)** | Production | Textline network, gap split / same-schema merge, glued label+numeric rows, list/prose FP reject |
| **Nested multi-table** | Production | Outer + inner lattices both kept when nested |
| **Multi-page stitch** | Production | Optional header/column continuity stitch |
| **Objects** | Production | Images, URI links, AcroForm fields, outline |
| **Security** | Production | Stream expansion budgets; no silent encrypted open |
| **Full-page OCR** | Not in product | No text from full-page scans |

### Limitations

- **Scan/OCR PDFs** without vector text: not supported as a product path.
- **ICDAR-class wild pages** (complex multi-table, background lines, image-only cells): still behind Camelot / PyMuPDF on quality (see ICDAR board below).
- **Encoding-broken pages** (missing ToUnicode / odd CMaps): cell quality can collapse (e.g. some BEA NIPA pages).
- **Glued numeric streams** (RBI-style): improved but not perfect vs human grids.
- **ICDAR is never in CI** and is never used for threshold tuning (`assert_no_icdar.py`).

---

## Performance (refreshed 2026-07-12)

**Do not mix tracks.** Owned synthetic ≠ real G1 gold ≠ ICDAR.

### 1) Real-structure track (primary product bar)

15 reviewed T3 golds · stitch off · Auto = Engine V2.  
Source: `benchmark/real_track/results/real_structure_latest.json` · freeze: `freezes/g2.json`.

| Metric | Value |
|--------|------:|
| n_docs | **15** |
| micro **cell F1** | **0.637** |
| micro **det count F1** | **0.964** |
| shape exact rate | **0.533** |
| score 0–100 | **72.6** |

**Peers on the same gold** (equal-weight mean cell F1):  
Source: `benchmark/real_track/results/REALITY_CHECK_PEERS.md`.

| Rank | Library | mean cell F1 | mean det F1 | wins (best cell) |
|-----:|---------|-------------:|------------:|-----------------:|
| 1 | **pdfparser Auto** | **0.623** | **0.964** | 2 |
| 2 | pdfplumber | 0.583 | 0.812 | 3 |
| 3 | Camelot lattice | 0.578 | 0.844 | 7 |
| 4 | Camelot stream | 0.347 | 0.922 | 3 |

> Progressive **lab** numbers on a fixed reviewed set — **not** a market SOTA claim.

Strong on this set: R018/43 (~0.99), 45 donors (~0.94), 42 nested insurance (~0.95), R003 (~0.87).  
Weak: R010 (encoding), 32 census cells, residual 34/36/37.

### 2) Owned multi-lib regression (synthetic / designed)

| Suite | pdfparser cell F1 | det F1 | shape | overall | Source |
|-------|------------------:|-------:|------:|--------:|--------|
| Main (basic+stress+hard) | **0.975** | **0.969** | **0.938** | **98.5** | `accuracy_results.json` |
| Hard 50–62 | **1.000** | **1.000** | **1.000** | **100** | `accuracy_results_hard.json` |
| Compete hard (81 docs) | **0.799** | **0.934** | **0.796** | **84.9** | `accuracy_results_compete_hard.json` |

On these **owned** suites pdfparser is **#1** among open-source peers — progress on fixtures we control, **not** wild-PDF SOTA.

### 3) External ICDAR-2013 (67 PDFs) — honesty check

Camelot-shipped PDFs + `*-str.xml`, Camelot-compatible F1 / TEDS / row / col.  
**Not in repo · not CI · not for tuning.**  
Source: `benchmark/results/camelot_icdar_headtohead.json` · report: [`docs/icdar-competitive-report.md`](docs/icdar-competitive-report.md).

**Latest run (2026-07-12, product Auto, post production-readiness):**

| Rank | Tool | F1 | TEDS | row | col | time (s) |
|-----:|------|---:|-----:|----:|----:|---------:|
| 1 | camelot auto | **0.864** | **0.786** | 0.564 | 0.792 | 73.4 |
| 2 | pymupdf | 0.776 | 0.674 | 0.578 | 0.642 | 10.7 |
| 3 | camelot lattice (vector) | 0.766 | 0.784 | **0.748** | **0.806** | 3.6 |
| 4 | pdfplumber | 0.662 | 0.650 | 0.571 | 0.533 | 8.5 |
| **5** | **pdfparser** | **0.495** | **0.322** | 0.329 | 0.452 | 28.1 |

**Honest read:** pdfparser is **not competitive for #1 on ICDAR**. Quality lags Camelot and PyMuPDF (over-detect / shape / structure). Latency on this run includes opportunistic render probing and is not our best speed path (owned harness is still milliseconds per page without external render).

**ICDAR trajectory:** mid-cycle ~**0.58** → pre-readiness **0.457** (over-detect ~481 preds / 158 GT) → post-readiness **0.495** (~432 preds / 158 GT). Still over-detect-dominated; rank remains **#5**. Analysis: [`docs/icdar-regression-analysis.md`](docs/icdar-regression-analysis.md).

---

## Future improvement plan

Prioritized roadmap. **Do not** retune thresholds on ICDAR gold in CI. Every detector change must A/B **real_structure G1** (and nested keep on doc 42) before shipping; ICDAR remains an external honesty check only.

### P0 — Detection discipline (shared builders; lifts ICDAR without undoing V2)

| # | Item | Why | Guard |
|---|------|-----|-------|
| 1 | **Stricter multi-region lattice merge** — same-schema adjacent fragments that share nearly full page width | Kills phantom slices (e.g. extra 4×4 next to real grids on `eu-001`-class pages) | real G1 det/cell; nested 42 still 2 tables |
| 2 | **Nested keep gates** — require min area ratio + min rows / independent structure on both regions | Nested keep is correct for insurance forms; too loose → corner grid + full grid as two tables on competition layouts | doc 42 cell F1; no mass OVER_DETECT on real set |
| 3 | **Per-page proposal budget / fragment fusion** — merge or drop weak lattice/stream fragments after exclusive route | ICDAR failure mode is **OVER_DETECT (54/67)**, not miss-all; 3× table count destroys Camelot F1 | pred count ↓ on ICDAR external; real G1 det stays high |

**Not planned:** flipping product Auto back to `legacy_router` for ICDAR. A/B shows legacy does **not** restore the ~0.58 snapshot; residual pain is in shared sensing/builders.

### P1 — Structure quality (real G1 residuals + ICDAR TEDS)

| # | Item | Why |
|---|------|-----|
| 4 | **R010 encoding path** — better Differences / missing ToUnicode handling | Cell F1 collapses when text is mojibake even if grid is right |
| 5 | **Census / dense numeric grids (doc 32)** — row/col assignment and glued stream densify | High det, weak cells |
| 6 | **Residual real docs 34 / 36 / 37** — shape + content alignment | Close shape gaps that still hurt micro cell F1 |
| 7 | **TEDS / row-col miscount** on ICDAR — fewer decorative-line rows, better span merge | ROW_MISCOUNT 54, WRONG_SHAPE 56, BAD_STRUCTURE 51 on latest board |

### P2 — Latency and product polish

| # | Item | Why |
|---|------|-----|
| 8 | **Tighten K25 / opportunistic full-page render** — cheaper Auto path when vector rules suffice | ICDAR product run ~30s partly from render probes; owned harness without external render is still ms/page |
| 9 | **CLI / preset clarity** — document when to use HQ vs Auto vs lattice-only for batch jobs | Users should not pay render cost by default for pure vector corpora |
| 10 | **Expand real_structure gold** (n≫15) once P0–P1 stabilize | Broaden the primary bar without folding ICDAR into CI |

### Acceptance bar for shipping detector changes

1. **real_structure** micro cell F1 and det F1 do not regress vs freeze `g2.json` (or intentional freeze bump with review).
2. Nested multi-table (doc **42**) stays outer + inner, not collapsed or triple-split.
3. Owned multi-lib suites (main / hard / compete-hard) stay green.
4. ICDAR re-run is **optional external** evidence only — improve pred/GT ratio and F1 if measured; never gate CI on it.

### Explicit non-goals (near term)

- Full-page OCR product path
- Ranking #1 on ICDAR as a release gate
- Threshold magic keyed on ICDAR filenames
- Replacing Engine V2 Auto with legacy NMS as the default

---

## Quick start

```bash
cargo build --release -p pdfparser-cli

# Text
./target/release/pdfparser extract path/to/file.pdf

# Tables (product Auto = Engine V2)
./target/release/pdfparser extract --tables --format json path/to/file.pdf

# Eval-friendly (per-page fragments, no multipage stitch)
./target/release/pdfparser extract --tables --no-stitch --page-tables --format json path.pdf

# Diagnostics
./target/release/pdfparser extract --tables --dump-evidence --format json path.pdf 2>evidence.json

# High-quality (request full-page render for lines)
./target/release/pdfparser extract --tables-hq --format json path.pdf
```

Library:

```rust
use pdfparser::{Document, TableOptions, TablePreset, TextOptions};

let doc = Document::open("file.pdf")?;
let text = doc.page(0)?.text(&TextOptions::default())?;
let opts = TableOptions::from_preset(TablePreset::Auto);
let (pages, logical) = doc.tables(&TextOptions::default(), &opts)?;
```

---

## Reproducing benchmarks

```bash
python3 -m venv .venv && source .venv/bin/activate
pip install -r benchmark/requirements.txt
cargo build --release -p pdfparser-cli

# Real structure + peers
python3 benchmark/scripts/run_real_structure.py --preset auto --compare
python3 benchmark/scripts/run_reality_check_peers.py

# Owned multi-lib
python3 benchmark/scripts/run_accuracy_benchmark.py --suite regression
python3 benchmark/scripts/run_accuracy_benchmark.py --suite regression_hard

# ICDAR (external data only)
python3 benchmark/scripts/run_icdar_competitive.py \
  --data-dir /path/to/icdar2013-dataset
```

---

## Project layout

| Path | Role |
|------|------|
| `crates/` | Rust workspace (core, content, tables, CLI) |
| `docs/` | Architecture + ICDAR report |
| `benchmark/corpus/` | Owned PDFs (synthetic + real) |
| `benchmark/real_track/gold/` | Reviewed T3 structure golds |
| `benchmark/real_track/freezes/` | Steady freeze `g2.json` |
| `benchmark/real_track/results/` | Latest structure / peer / smoke JSON |
| `benchmark/results/` | Owned multi-lib scoreboards + ICDAR JSON |

---

## License

MIT OR Apache-2.0. See `LICENSE-MIT` and `LICENSE-APACHE`.
