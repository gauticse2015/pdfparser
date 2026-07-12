# Real-track manifests

Suite pointers and coverage inventory for the **primary** quality track.
See parent [`../README.md`](../README.md) and `docs/design-table-engine-v2.md` (Corpus Strategy).

## Files

| File | Purpose |
|------|---------|
| `real_detect_smoke_v0.json` | Detection smoke suite (v0): real PDFs + soft `expected_table_count` |
| `coverage_matrix.json` | Mode tag → detect/structure docs + explicit waivers |
| `demotion_list.json` | **C0 signed** list of synthetic bulk demoted from primary (PDFs not deleted) |
| `latency_probe.json` | Doc ids for p50/p95 timing (informative until rebaseline) |

Planned (later corpus PRs):

| File | Purpose |
|------|---------|
| `real_structure_core_vN.json` | T3 cell-grid structure core (5 → 15 → 25) |
| `real_fp_smoke_vN.json` | Forms / chrome / prose over-detect |

## Suite roles

| Suite id | Primary? | Gold | Runner |
|----------|----------|------|--------|
| `real_detect_smoke` | Yes (direction) | T0 count / T1 bbox IoU | `scripts/run_real_detect_smoke.py` |
| `real_structure` | Yes (G1 gate) | T3 cells; **stitch off** | accuracy harness (PR6c+) |
| `real_fp_smoke` | Yes (precision floors) | Expect 0 or low count | TBD |
| `geom_unit` | No (unit only) | Synthetic ≤20 | cargo + optional light Python |
| `legacy_synthetic_*` | No | Full grids | Multi-lib regression only |

## Policy reminders

1. **No ICDAR** files under `benchmark/corpus`, `real_track`, or CI ground truth (`assert_no_icdar.py`).
2. Demoted compete/hard synthetic PDFs stay on disk until C1 archive move; they are **not** primary README claims.
3. Structure eval must use **`stitch_multipage=false`** when CLI supports it (document product default separately).
4. New real PDF: add SOURCES.md entry + matrix mode tags before merge.

## How to run detect smoke

```bash
# dry-run (no binary required)
python3 benchmark/scripts/run_real_detect_smoke.py --dry-run
python3 benchmark/scripts/run_real_detect_smoke.py --help

# live: build release CLI then extract + write baseline artifact
cargo build --release -p pdfparser-cli
python3 benchmark/scripts/run_real_detect_smoke.py
# or: python3 benchmark/scripts/run_real_detect_smoke.py --build

# writes benchmark/real_track/results/real_detect_smoke_latest.json
# fields: per-doc n_pred, n_exp, ok; stitch_multipage=unknown_or_default
# default exit 0 on count DIFF; --strict → exit 1 on mismatch/fail
```

IoU unit tests (no PDF needed):

```bash
python3 benchmark/scripts/test_iou_metrics.py
```
