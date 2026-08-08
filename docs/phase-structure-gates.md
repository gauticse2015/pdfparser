# Structure quality phases (TEDS / row / col) — success criteria

**Gate PASS/FAIL SSOT:** [`STATUS.md`](STATUS.md). This file defines criteria; do not treat the status log below as current promotion. GATE-3/4/5 are **not** green.

Autonomous develop→assess loop. **No ICDAR doc-id coupling in engine code.** ICDAR is never CI.

## Shared guards (every phase)

| Guard | Threshold |
|-------|-----------|
| Real g2 core cell F1 | ≥ freeze − 0.02 (no-regress) |
| Real g2 det count F1 | ≥ 0.88 |
| ICDAR F1 | ≥ 0.815 (honest floor) |
| Unit tests `pdfparser-tables` | pass |
| Stream fixtures `07` / `59` | pass |
| No gold pads / suite filename hacks | required |

## Phase 1 — Framework + taxonomy (this phase)

| Criterion | Pass condition |
|-----------|----------------|
| P1.1 `TableProfile` + `from_profile` | unit tests green |
| P1.2 CLI `--table-profile` | help + parse works |
| P1.3 Wire `densify_y_small_growth_max` + overdense factor from tuning | code + units |
| P1.4 Taxonomy harness | writes `structure_error_taxonomy_latest.json` |
| P1.5 No-regress | shared guards pass on **default** profile |

Phase 1 does **not** require row/col/TEDS lifts.

## Phase 2 — Row geometry (default-safe)

| Criterion | Pass condition |
|-----------|----------------|
| P2.1 ICDAR row | ≥ **0.50** OR +0.02 vs phase-1 baseline, no F1 drop |
| P2.2 Real core shape zeros | ≤ 5 (hold) or improved |
| P2.3 Shared guards | pass |

## Phase 3 — Col geometry

| Criterion | Pass condition |
|-----------|----------------|
| P3.1 ICDAR col | ≥ **0.55** OR +0.02 vs phase-2 baseline |
| P3.2 Shared guards | pass |

## Phase 4 — TEDS / content

| Criterion | Pass condition |
|-----------|----------------|
| P4.1 ICDAR TEDS | ≥ **0.50** OR +0.02 vs phase-3 |
| P4.2 Shared guards | pass |

## Profiles (eval / customer; not CI default)

- `sparse_ruled`, `prose_grid`, `sparse_v_numeric`, `multi_level_header`
- Report-only ICDAR with profiles; default path stays Auto + Default tuning.


## Status log

| Phase | Status | Notes |
|------:|:------:|-------|
| 1 Framework + taxonomy + profiles | **PASS** | TableProfile, CLI, wired growth/overdense keys, taxonomy harness; F1 0.832 core 0.738 |
| 2 Row geometry | **PASS** | row **0.500–0.507**, F1 ≥0.815, core ok, stream 07/59 green. Stream header keep + footnote strip. |
| 3 Col geometry | **FAIL** | col **0.535** (need ≥0.55). Shippable path after fixture fix; under-col densify_x/partial-V next. |
| 4 TEDS/content | **BLOCKED** | TEDS **~0.46** (need ≥0.50). Depends on col+lattice content quality. |

### Phase 2 landed (generic, no ICDAR ids)
1. Network: near-top single-run multi-word headers; multi-run headers with aligned≥1; trailing prose footnote strip.
2. TableTuning multi-band densify growth keys + profiles (defaults no-op-safe for densify force).
3. Rejected: aggressive moderate densify; orphan header attach (broke 07/59).

### Phase 3–4 next (human direction useful)
1. Partial-V / exterior densify_x for 2→3 col under-count (us-036 class) without over-col.
2. Lattice cell content assignment (us-004 exact shape, TEDS 0.24).
3. Multi-page under-row (us-018/024) without detection F1 loss.
