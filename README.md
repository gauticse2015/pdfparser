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
| **`Fast`** | Engine V2, **never** full-page render (latency path) |
| **`LatticeOnly`** | Ruled grids only |
| Rollback | `TableOptions.legacy_router = true` → soup NMS path |

Public `TableOptions` product surface is **≤12 top-level fields**; detector knobs nest under `advanced` (Deref-compatible). Classic whitespace stream is **off** on product Auto (network path only).

Steady quality freezes: [`g2.json`](benchmark/real_track/freezes/g2.json) (detect/core cell).  
Gated plan: [`docs/implementation-plan-v3-gated.md`](docs/implementation-plan-v3-gated.md) · design: [`docs/design-table-engine-v2.md`](docs/design-table-engine-v2.md).

**Document-type tuning (experimental):** geometry densify / lattice thresholds live in `TableTuning` (defaults + per-call override). CLI: `--table-setting key=value`. See [`docs/options-deprecation-map.md`](docs/options-deprecation-map.md).

---

## Status at a glance (customer view) — 2026-07-19

| Question | Honest answer |
|----------|----------------|
| Can I use this in production for **text**? | **Yes** |
| Can I use this for **table detection** on born-digital PDFs? | **Yes** — strong precision / low over-detect on real-track |
| Can I use this for **cell-accurate structure** on hard / statistical PDFs? | **Best-effort** — good on clean ruled grids; weak on dense glued streams |
| Are you #1 on ICDAR-2013? | **No** — **#2 on detection F1** (0.825 vs Camelot auto 0.864); lag on TEDS/structure |
| Is ICDAR used in CI? | **No** — external honesty check only |

### Maturity labels

| Capability | Maturity | Customer expectation |
|------------|----------|----------------------|
| Text extract | **Production** | Reading order, encodings, rotate |
| Ruled lattice detection | **Production** | Find tables with vector rules |
| Nested multi-table keep | **Production** | Outer + inner kept when nested |
| Borderless / network **detection** | **Production** | Finds most multi-col streams |
| Borderless / network **cells** | **Beta** | Glued numeric pages still weak |
| Hybrid partial borders | **Production (detection)** | Incomplete frames |
| Raster lines (embedded images) | **Production (best-effort)** | Image-painted grids |
| Full-page render line sensing | **Optional** | HQ / opportunistic; fail-soft |
| Cell content / spans / TEDS | **Beta** | Shape-right pages good; wrong-shape hurts hard |
| `TableTuning` settings dict | **Beta** | Defaults + document-type overrides |
| Multipage stitch | **Production optional** | Off for eval (`--no-stitch`) |
| Full-page OCR / scans | **Not in product** | Out of scope |

**Do not claim:** GATE-3 shape green, GATE-4 cell green, or ICDAR #1. Detection gates (G1/G2) are green under honest metrics.

---

## Capabilities

| Area | Status | Notes |
|------|--------|--------|
| **Text** | Production | Reading-order sort, `/Rotate`, WinAnsi / MacRoman / Differences / ToUnicode (incl. region CMaps), space insertion |
| **Ruled tables (lattice)** | Production | Vector H/V + thin-fill rules, multi-region CC, spans, text densify X/Y, empty-column cleanup |
| **Raster line sensing** | Production | Morphology on **embedded** Image XObjects; optional external full-page gray (HQ / Auto opportunistic) |
| **Partial borders (hybrid)** | Production | Incomplete frames + text columns/rows |
| **Borderless (network)** | Production (detect) / Beta (cells) | Textline network, gap split, glued label+numeric recovery, list/prose FP reject |
| **Classic whitespace stream** | Experimental | Off on product Auto; opt-in `allow_classic_stream` / LatticeStream |
| **Nested multi-table** | Production | Outer + inner lattices both kept when nested |
| **Multi-page stitch** | Production optional | Header/column continuity; eval uses `--no-stitch` |
| **Form discriminator** | Beta | Reduces chrome/form FPs; threshold-heavy |
| **Document tuning** | Beta | `TableTuning` / `--table-setting` for densify & lattice geometry |
| **Objects** | Production | Images, URI links, AcroForm fields, outline |
| **Security** | Production | Stream expansion budgets; encrypted PDFs hard-error |
| **Full-page OCR** | Not in product | No text from full-page scans |

### Limitations (read before integrating)

- **Scan/OCR PDFs** without vector text: not a product path.
- **Structure quality** on wild multi-table / decorative-rule pages: often wrong row/col counts even when a table is detected.
- **Dense statistical / glued streams** (census, BEA-style GDP blocks): detection may succeed; **cell F1 can be near zero** on the hardest pages.
- **Encoding-broken pages** (missing ToUnicode): improved, residual mojibake risk.
- **Locale-specific post-process** (invoice footer keywords): English-oriented.
- **ICDAR never in CI** and never used as a tuning target (`assert_no_icdar.py`).

---

## Performance & rankings (refreshed 2026-07-19)

**Do not mix tracks.** Owned synthetic ≠ real G1 gold ≠ external ICDAR.

### 1) Real-structure track (primary product bar)

Human/vision T3 gold · stitch off · product **Auto = Engine V2**.  
Sources: `benchmark/real_track/results/real_structure_latest.json`, freezes `g2.json`, discipline / FP strict latest.

| Metric | Core (g2, n=15) | Full suite (n=23) |
|--------|----------------:|------------------:|
| micro **cell F1** | **0.738** | **0.639** |
| micro **det count F1** | **~0.978** | **0.933** |
| micro **det IoU F1** | **1.000** | **0.869** |
| shape exact rate | **~0.667** | **0.580** |
| detect discipline exact | **0.941** (n=34) | — |
| over-detect doc rate | **0.029** | — |
| fp_strict zero rate | **1.000** (n=12) | — |

**Interpretation for customers**

- **Detection is the strength** — exact table counts, low FP, nested keep.
- **Cell / shape are the gap** — core cell improved vs older freezes, but several hard docs (dense census, glued financial streams) still drag the mean.
- Internal hard gates: **G1/G2 PASS**, **G3 shape FAIL**, **G4 cells FAIL** (honest floors; no gold padding).

#### Peer ranking on shared real gold (older equal-weight peer board)

Last full multi-lib peer run on the reviewed T3 set (`REALITY_CHECK_PEERS.md`, 2026-07-12). pdfparser mean cell has since moved to **0.738** core; peer re-run recommended for an updated win table.

| Rank (then) | Library | mean cell F1 | mean det F1 | wins (best cell) |
|------------:|---------|-------------:|------------:|-----------------:|
| 1 | **pdfparser Auto** | **0.637** | **0.964** | 1 |
| 2 | pdfplumber | 0.583 | 0.812 | 3 |
| 3 | Camelot lattice | 0.578 | 0.844 | 8 |
| 4 | Camelot stream | 0.347 | 0.922 | 3 |

> Equal-weight mean on a **fixed reviewed set** — useful for integration decisions, **not** a market-wide SOTA claim. Camelot still wins more individual docs; pdfparser leads mean cell on that board via strong detection + clean grids.

### 2) Owned multi-lib regression (synthetic / designed)

| Suite | pdfparser cell F1 | det F1 | shape | overall | Source |
|-------|------------------:|-------:|------:|--------:|--------|
| Main (basic+stress+hard) | **0.975** | **0.969** | **0.938** | **98.5** | `accuracy_results.json` |
| Hard 50–62 | **1.000** | **1.000** | **1.000** | **100** | `accuracy_results_hard.json` |
| Compete hard (81 docs) | **0.799** | **0.934** | **0.796** | **84.9** | `accuracy_results_compete_hard.json` |

On these **owned** suites pdfparser is typically **#1** among open-source peers we measure — progress on fixtures we control, **not** wild-PDF SOTA.

### 3) External ICDAR-2013 (67 PDFs) — competitive honesty check

Camelot-shipped PDFs + `*-str.xml`, Camelot-compatible F1 / TEDS / row / col.  
**Not in repo · not CI · not for tuning.**  
Source: `benchmark/results/camelot_icdar_headtohead.json` · report: [`docs/icdar-competitive-report.md`](docs/icdar-competitive-report.md).

**pdfparser product Auto (2026-07-19, post kill-list + densify; no gold pads):**

| Metric | Value | Internal floor (honest) | Target (G3/G4) |
|--------|------:|------------------------:|---------------:|
| Detection **F1** | **0.825** | ≥0.815 | ≥0.65 (met) |
| **TEDS** (difflib proxy) | **0.475** | ≥0.472 | ≥0.50 (miss) |
| **row** exact | **0.489** | ≥0.484 | ≥0.50 (miss) |
| **col** exact | **0.547** | ≥0.542 | ≥0.55 (miss) |
| Runtime (this machine) | ~26 s / 67 docs | — | Fast preset skips render |

**Multi-peer ranking (same harness, 2026-07-19)** — sorted by detection F1:

| Rank | Tool | F1 | TEDS | row | col | time (s) |
|-----:|------|---:|-----:|----:|----:|---------:|
| **1** | camelot auto | **0.864** | **0.786** | 0.564 | 0.792 | 72.4 |
| **2** | **pdfparser** | **0.825** | 0.475 | 0.489 | 0.547 | 25.4 |
| 3 | pymupdf | 0.776 | 0.674 | 0.578 | 0.642 | 10.9 |
| 4 | camelot lattice (vector) | 0.766 | **0.784** | **0.748** | **0.806** | 3.8 |
| 5 | pdfplumber | 0.662 | 0.650 | 0.571 | 0.533 | 8.7 |

**How to read this for customers**

| If you care about… | Leader | pdfparser |
|--------------------|--------|-----------|
| Finding tables (F1) | Camelot auto | **#2** — close to #1 |
| Structure / content (TEDS) | Camelot auto / lattice | **#5-ish** — main gap |
| Exact row/col counts | Camelot lattice | Behind peers |
| Latency (this board) | Camelot lattice | Mid; use `Fast` preset to skip render probes |
| Pure-Rust / embeddable | — | **Yes** (no Camelot native dep) |

Trajectory: mid-cycle F1 ~**0.50** → **0.825** after Engine V2 + densify + kill-list honesty (no gold pads). Structure (TEDS) remains the product bottleneck.

ICDAR matching is **page order**, not IoU. TEDS here is a **difflib proxy**, not tree-edit TEDS. Full analysis: [`docs/icdar-competitive-report.md`](docs/icdar-competitive-report.md).

---

## Roadmap (what we are not claiming yet)

Prioritized. **Never** retune thresholds on ICDAR filenames. Every detector change A/B’s real_structure + discipline before ship.

### Now → near term

1. **Shape / topology (G3)** — close ICDAR row/col and core shape zeros **without** gold pads.
2. **Cell content (G4)** — TEDS ≥0.50 when shape is right; census / glued multi-col assignment.
3. **Finish control-plane cleanup** — typed flags only (no `notes` string gates).
4. **Expand `TableTuning`** — form/network knobs + named document profiles (`statistical`, `prose_grid`, `invoice`).
5. **Module split** — `ruled` / `network` / `page` god-files for safer review.

### Explicit non-goals (near term)

- Full-page OCR product path
- Ranking #1 on ICDAR as a release gate
- Threshold magic keyed on ICDAR doc names
- Replacing Engine V2 Auto with legacy NMS as the default
- Gold padding / invisible Unicode cells to inflate metrics

### Acceptance bar for shipping detector changes

1. real_structure core cell / det no-regress vs freeze `g2.json` (or intentional freeze bump with review).
2. Nested multi-table (doc **42**) stays outer + inner.
3. Detect discipline exact stays high; fp_strict stays clean.
4. ICDAR re-run is **optional external** evidence — never CI gate.

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

# Document-type densify/lattice tuning (optional)
./target/release/pdfparser extract --tables \
  --table-setting densify_y_skip_numeric_frac=0.10 \
  --format json path.pdf
```

Library:

```rust
use pdfparser::{Document, TableOptions, TablePreset, TextOptions};

let doc = Document::open("file.pdf")?;
let text = doc.page(0)?.text(&TextOptions::default())?;
let mut opts = TableOptions::from_preset(TablePreset::Auto);
// Optional: tune densify for statistical yearbooks / prose grids
opts.apply_tuning_overrides([
    ("densify_y_skip_numeric_frac", 0.10),
])?;
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
