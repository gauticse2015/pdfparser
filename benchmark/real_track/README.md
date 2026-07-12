# Real-PDF evaluation track

Primary quality signal for table structure (see `docs/design-table-engine-v2.md`).

## Policy

| Allowed | Forbidden |
|---------|-----------|
| Licensed public / gov / academic PDFs | **ICDAR-2013 competition set** in this tree |
| Soft detect gold + phased full cell grids | Tuning code to named docs |
| Mode tags (`over_detect`, `partial_rules`, …) | File-oriented special cases |

ICDAR remains an **optional external** competitive script only (`run_icdar_competitive.py` + external checkout). Never copy ICDAR PDFs or `*-str.xml` here. Enforced by `benchmark/scripts/assert_no_icdar.py`.

**Corpus program** (archive happy-path synthetic, add hard real PDFs, gold tiers T0–T3, anti self-approval):  
see [`docs/design-table-engine-v2.md`](../../docs/design-table-engine-v2.md) section **Corpus Strategy**.

Synthetic multi-lib boards are **regression only**. Primary quality claims come from this `real_track`.

## Suites

| Suite | Purpose | Gold |
|-------|---------|------|
| `real_detect_smoke` | Detection smoke (count + optional bbox IoU) | Soft count / bbox (promote to T1) |
| `real_structure` | Structure quality (phased 5→15→25 **real** docs; soft-gold seed n=5, G1 locked) | Full cell grids (T3) |
| `real_fp_smoke` | Forms / chrome / prose over-detect | Expect 0 or low table count |
| `geom_unit` (under `corpus/`) | ≤20 synthetic geometry units | Not a market scoreboard |

Manifest index: [`manifests/README.md`](manifests/README.md).

## Provenance & demotion (C0)

| File | Role |
|------|------|
| [`SOURCES.md`](SOURCES.md) | License + URL template and inventory per real doc |
| [`manifests/demotion_list.json`](manifests/demotion_list.json) | compete_synthetic / solved hard demoted from **primary** (PDFs kept) |
| [`manifests/coverage_matrix.json`](manifests/coverage_matrix.json) | Mode → detect/structure docs + waivers |

## Gold schema (v1)

See `schema/gold_table_v1.md`. Structure core docs require `expected_tables[].cells` and preferably `bbox`.

## Stitch multipage (eval vs product)

| Context | `stitch_multipage` | Rationale |
|---------|-------------------|-----------|
| **Product** Auto default | typically **on** (when implemented) | End-user multipage ledgers |
| **`real_structure` / structure metrics** | **must be off** | Page-local gold; K27 exclusive ownership is per-page |
| **`real_detect_smoke` (this harness)** | `unknown_or_default` until CLI flag exists | Smoke counts document- or page-level tables and records the policy note; structure eval still needs stitch **false** |

The CLI may not yet expose a stitch toggle. When it lands, structure runners must pass stitch off explicitly. Do not score structure F1 under stitched root tables against page-local gold.

## Freezes

`freezes/*.json` — Boot then Steady regression (Gate G1). Empty until first Boot write.

## Latency

`manifests/latency_probe.json` lists docs for p50/p95 timing (informative until rebaseline).

## Harness scripts (PR6a / Phase 3 live smoke)

```bash
# IoU metrics unit tests (no binary)
python3 benchmark/scripts/test_iou_metrics.py

# Detect smoke dry-run (no binary required)
python3 benchmark/scripts/run_real_detect_smoke.py --dry-run
python3 benchmark/scripts/run_real_detect_smoke.py --help

# Live detect smoke — release binary required
# Option A: build first, then run
cargo build --release -p pdfparser-cli
python3 benchmark/scripts/run_real_detect_smoke.py

# Option B: let the runner build release if missing
python3 benchmark/scripts/run_real_detect_smoke.py --build

# Optional: exit 1 on count mismatches / extract failures
python3 benchmark/scripts/run_real_detect_smoke.py --strict

# Limit docs / custom out
python3 benchmark/scripts/run_real_detect_smoke.py --limit 3
# → benchmark/real_track/results/real_detect_smoke_latest.json
#    per-doc: n_pred, n_exp, ok; summary det_proxy (count-based P/R);
#    stitch_multipage: "unknown_or_default"; structure_eval_needs_stitch_false: true

# ICDAR refuse (always green in this tree)
python3 benchmark/scripts/assert_no_icdar.py
```

**Release CLI note:** build with `cargo build --release -p pdfparser-cli` (binary at
`target/release/pdfparser`). There is no required `benchmark/scripts/build_release_cli.sh`;
if you add a local helper script, keep it optional and document it here only.

Default live exit is **0** with a summary even when counts mismatch (soft gold).
Missing binary without `--build` exits **2**. Use `--strict` for CI hard fail on DIFF/ERR.

IoU matching lives in `benchmark/scripts/metrics.py` (`table_detection_metrics_iou`, thresh 0.5 greedy bipartite).
