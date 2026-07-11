# pdfparser

[![CI](https://github.com/gauticse2015/pdfparser/actions/workflows/ci.yml/badge.svg)](https://github.com/gauticse2015/pdfparser/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

**Native, library-first PDF parser in Rust** for born-digital (vector/text) PDFs.

`pdfparser` extracts text, tables, and common document objects through a security-conscious pipeline (resource limits, no silent encrypted opens in v0.1). Tables are **opt-in** and multi-strategy: lattice (ruled), hybrid (partial rules), and network (borderless).

> Not an OCR engine and not a full page renderer. Scanned / image-only PDFs are out of scope for v0.1 (except thin-fill painted rules in the content stream).

---

## Capabilities

| Area | Status | What you get |
|------|--------|----------------|
| **Text** | Production | Reading-order sort, `/Rotate`, WinAnsi / MacRoman / Differences / ToUnicode, space insertion |
| **Ruled tables (lattice)** | Production | Vector H/V rules + **thin-fill painted rect rules**, multi-region CC, spans, text densify-Y, false-underline collapse |
| **Partial borders (hybrid)** | Production | Incomplete frames recovered with text columns/rows |
| **Borderless tables (network)** | Production | Textline + column-alignment builder; gap split + same-schema re-merge; list/prose FP reject |
| **Orchestration** | Production | Strong lattice **excludes** overlapping borderless tables (no dual soup by default) |
| **Multi-page** | Production | Optional stitch when continued tables share header/columns |
| **FP control** | Production | Form-like chrome scrub, caption/tiny grid reject, 2-col prose / numbered-list reject |
| **Objects** | Production | Image count/meta, URI links, AcroForm fields, outline titles |
| **Security** | Production | Stream expansion budgets; encrypted PDFs hard-error until crypto ships |
| **Raster ROI lattice** | Not yet | Needed for pure image-grid tables (Camelot-class line morph) |

### Table pipeline (product default: `TablePreset::Auto`)

```text
page rules + text
    → lattice (vector + thin-fill rules)
    → hybrid only outside strong lattice
    → network borderless only outside strong lattice
    → form scrub + NMS
```

---

## Accuracy benchmarks

Numbers below are from the **owned accuracy harness** (`benchmark/scripts/run_accuracy_benchmark.py`) and the **external ICDAR-2013 competitive** runner. Re-run commands are at the end of this section.

### Metric definitions

| Metric | Meaning |
|--------|---------|
| **overall** | Weighted 0–100 score (text / tables / objects per-doc GT weights) |
| **text F1** | Token F1 on `must_contain` / required substrings |
| **det F1** | Table **count** detection F1 vs gold |
| **cell F1** | Aligned cell-text micro-F1 (TEDS-like content quality) |
| **shape** | Fraction of gold tables with exact R×C |
| **row / col** | Exact row / column count accuracy |
| **TEDS** | ICDAR only — Camelot-style difflib structure proxy (not tree-edit TEDS) |
| **ms** | Mean wall time per doc (library adapter) |

### 1) Main multi-library scoreboard (owned corpus, 33 docs)

Suite mixes basic + stress + hard synthetic fixtures with full grid gold where available.  
Source: `benchmark/results/accuracy_results.json`.

| Rank | Library | overall | text F1 | det F1 | cell F1 | shape | row | col | ms/doc |
|-----:|---------|--------:|--------:|-------:|--------:|------:|----:|----:|-------:|
| 1 | **pdfparser** | **99.3** | **1.000** | **0.938** | **0.994** | **0.958** | **0.958** | **1.000** | **11** |
| 2 | pdfplumber | 89.1 | 0.992 | 0.919 | 0.860 | 0.812 | 0.812 | 0.854 | 60 |
| 3 | pymupdf | 88.8 | 0.992 | 0.889 | 0.816 | 0.812 | 0.812 | 0.854 | 73 |
| 4 | camelot auto | 67.3 | 0.585 | 0.919 | 0.848 | 0.792 | 0.792 | 0.896 | 375 |
| 5 | camelot lattice | 59.0 | 0.456 | 0.889 | 0.821 | 0.771 | 0.771 | 0.812 | 26 |
| 6 | img2table | 57.8 | 0.444 | 0.854 | 0.769 | 0.771 | 0.771 | 0.812 | 73 |
| 7 | pypdf | 39.4 | 1.000 | 0.242 | 0.000 | 0.000 | 0.000 | 0.000 | 8 |
| 8 | pypdfium2 | 36.1 | 0.992 | 0.242 | 0.000 | 0.000 | 0.000 | 0.000 | 2 |
| 9 | pdfminer.six | 36.0 | 0.985 | 0.242 | 0.000 | 0.000 | 0.000 | 0.000 | 67 |

> **Reading this table:** pdfparser leads on *owned* synthetic/regression grids it was engineered against. That is real product progress, not a claim of SOTA on all real-world PDFs (see ICDAR below).

### 2) By owned suite (pdfparser vs peers)

#### Basic + stress (20 docs)

Source: `accuracy_results_basic_stress.json`.

| Library | overall | cell F1 | det F1 | shape | row | col | ms/doc |
|---------|--------:|--------:|-------:|------:|----:|----:|-------:|
| **pdfparser** | **99.8** | **0.992** | 0.895 | **1.000** | **1.000** | **1.000** | **11** |
| pdfplumber | 88.1 | 0.826 | **0.933** | 0.727 | 0.727 | 0.818 | 94 |
| camelot auto | 54.3 | 0.826 | 0.883 | 0.727 | 0.727 | 0.909 | 334 |
| camelot lattice | 46.9 | 0.826 | 0.933 | 0.727 | 0.727 | 0.818 | 32 |

#### Hard structure 50–62 (13 docs)

Source: `accuracy_results_hard.json`.

| Library | overall | cell F1 | det F1 | shape | row | col | ms/doc |
|---------|--------:|--------:|-------:|------:|----:|----:|-------:|
| **pdfparser** | **100.0** | **1.000** | **1.000** | **1.000** | **1.000** | **1.000** | 52 |
| pdfplumber | 90.7 | 0.890 | 0.897 | 0.885 | 0.885 | 0.885 | **16** |
| camelot auto | 87.1 | 0.866 | 0.974 | 0.846 | 0.846 | 0.885 | 439 |
| camelot lattice | 77.7 | 0.817 | 0.821 | 0.808 | 0.808 | 0.808 | 56 |

#### Precision FP suite 70–82 (13 docs)

Source: `accuracy_results_regression_precision.json`.

| Library | overall | cell F1 | det F1 | shape | row | col | ms/doc |
|---------|--------:|--------:|-------:|------:|----:|----:|-------:|
| **pdfparser** | **99.4** | **1.000** | **1.000** | **1.000** | **1.000** | **1.000** | **7** |
| camelot lattice | 83.2 | 0.900 | 0.897 | 0.833 | 0.917 | 0.833 | 32 |
| camelot auto | 81.3 | 0.900 | 0.897 | 0.833 | 0.917 | 0.833 | 384 |
| pdfplumber | 89.8 | 0.894 | 0.897 | 0.833 | 0.917 | 0.833 | 7 |

#### Sensing suite 90–95 (6 docs, pdfparser)

Source: `accuracy_results_sensing_prod.json`.

| Library | n | cell F1 | det F1 | shape | row | col | overall |
|---------|--:|--------:|-------:|------:|----:|----:|--------:|
| **pdfparser** | 6 | **1.000** | **1.000** | **1.000** | **1.000** | **1.000** | **100.0** |

Painted thin-fill rules, multi large lattices, borderless prose gap, partial H densify, invoice footer strip, multi-col prose FP reject.

#### Compete hard struggle suite C100–C180 (81 docs, pdfparser)

Source: `accuracy_results_compete_hard_prod.json` (open struggle modes; full cell gold).

| Library | n | cell F1 | det F1 | shape | row | col | overall |
|---------|--:|--------:|-------:|------:|----:|----:|--------:|
| **pdfparser** | 81 | **0.445** | 0.785 | 0.346 | 0.568 | 0.509 | 55.8 |

This suite is intentionally hard (image-only rules, partial V, header-slice borderless, underlines, multi-table chaos). It is the primary **development target** for remaining quality gaps—not a victory board.

### 3) External competitive: ICDAR-2013 (67 PDFs)

Black-box head-to-head using Camelot’s competitive metrics (detection F1, TEDS proxy, row/col).  
**ICDAR files are not in this repo** (external clone only).  
Source: `benchmark/results/camelot_icdar_headtohead.json`.

| Rank | Tool | F1 | TEDS | row | col | time (s, full set) |
|-----:|------|---:|-----:|----:|----:|-------------------:|
| 1 | camelot auto | **0.864** | **0.786** | 0.564 | **0.792** | 72.3 |
| 2 | pymupdf | 0.776 | 0.674 | 0.578 | 0.642 | 10.8 |
| 3 | camelot lattice (vector) | 0.766 | 0.784 | **0.748** | 0.806 | 3.6 |
| 4 | **pdfparser** | **0.672** | **0.331** | 0.382 | 0.489 | **0.58** |
| 5 | pdfplumber | 0.662 | 0.650 | 0.571 | 0.533 | 8.4 |

**Honest takeaway**

| Track | pdfparser | Interpretation |
|-------|-----------|----------------|
| Owned synthetic / hard / precision / sensing | **#1**, often ~1.0 cell F1 | Product regression for digital ruled + many borderless cases is strong and fast |
| ICDAR competitive | **#4**, TEDS **0.33** | Not Camelot-class on wild real competition pages yet (raster lines + network-class borderless still open) |
| Latency | **~0.6 s / 67 docs** | ~100× faster than camelot auto on that set |

### Reproducing benchmarks

```bash
python3 -m venv .venv && source .venv/bin/activate
pip install -r benchmark/requirements.txt
cargo build --release -p pdfparser-cli

# Multi-library owned scoreboard
python benchmark/scripts/run_accuracy_benchmark.py

# Hard / precision / sensing / compete hard
python benchmark/scripts/run_accuracy_benchmark.py --suite regression_hard --tag hard
python benchmark/scripts/run_accuracy_benchmark.py --suite regression_precision --tag regression_precision
python benchmark/scripts/run_accuracy_benchmark.py --suite regression_sensing --tag sensing
python benchmark/scripts/run_accuracy_benchmark.py --suite regression_compete_hard --libs pdfparser --tag compete_hard

# Optional: regenerate synthetic corpora
python benchmark/scripts/generate_hard_corpus.py
python benchmark/scripts/generate_sensing_corpus.py
python benchmark/scripts/generate_compete_corpus.py
python benchmark/scripts/generate_compete_hard_corpus.py

# ICDAR competitive (requires external Camelot/ICDAR checkout — see docs)
python benchmark/scripts/run_icdar_competitive.py
```

Scoreboards: `docs/accuracy-scoreboard.md`, `docs/accuracy-scoreboard-hard.md`, `docs/accuracy-scoreboard-sensing.md`, `docs/icdar-competitive-report.md`, `docs/compete-struggle-analysis.md`.

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

1. **Done** — text path, lattice/hybrid/network tables, objects, owned multi-lib scoreboard  
2. **In progress** — hard struggle suite (compete_hard); network + exclusive lattice quality  
3. **Next** — ROI raster line sensing for image-painted grids; deeper network alignments; ICDAR TEDS/row/col  
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
