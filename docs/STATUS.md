# Status (single source of truth)

**Updated:** 2026-08-08 (P0.1)  
**This file is the only place that may say PASS/FAIL for gates.**  
Other docs (README, CHANGELOG, AUTONOMOUS_PROGRESS, freeze notes, phase reports) must not invent a second status plane. Point here.

As-built hardening bible: [`design-zero-regression-hardening.md`](design-zero-regression-hardening.md) (A1.12 / A3.23 / A4.8 evidence columns are **pre-P0.1**; living board is this file).  
As-built pointer (not status): [`ARCHITECTURE.md`](ARCHITECTURE.md).

---

## Hard rules

| Rule | Meaning |
|------|---------|
| Do **not** claim GATE-3 / GATE-4 / GATE-5 green | Shape, cells, industry polish are **not** promoted |
| `g3_industry.json` is **INVALID** | Revoked (`revoked_at` set). Do not cite as PASS or as a freeze lock |
| Core cell freeze = `g2.json` | Auto micro cell F1 **0.637** (n=15). That is the regression lock |
| README **0.738** is not a freeze | Dated 2026-07-18 snapshot in `phase_ab_baseline.json` `real_core_cell_f1` (also `phase1_structure_framework.json`). **Not** `real_structure_latest.json` |
| ICDAR never in CI / corpus / tuning | External honesty check only. Never a PR merge bar |
| No `impl Deref` on `TableOptions` | Advanced knobs live on `opts.advanced` (serde flatten only) |
| Fast preset never full-page-renders | `allow_auto_render=false` |

Committed `*_latest.json` is **not** a live `--binary` re-run. Detector PRs must not claim PASS from committed JSON alone (nightly live after P0.6).

---

## Gate board

Owned detection is the strength. Quality (shape / cells / TEDS) is not.

| Gate | Status | Evidence (owned) | Must not claim |
|------|:------:|------------------|----------------|
| **GATE-0** measurement foundation | **PASS** | Discipline manifest n=34; fp_strict n=12; T3 gold files **25**; count golds >=25; `baseline_pre_v3.json`; `assert_no_icdar.py` | - |
| **GATE-1** over-detect / precision | **PASS** (owned detection) | `detect_discipline_latest.json` exact **0.941**, over_doc **0.029**, pred/gt **1.0**, severe over **0**; `real_fp_strict_latest.json` zero_rate **1.000** (n=12); g2 core det F1 **0.964** | ICDAR F1 as a CI gate |
| **GATE-2** completeness | **PASS** (owned detection) | Discipline under_doc **0.029**; nested doc **42** keep is a product invariant (outer + inner) | ICDAR under-detect as a CI gate |
| **GATE-3** shape / topology | **NOT green** | Core freeze shape exact **0.533** (`g2.json`); latest committed full-suite shape **0.565** (`real_structure_latest.json`) still below honest G3 floors | GATE-3 PASS |
| **GATE-4** cells / TEDS | **NOT green** | Freeze core cell **0.637**; census / NIPA cell F1 still near-zero on live structure; ICDAR TEDS ~0.46 | GATE-4 PASS; g3 cell **0.787** |
| **GATE-5** industry / production polish | **NOT green** | `g3_industry.json` phase4/5 **INVALID**; freeze README must not cite it | GATE-5 PASS; production-ready |

CI may keep phase 3-5 jobs `continue-on-error` until honestly green. That is **not** PASS.

`--phase 1` / `--phase 2` on `check_phase_gates.py` still load ICDAR JSON today. That is a P0.6 harness bug, **not** policy. Merge bar for this PR: `--phase 0` only. Do not use `--phase 1/2` as a merge bar.

---

## Freeze lock vs live numbers

| Artifact | Role | Core cell (Auto) | Notes |
|----------|------|-----------------:|-------|
| [`g2.json`](../benchmark/real_track/freezes/g2.json) | **Steady freeze / regression lock** | **0.637** | Active. Shape exact **0.533**; det count F1 **0.964** |
| [`phase_ab_baseline.json`](../benchmark/real_track/results/phase_ab_baseline.json) | Dated 2026-07-18 snapshot | **0.738** | Field `real_core_cell_f1`. Same value in `phase1_structure_framework.json`. **Not** latest. **Not** a freeze |
| [`real_structure_latest.json`](../benchmark/real_track/results/real_structure_latest.json) | Latest committed run (2026-07-19) | core mean **0.770** / full **0.656** | Core = equal-weight mean of 15 g2 ids; full = summary n=23 (cell 0.656, shape 0.565, det 0.935, IoU 0.844). **Not** a freeze |
| AUTONOMOUS_PROGRESS "core 0.820" | Retracted claim | - | Ignore |
| [`g3_industry.json`](../benchmark/real_track/freezes/g3_industry.json) | Revoked industry freeze | claimed 0.787 | **INVALID** |

No-regress floor (G1.7): live core cell >= **g2 auto - 0.03** (0.607). Do not invent a `live-main - 0.02` floor.

### T3 gold count (A4.9)

Pick both numbers; they measure different things:

| Count | What |
|------:|------|
| **15** | g2 core ids (`documents_auto`) - freeze comparison set |
| **23** | Latest `real_structure` scored suite (README full-suite board) |
| **25** | `benchmark/real_track/gold/*.json` files on disk (includes tracking 70/71) |

Do not claim G5.7 "T3 >= 25" as GATE-5 PASS. File count != promoted industry freeze.

---

## ICDAR (external honesty, not CI)

| Board | Peers | How to read |
|-------|------:|-------------|
| README multi-peer table (2026-07-19) | 5 | pdfparser detection F1 **#2** (0.825 vs Camelot auto 0.864). **Not #1** |
| `camelot_icdar_headtohead.json` / `docs/icdar-competitive-report.md` latest dump | **1** | pdfparser-only. **peers=1**. Solo board is not rank #1 |

Never copy ICDAR PDFs or `*-str.xml` into this repo. `assert_no_icdar.py` enforces that.

---

## Product Auto (as-built)

- `TablePreset::Auto` / `Full` = Engine V2 (`use_engine_v2=true`) then `finalize_engine_v2`.
- Rollback: `TableOptions.legacy_router = true` / CLI `--legacy-router` only.
- `PDFPARSER_TABLE_LEGACY` and `--tables-legacy` are **unimplemented** (do not document as working).
- `TableOptions::default()` stays tables-off / legacy. Product path is `from_preset(Auto)`.
- `impl Deref` for advanced knobs is **gone**.

Maturity labels (customer): text + ruled/borderless **detection** production; cell/TEDS **Beta**. See README.

---

## What is still open (not this PR)

Harness / gold / executable contract (P0.2-P0.6): taxonomy not-all-unknown, R005/R010 gold review, G5.4 unit-test SSOT, latency freeze file, `--owned-only` + `--binary`. Do not invent `--binary` until P0.6.
