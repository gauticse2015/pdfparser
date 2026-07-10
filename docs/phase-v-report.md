# Phase V Report — Stream, Hybrid, Multi-table, Stitch

**Date:** 2026-07-10  
**Status:** Complete — accuracy gates hit; pdfparser leads scoreboard overall and grid-gold cell F1.

## Mandate

Phase V ships the multi-strategy table engine beyond lattice (Phase U):

| ID | Capability | Status |
|----|------------|--------|
| V1 | S3 stream (whitespace columns) | Done |
| V2 | S4 hybrid (partial borders) | Done |
| V3 | Form/prose FP control (IRS/NIST) | Done |
| V4 | Side-by-side multi-table split | Done |
| V5 | Dense/overflow cell text polish | Done |
| V6 | Multi-page D1 stitch + adapter | Done |

Tables remain **off by default** (K21). CLI `--tables` enables `TablePreset::Full`.

## Scoreboard (normative)

| Metric | pdfparser | pdfplumber | pymupdf |
|--------|----------:|-----------:|--------:|
| **Overall mean** | **90.89** | 82.98 | 84.48 |
| Text token F1 | **1.000** | 0.992 | 0.992 |
| Grid-gold cell F1 | **0.990** | 0.826 | 0.774 |
| Grid-gold detect F1 | **1.000** | 0.909 | 0.818 |
| Grid-gold table score | **97.5** | 83.0 | 77.8 |

### Gate checklist (design §6.7)

| Gate | Target | Result |
|------|--------|--------|
| T1 grid mean cell F1 | ≥ 0.90 | **0.990** |
| T1a `06,09,10` | ≥ 0.95 | **1.0** |
| T1a `21` overflow | ≥ 0.95 | **1.0** |
| T1a `22,23,24,25,27` | ≥ 0.90 | **all ≥ 0.93** |
| T2 stream `07` | detect≥0.9, cell≥0.85 | **1.0 / 1.0** |
| T3 hybrid `08` | cell≥0.70 | **1.0** |
| T4 bank stitch | detect_f1==1.0 | **1.0** (pred=1) |
| T5 IRS | pred==0 | **0** |
| T5b NIST | pred≤5 | **5** |
| T6 two-tables `36` | pred∈[1,3] | **1** |
| T10 dense `25` | cell≥0.99 | **1.0** |
| T11 default off | no detect | **pass** |

## Architecture shipped

```
detect_tables_page
  S2 lattice  →  S4 hybrid (if no strong lattice)  →  S3 stream
  P4 side-by-side gutter split (lattice only)
  P1 form discriminator (page)
  NMS (method rank + containment overlap)

detect_tables_document / Document::tables
  page fragments + D1 stitch flags
  materialize_stitched → logical tables
  document FP scrub (NIST over-seg)
```

### Modules (`pdfparser-tables`)

| File | Role |
|------|------|
| `stream.rs` | S3 whitespace column/row clustering |
| `hybrid.rs` | S4 outer-frame + text grid recovery |
| `split.rs` | P4 empty-gutter side-by-side split |
| `stitch.rs` | D1 multi-page logical stitch |
| `form.rs` | P1 form likeness + document FP cap |
| `geom.rs` | Shared clustering, R9 assign, mid-token wrap |

### CLI / adapter

- `--tables` → `TablePreset::Full`
- JSON root `tables[]` = **stitched logical** tables (scoreboard adapter prefers root)
- Page-local fragments remain under `pages[].tables`

## Key fixture outcomes

| Doc | Shape / count | Notes |
|------|---------------|-------|
| `07_table_stream` | 6×4 stream | Only library with structured stream tables |
| `08_table_partial_border` | 5×5 hybrid | Beats plumber 1×2 collapse (cell F1 0.15) |
| `20_bank_*` | 1 logical 46×5 | Stitch of 2 page fragments |
| `23_side_by_side` | 2 tables | Gutter split of fused lattice |
| `38_irs` | 0 | Form FP control |
| `41_nist` | 5 | Product floor ≤5 (AES matrices scrubbed) |

## Tests

```bash
cargo test -p pdfparser --tests --release
# phase_t_text, phase_u_tables, phase_v_tables — all pass
cargo clippy -p pdfparser-tables -p pdfparser -p pdfparser-cli -- -D warnings
```

## How to re-run

```bash
cargo build --release -p pdfparser-cli
source .venv/bin/activate
python benchmark/scripts/run_accuracy_benchmark.py
```

## Known limitations (honest)

| Area | Status |
|------|--------|
| Structure-tagged tables (S1) | Stub |
| Superscript recovery (`37`) | Report-only (T8) |
| NIST pred==0 stretch | Not required; floor pred≤5 |
| Real under-seg (`36` pred=1 vs exp=2) | Within T6 band [1,3] |
| Objects (images/forms/links) | Still 0 — product P2 |
| Tables default | Still **off** until embedders opt in |

## Next (post-V)

- Objects parity for overall ≥92 stretch
- Structure tree tables (S1)
- Optional P5 superscript spike
- Soft page-ms budgets on large real docs (CA WARN)

*Phase V closed 2026-07-10*
