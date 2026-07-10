# Phase T Implementation Report — Text Parity

| Field | Value |
|-------|-------|
| **Date** | 2026-07-10 |
| **Phase** | T (Text parity) — T0–T6 |
| **Status** | **Complete** — production-ready text path; text_token_f1 **matched** competitors after encoding fix |
| **Crate version** | 0.1.0 |
| **Scoreboard** | `benchmark/results/accuracy_results.json` (includes `pdfparser`) |
| **Human board** | `docs/accuracy-scoreboard.md` |

---

## Delivered (production layout)

```text
crates/
  pdfparser-ir/        # Rect, TextRun, ExtractedDocument, WarningCode
  pdfparser-core/      # open, governor, filters (Flate+Predictor, ASCII*, LZW, RLE), page tree
  pdfparser-fonts/     # encodings, widths, ToUnicode CMap (bfchar/bfrange)
  pdfparser-content/   # lexer + text graphics VM (Tj/TJ/Tm/…)
  pdfparser-layout/    # R8 page rotate, space insertion, multi-column reading order
  pdfparser-export/    # JSON helpers
  pdfparser-tables/    # stub (Phase U/V)
  pdfparser/           # public Document / Page API
  pdfparser-cli/       # `pdfparser extract|info` binary
```

**Public API (library-first):**

```rust
let doc = pdfparser::Document::open("file.pdf")?;
let text = doc.page(0)?.text(&TextOptions::default())?;
```

**CLI:**

```bash
cargo build --release -p pdfparser-cli
./target/release/pdfparser extract file.pdf
./target/release/pdfparser extract --format json file.pdf
./target/release/pdfparser info file.pdf
```

**Security (K15/K16):** encrypted PDFs → hard `Error::Encryption`; stream decode under ResourceGovernor.

---

## Phase T gates (design §13)

| Gate | Requirement | Result |
|------|-------------|--------|
| **T0** | Workspace compiles | **PASS** |
| **T1** | Backend + governor + filters | **PASS** |
| **T2** | Content VM + fonts | **PASS** |
| **T3** | `11_rotated_page` token F1 = 1.0 | **PASS** (1.0) |
| **T4** | `02_multi_column` reading order score 1.0 | **PASS** (markers L→R order) |
| **T5** | basic text_token_f1 ≥ 0.99 | **PASS** (**1.0** mean on 11 basic text fixtures) |
| **T6** | Lazy pages; report latency | **PASS** (mean wall ~18 ms; 80-page doc ~22 ms extract) |

**Automated tests:** `cargo test -p pdfparser --test phase_t_text --release`

---

## Accuracy scoreboard — `pdfparser` vs competitors

> **Note on an earlier premature “pass”:** An intermediate report claimed Phase T success while **overall** `text_token_f1` (~0.97) was still below competitors because **generic font `/Differences` encoding** was not implemented. That was a **real accuracy bug**, not “we cannot match SOTA.” It is fixed. Gates must use the **full scoreboard text metrics**, not only the simple synthetic subset.

### Text token F1 (after encoding fix — current)

| Library | Mean text_token_f1 | text_score | Notes |
|---------|-------------------:|-----------:|-------|
| **pdfparser (ours)** | **1.000** | **100** | Matches pypdf; ≥ plumber/mupdf/pdfium |
| **pypdf** | **1.000** | 100 | |
| pymupdf | 0.992 | 99.2 | |
| pdfplumber | 0.992 | 98.4 | |
| pypdfium2 | 0.992 | 99.2 | Fastest |
| pdfminer.six | 0.984 | 97.6 | |

(Encrypted doc is not decrypted in Phase T — excluded as hard failure on open, by product K15.)

### Root cause of the prior gap (generic, not corpus-specific)

Many production PDFs use **custom encodings**: `/Encoding << /BaseEncoding /WinAnsiEncoding /Differences [ 1 /T /h /e … ] >>`.  
We previously mapped bytes as Latin-1 and ignored **Differences** and under-used **ToUnicode**. That produces garbage on real documents (e.g. Fed Beige Book).  

**Fix:** parse Differences + glyph-name → Unicode (generic AGL-style names), prefer ToUnicode when present. No fixture hardcoding.

### Tables (expected for Phase T)

| Metric | pdfparser | Note |
|--------|----------:|------|
| table_cell_f1 | 0.0 | **No table engine yet** (Phase U/V) |
| table_detect_f1 | ~0.38 | Same class as pypdf/pdfium (no tables API) |

Overall score is still table-weighted and is **not** the Phase T success metric.

### Encryption

| Case | Result |
|------|--------|
| Encrypted open | **Rejected** (`Error::Encryption`) — product K15 |
| Password decrypt | **Not in Phase T** (0.2) |

---

## Known limitations (Phase T → next)

1. **No table extraction** — Phase U/V per redesign (this is why overall score is lower than pymupdf).
2. **No forms/outline/images export** — later PRs.
3. **No encryption** — K15 intentional.
4. **Glyph-name coverage** is a practical subset of Adobe Glyph List; rare names still fall back (generic extension path exists).
5. Spacing/kerning on some real PDFs may still insert odd spaces — continuous improvement, not fixture hacks.

---

## How to re-run scoreboard

```bash
source "$HOME/.cargo/env"
cargo build --release -p pdfparser-cli
source .venv/bin/activate
python benchmark/scripts/run_accuracy_benchmark.py
# outputs:
#   benchmark/results/accuracy_results.json
#   docs/accuracy-scoreboard.md
```

---

## Next phase

**Phase U** — `pdfparser-tables` foundation: geometry index, lattice S2, R9 cell assign, gates on `06/09/10` cell_f1 ≥ 0.95.

---

*Phase T closed 2026-07-10*
