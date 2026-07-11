# pdfparser

[![CI](https://github.com/gauticse2015/pdfparser/actions/workflows/ci.yml/badge.svg)](https://github.com/gauticse2015/pdfparser/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

**Native, library-first PDF parser in Rust** for born-digital (vector/text) PDFs.

`pdfparser` extracts text, tables, and common document objects through a security-conscious pipeline (resource limits, no silent encrypted opens in v0.1). Tables are **opt-in** and multi-strategy: lattice (ruled), hybrid (partial rules), and network (borderless).

> Not an OCR engine and not a full page renderer. Full-page scan OCR is out of scope for v0.1; **embedded image-painted grids** are recovered via Camelot-class raster morphology.

---

## Capabilities

| Area | Status | What you get |
|------|--------|----------------|
| **Text** | Production | Reading-order sort, `/Rotate`, WinAnsi / MacRoman / Differences / ToUnicode, space insertion |
| **Ruled tables (lattice)** | Production | Vector H/V rules + **thin-fill painted rect rules**, multi-region CC, spans, text densify-Y, false-underline collapse |
| **Raster line sensing** | Production | Camelot-class morph on **embedded Image XObjects**: adaptive threshold, dashed-gap close, H/V open, multi-scale, joint-graph + regularity gate → lattice rules |
| **Partial borders (hybrid)** | Production | Incomplete frames recovered with text columns/rows |
| **Borderless tables (network)** | Production | Textline + column-alignment builder; gap split + same-schema re-merge; list/prose FP reject |
| **Orchestration** | Production | Strong lattice **excludes** overlapping borderless tables (no dual soup by default) |
| **Multi-page** | Production | Optional stitch when continued tables share header/columns |
| **FP control** | Production | Form-like chrome scrub, caption/tiny grid reject, 2-col prose / numbered-list reject; chart-axis raster reject |
| **Objects** | Production | Image count/meta, URI links, AcroForm fields, outline titles |
| **Security** | Production | Stream expansion budgets; encrypted PDFs hard-error until crypto ships |
| **Full-page render OCR** | Not yet | Deferred; optional external render path, not the product table core |

### Table pipeline (product default: `TablePreset::Auto`)

```text
page rules + text + embedded images
    → raster morph (Image XObject → H/V rules, grid-validated)
    → lattice (vector + thin-fill + raster rules)
    → hybrid only outside strong lattice
    → network borderless only outside strong lattice
    → form scrub + NMS
```

---

## Accuracy benchmarks

**Last refreshed:** 2026-07-11 (release CLI + current table engine).

Two different measurement tracks — **do not mix them**:

| Track | What it is | pdfparser status |
|-------|------------|------------------|
| **Owned multi-lib harness** | Synthetic + designed fixtures in this repo | **#1** on main / hard / precision / compete_hard |
| **ICDAR-2013 competitive** | External 67-PDF Camelot ICDAR set | **#5** — **not SOTA** (F1 0.58, TEDS 0.33) |

### Metric definitions

| Metric | Meaning |
|--------|---------|
| **overall** | Weighted 0–100 score (text / tables / objects per-doc GT weights) — owned harness only |
| **text F1** | Token F1 on `must_contain` / required substrings |
| **det F1** | Table **count** detection F1 vs gold |
| **cell F1** | Aligned cell-text micro-F1 (owned harness) |
| **shape** | Fraction of gold tables with exact R×C |
| **row / col** | Exact row / column count accuracy |
| **F1 / TEDS** | ICDAR only — Camelot `bench/_metrics.score` (detection F1 + difflib TEDS proxy) |
| **ms / time** | Mean wall time per doc (owned) or full-set seconds (ICDAR) |

### 1) Main multi-library scoreboard (owned corpus)

Suite: basic + stress + hard synthetic (full grid gold where available).  
Source: `benchmark/results/accuracy_results.json`.

| Rank | Library | overall | det F1 | cell F1 | shape | ms/doc |
|-----:|---------|--------:|-------:|--------:|------:|-------:|
| 1 | **pdfparser** | **98.9** | **0.969** | **0.977** | **0.958** | **26** |
| 2 | pdfplumber | 89.1 | 0.919 | 0.860 | 0.812 | 61 |
| 3 | pymupdf | 88.8 | 0.889 | 0.816 | 0.812 | 73 |
| 4 | camelot auto | 67.3 | 0.919 | 0.848 | 0.792 | 376 |
| 5 | camelot stream | 60.4 | 0.707 | 0.333 | 0.302 | 11 |
| 6 | camelot lattice | 59.0 | 0.889 | 0.821 | 0.771 | 26 |
| 7 | img2table | 57.8 | 0.854 | 0.769 | 0.771 | 73 |
| 8 | pypdf | 39.4 | 0.242 | 0.000 | 0.000 | 8 |
| 9 | pypdfium2 | 36.1 | 0.242 | 0.000 | 0.000 | 2 |
| 10 | pdfminer.six | 36.0 | 0.242 | 0.000 | 0.000 | 66 |

> **Reading this table:** #1 on *owned* regression is product progress, **not** a claim of SOTA on wild PDFs. See ICDAR (§3).

### 2) By owned suite (multi-lib)

#### Basic + stress

Source: `accuracy_results_basic_stress.json`.

| Library | overall | cell F1 | det F1 | shape | ms/doc |
|---------|--------:|--------:|-------:|------:|-------:|
| **pdfparser** | **98.1** | **0.950** | **0.947** | **0.909** | **11** |
| pdfplumber | 88.1 | 0.826 | 0.933 | 0.727 | 93 |
| camelot auto | 54.3 | 0.826 | 0.883 | 0.727 | 338 |
| camelot lattice | 46.9 | 0.826 | 0.933 | 0.727 | 36 |

#### Hard structure 50–62

Source: `accuracy_results_hard.json`.

| Library | overall | cell F1 | det F1 | shape | ms/doc |
|---------|--------:|--------:|-------:|------:|-------:|
| **pdfparser** | **100.0** | **1.000** | **1.000** | **1.000** | **8** |
| pdfplumber | 90.7 | 0.890 | 0.897 | 0.885 | 14 |
| camelot auto | 87.1 | 0.866 | 0.974 | 0.846 | 430 |
| camelot lattice | 77.7 | 0.817 | 0.821 | 0.808 | 40 |

#### Precision FP suite 70–82

Source: `accuracy_results_regression_precision.json`.

| Library | overall | cell F1 | det F1 | shape | ms/doc |
|---------|--------:|--------:|-------:|------:|-------:|
| **pdfparser** | **99.4** | **1.000** | **1.000** | **1.000** | **7** |
| pdfplumber | 89.8 | 0.894 | 0.897 | 0.833 | 8 |
| camelot lattice | 83.2 | 0.900 | 0.897 | 0.833 | 34 |
| camelot auto | 81.3 | 0.900 | 0.897 | 0.833 | 392 |

#### Compete hard struggle suite C100–C180 (81 docs)

Source: `accuracy_results_compete_hard.json` (multi-lib).  
Freeze: `compete_hard_baseline_post_generic.json`.

| Rank | Library | cell F1 | det F1 | shape | overall | ms/doc |
|-----:|---------|--------:|-------:|------:|--------:|-------:|
| 1 | **pdfparser** | **0.827** | **0.945** | **0.846** | **87.5** | **10** |
| 2 | camelot stream | 0.630 | 0.913 | 0.543 | 69.8 | 25 |
| 3 | camelot auto | 0.328 | 0.795 | 0.284 | 48.1 | 412 |
| 4 | pdfplumber | 0.319 | 0.735 | 0.247 | 47.0 | 14 |
| 5 | camelot lattice | 0.319 | 0.735 | 0.247 | 44.1 | 32 |
| 6 | pymupdf | 0.312 | 0.735 | 0.247 | 46.7 | 17 |

Progress vs pre-algorithm freeze (pdfparser only): cell **0.383 → 0.827**, shape **0.204 → 0.846**, overall **50.4 → 87.5**.

Capabilities exercised: lattice densify X/Y, network regioning, embedded-image raster lines, exclusive-under-lattice, FP scrub.  
Still open on this suite: image **cell text** (OCR), complex spans, invoice footer shape, severe overdetect count, header-slice row exactness.

### 3) External competitive: ICDAR-2013 (67 PDFs) — **market claim source of truth**

Black-box head-to-head on **Camelot-shipped ICDAR-2013** PDFs + `*-str.xml`, scored with Camelot `bench/_metrics.score`.  
**ICDAR is not in this repo** and **not** part of regression.  
Source: `benchmark/results/camelot_icdar_headtohead.json` · full write-up: `docs/icdar-competitive-report.md`.

| Rank | Tool | F1 | TEDS | row | col | time (s, full set) |
|-----:|------|---:|-----:|----:|----:|-------------------:|
| 1 | camelot auto | **0.864** | **0.786** | 0.564 | 0.792 | 72.6 |
| 2 | pymupdf | 0.776 | 0.674 | 0.578 | 0.642 | 10.7 |
| 3 | camelot lattice (vector) | 0.766 | 0.784 | **0.748** | **0.806** | 3.5 |
| 4 | pdfplumber | 0.662 | 0.650 | 0.571 | 0.533 | 8.5 |
| **5** | **pdfparser** | **0.584** | **0.333** | 0.338 | 0.500 | **0.80** |

| vs prior ICDAR snapshot | Previous | Now | Δ |
|-------------------------|---------:|----:|--:|
| F1 | 0.672 | **0.584** | **−0.088** |
| TEDS | 0.331 | **0.333** | +0.001 |
| row | 0.382 | 0.338 | −0.043 |
| col | 0.489 | 0.500 | +0.011 |

Dominant pdfparser failure modes on ICDAR: **over-detect**, wrong shape, row/col miscount (see competitive report).

### Honest summary

| Track | pdfparser | Claim |
|-------|-----------|--------|
| Owned multi-lib regression / hard / precision | **#1** | Strong born-digital product path |
| Owned compete_hard struggle (81 docs) | **#1** vs open-source peers | Geometry progress is real on designed hard modes |
| **ICDAR-2013 competitive** | **#5**, F1 **0.58**, TEDS **0.33** | **Not SOTA.** Behind Camelot and PyMuPDF on quality; fastest latency |
| Speed | ~8–26 ms/doc owned; **0.8 s / 67 ICDAR docs** | ~90× faster than camelot auto on ICDAR set |

### Reproducing benchmarks

```bash
python3 -m venv .venv && source .venv/bin/activate
pip install -r benchmark/requirements.txt
cargo build --release -p pdfparser-cli

# Owned multi-library boards
python benchmark/scripts/run_accuracy_benchmark.py
python benchmark/scripts/run_accuracy_benchmark.py --suite regression_hard --tag hard
python benchmark/scripts/run_accuracy_benchmark.py --suite regression_precision --tag regression_precision
python benchmark/scripts/run_accuracy_benchmark.py --suite regression_compete_hard --tag compete_hard

# ICDAR competitive (external data — market claim)
python benchmark/scripts/run_icdar_competitive.py \
  --data-dir /path/to/icdar2013-dataset \
  --camelot-root /path/to/camelot-upstream
```

Scoreboards: `docs/accuracy-scoreboard.md`, `docs/accuracy-scoreboard-hard.md`, `docs/accuracy-scoreboard-compete-hard.md`, `docs/icdar-competitive-report.md`.

---

## Quick start

### Requirements

- Rust **1.75+** ([rustup](https://rustup.rs/))
- Optional (benchmarks only): Python **3.10+**

### Install CLI from source

```bash
git clone https://github.com/gauticse2015/pdfparser.git
cd pdfparser
cargo build --release -p pdfparser-cli
./target/release/pdfparser extract path/to/file.pdf
./target/release/pdfparser extract --tables --format json path/to/file.pdf
```

### Library usage

```toml
[dependencies]
pdfparser = { path = "crates/pdfparser" }
```

```rust
use pdfparser::{Document, TableOptions, TablePreset, TextOptions};

fn main() -> pdfparser::Result<()> {
    let doc = Document::open("document.pdf")?;
    let text_opts = TextOptions::default();
    let table_opts = TableOptions::from_preset(TablePreset::Auto);

    for i in 0..doc.page_count() {
        let page = doc.page(i)?;
        let text = page.text(&text_opts)?;
        let tables = page.tables(&text_opts, &table_opts)?;
        println!("page {i}: {} chars, {} tables", text.len(), tables.len());
    }
    Ok(())
}
```

### CLI

```bash
# Plain text (reading order)
pdfparser extract document.pdf

# JSON (pages + metadata)
pdfparser extract --format json document.pdf

# Tables (Auto pipeline)
pdfparser extract --tables --format json document.pdf

# Page range (1-based)
pdfparser extract --pages 1-3 document.pdf

pdfparser info document.pdf
```

---

## Workspace layout

```text
pdfparser/
├── crates/
│   ├── pdfparser/           # Public library façade
│   ├── pdfparser-cli/       # Binary: pdfparser
│   ├── pdfparser-ir/        # IR types
│   ├── pdfparser-core/      # Open, page tree, filters, limits
│   ├── pdfparser-fonts/     # Encodings, widths, ToUnicode
│   ├── pdfparser-content/   # Content-stream VM (+ thin-fill rules)
│   ├── pdfparser-layout/    # Reading order, spaces, rotate
│   ├── pdfparser-export/    # JSON helpers
│   └── pdfparser-tables/    # Lattice / hybrid / network tables
├── benchmark/               # Corpus, gold, multi-lib harness
├── docs/                    # Design, scoreboards, competitive notes
└── README.md
```

---

## Development

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release -p pdfparser-cli
```

---

## Design documentation

| Document | Description |
|----------|-------------|
| [docs/design-native-pdf-parser.md](docs/design-native-pdf-parser.md) | Product design & roadmap |
| [docs/design-architecture-feature-extraction.md](docs/design-architecture-feature-extraction.md) | Architecture / feature LLD |
| [docs/table-orchestrator-architecture.md](docs/table-orchestrator-architecture.md) | Production table pipeline |
| [docs/compete-struggle-analysis.md](docs/compete-struggle-analysis.md) | Hard-suite failure taxonomy |
| [docs/icdar-competitive-report.md](docs/icdar-competitive-report.md) | ICDAR head-to-head report |
| [docs/accuracy-scoreboard.md](docs/accuracy-scoreboard.md) | Latest multi-lib owned board |

---

## Roadmap

1. **Done** — text path, lattice/hybrid/network tables, embedded-image raster line sensing, objects, owned multi-lib scoreboard  
2. **In progress** — compete_hard residual (header-slice row exact, spans, invoice shape, overdetect count)  
3. **Next** — ICDAR TEDS/row/col; optional full-page render / OCR for image text  
4. **Later** — encryption subset, structure tree, crates.io publish  

---

## Security notes

- **v0.1 does not open encrypted PDFs** (hard error).  
- Stream decoding is budgeted via a **resource governor**. Prefer process isolation for untrusted multi-tenant uploads.  
- PDF is a high-risk format; treat untrusted files accordingly.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Run `cargo fmt`, `cargo clippy`, and `cargo test` before opening a PR.

---

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
