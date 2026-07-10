# pdfparser

[![CI](https://github.com/gauticse2015/pdfparser/actions/workflows/ci.yml/badge.svg)](https://github.com/gauticse2015/pdfparser/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

**Native, library-first PDF parser in Rust.**

`pdfparser` extracts structured content from born-digital (vector/text) PDF files with a security-conscious pipeline (resource limits, no silent encrypted opens in v0.1).

| Status | Capability |
|--------|------------|
| **Available (Phase T)** | Text extraction (reading order, `/Rotate`, encodings / Differences / ToUnicode) |
| **Available (Phase U)** | Lattice (ruled) table extraction — opt-in via `TableOptions` / CLI `--tables` |
| **Next (Phase V)** | Stream/hybrid tables, multi-page stitch, form FP control |

> Not an OCR engine and not a full PDF renderer. Scanned/image-only PDFs are out of scope for v0.1.

---

## Features

- **Library-first API** — idiomatic Rust `Document` / `Page` types for embedders
- **CLI** — thin wrapper for batch extract and inspection
- **Security defaults** — stream expansion budgets, encryption rejected until crypto ships
- **Layout-aware text** — multi-column reading order, page rotation normalize, space insertion
- **Font encodings** — WinAnsi / MacRoman / Differences / ToUnicode (generic, not fixture-specific)
- **Workspace crates** — clear layering (`ir` → `core` → `fonts` / `content` → `layout` → façade)
- **Competitive benchmark harness** — measure text (and later tables) against pdfplumber, PyMuPDF, pypdf, etc.

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
```

### Library usage

Add to your `Cargo.toml` (path dependency until published):

```toml
[dependencies]
pdfparser = { path = "crates/pdfparser" }
```

```rust
use pdfparser::{Document, TextOptions};

fn main() -> pdfparser::Result<()> {
    let doc = Document::open("document.pdf")?;
    println!("pages: {}", doc.page_count());

    let opts = TextOptions {
        sort_reading_order: true,
        insert_spaces: true,
        apply_page_rotate: true,
        include_invisible: true,
    };

    for i in 0..doc.page_count() {
        let page = doc.page(i)?;
        let text = page.text(&opts)?;
        println!("--- page {i} ---\n{text}");
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

# Page range (1-based)
pdfparser extract --pages 1-3 document.pdf

# Document info
pdfparser info document.pdf
```

---

## Workspace layout

```text
pdfparser/
├── crates/
│   ├── pdfparser/           # Public library façade (publish surface)
│   ├── pdfparser-cli/       # Binary: pdfparser
│   ├── pdfparser-ir/        # IR types (TextRun, Rect, extract document)
│   ├── pdfparser-core/      # Open, page tree, filters, resource governor
│   ├── pdfparser-fonts/     # Encodings, widths, ToUnicode
│   ├── pdfparser-content/   # Content-stream lexer + text VM
│   ├── pdfparser-layout/    # Reading order, spaces, page rotate
│   ├── pdfparser-export/    # JSON/text export helpers
│   └── pdfparser-tables/    # Table engine (stub — next phases)
├── benchmark/               # Competitive corpus + accuracy harness
├── docs/                    # Design docs, scoreboards, phase reports
├── schemas/                 # JSON schema placeholders
├── Cargo.toml               # Workspace root
├── LICENSE-MIT
├── LICENSE-APACHE
└── README.md
```

---

## Development

```bash
# Format & lint
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings

# Unit + integration tests (includes Phase T corpus gates)
cargo test --workspace

# Release CLI
cargo build --release -p pdfparser-cli
```

### Accuracy / competitive benchmark

```bash
python3 -m venv .venv && source .venv/bin/activate
pip install -r benchmark/requirements.txt

# Optional: regenerate synthetic + register real fixtures
python benchmark/scripts/generate_corpus.py
python benchmark/scripts/generate_complex_corpus.py
python benchmark/scripts/build_gold_standards.py

cargo build --release -p pdfparser-cli
python benchmark/scripts/run_accuracy_benchmark.py
```

Outputs:

- `benchmark/results/accuracy_results.json`
- `docs/accuracy-scoreboard.md`

See [docs/phase-t-report.md](docs/phase-t-report.md), [docs/phase-u-report.md](docs/phase-u-report.md) for Phase T results and methodology.

---

## Design documentation

| Document | Description |
|----------|-------------|
| [docs/design-native-pdf-parser.md](docs/design-native-pdf-parser.md) | Product design & roadmap |
| [docs/design-architecture-feature-extraction.md](docs/design-architecture-feature-extraction.md) | Architecture / feature LLD |
| [docs/design-redesign-text-tables.md](docs/design-redesign-text-tables.md) | Text parity + table excellence redesign |
| [docs/market-analysis-pdf-parsers.md](docs/market-analysis-pdf-parsers.md) | Competitor analysis |
| [docs/accuracy-scoreboard.md](docs/accuracy-scoreboard.md) | Latest quantitative board |

---

## Roadmap (high level)

1. **Phase T (done)** — text path, encodings, layout, CLI, scoreboard adapter  
2. **Phase U (done)** — lattice tables + cell geometry assign  
3. **Phase V** — stream/hybrid tables, FP control, multi-page stitch  
4. **Later** — encryption subset, richer objects (forms/outline/images), crates.io publish  

---

## Security notes

- **v0.1 does not open encrypted PDFs** (hard error). Decrypt support is planned post-text/table path.
- Stream decoding is budgeted via a **resource governor** (expansion caps). Prefer process isolation when parsing untrusted multi-tenant uploads.
- PDF is a high-risk format; treat untrusted files accordingly.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

Please run `cargo fmt`, `cargo clippy`, and `cargo test` before opening a PR.

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
