# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Content -- cap q/Q graphics stack nesting (P2.2b) (2026-08-08)

- Content VM stops pushing `q` once `gstack` reaches `InterpretOptions.max_nesting_depth` (default 64; same as `ResourceLimits` / `hard_max::MAX_NESTING_DEPTH`).
- Fail-soft: warn once (`gstack_nesting_depth exceeded` → `WarningCode::LimitSoft`); extra `q` ignored; empty `Q` still no-ops. Does not panic.

### Content -- typed VmWarning (P2.1a) (2026-08-08)

- `InterpretResult.warnings` is `Vec<VmWarning>` (was `Vec<String>`).
- `Display` keeps legacy strings (`unknown_op:...`, `stack_underflow:numeric`, form/budget messages).
- Extract still maps every VM warning to `WarningCode::UnknownOperator` except q/Q nest (`LimitSoft`).
- Ignored-op skip set unchanged; `pop_num` still fail-soft `0.0`.

### Fonts -- ToUnicode decode shares document governor (P2.2a) (2026-08-08)

- `load_page_fonts` takes `&ResourceGovernor`; ToUnicode stream decode charges the document budget (A2.23).
- `LimitExceeded` does not fall back to ungoverned inflate.

### Tables -- DensifyMode default Primary (P1.8) (2026-08-08)

- Added `DensifyMode::{Primary, InsideFrameOnly, Off}` on `TableAdvancedOptions.densify_mode` (H20; not a 13th top-level field).
- Default **Primary** = today's densify-as-primary. Legacy `lattice_text_densify=false` maps to `Off`.
- **No densify math change.** `InsideFrameOnly` is a P5.2 placeholder and currently runs the same helpers.

### CLI -- optional text IR via pdfparser-export (P1.7) (2026-08-08)

- `pdfparser extract --format json --use-export-ir` serializes `ExtractedDocument` through `pdfparser-export`.
- Default off. Table `--format json` schema is unchanged; `--use-export-ir` is ignored when tables are enabled.

### Tables -- borrowed PageEvidence on diagnostics (P1.2) (2026-08-08)

- `PageEvidence<'a>` borrows runs/rules/rasters; `PageEvidenceOwned` dump type omits raster pixels.
- Diagnostics wrapper (`detect_tables_page_with_diagnostics`) no longer clones inputs.
- Dump clone only via `page_evidence_owned_if_dump` when `shadow_diagnostics && dump`.
- Product Auto/Fast detect still does **not** construct `PageEvidence` / `LineEvidence`.

### Bench -- executable owned contract (P0.6) (2026-08-08)

- `check_phase_gates.py --owned-only` is the ICDAR-free CI merge bar (`freezes/owned_gates_v0.json`).
- Runners accept `--binary PATH` (`run_detect_discipline`, `run_fp_strict`, `run_real_structure`, `run_latency_probe`).
- Dump-compare: `dump_product_tables.py` + `compare_table_dumps.py` (`pdfparser_table_dump_v1`).
- CI `real-track-gates` no longer runs `--phase 1` / `--phase 2` (those load ICDAR metrics).

### CI -- latency_fast_v0 freeze and nightly probe (P0.5) (2026-08-08)

- Recorded Fast p50/p95/max + hardware in `benchmark/real_track/freezes/latency_fast_v0.json` (copied from committed `latency_probe_latest.json`; local macOS, not ubuntu-latest).
- `budget_p95_ms = max(recorded_p95 * 1.5, recorded_max * 1.2)` (~725 ms). Two independent fail rules, no `min()`. Do not gate vs freeze `recorded_p95 * 1.10`.
- Required nightly workflow `.github/workflows/nightly-latency-fast.yml` runs Fast probe and compares to the freeze. Fast never full-page-renders. ICDAR not in that workflow.
- Probe script `budget_p95_ms=30000` stays informational. **Do not claim 30s->0.6s tightening** until an ubuntu-latest sample exists. **Not GATE-5.**

### Gold -- R005 / R010 PDF review (P0.3) (2026-08-08)

- **R005** structure expected_table_count **1 → 0**. Opened ACS PDF: first page is the title page, not a table. Dropped the 5×1 Camelot stub. Discipline stays n_exp=0 first_page.
- **R010** counts **unchanged** (structure 51×22 / n_exp=1; discipline n_exp=1 on gold page 7). Live 2 vs 1 is lattice+stream over-detect.
- `validate_structure_gold.py` allows empty `expected_tables` when `expected_table_count == 0`.

### Docs -- single status plane (P0.1) (2026-08-08)

- **`docs/STATUS.md`** is the only source of truth for gate PASS/FAIL.
- **Do not claim GATE-3 / GATE-4 / GATE-5 green.** Owned detection (G1/G2) is strong; shape / cells / industry polish are not.
- Freeze lock remains `benchmark/real_track/freezes/g2.json` (core cell Auto **0.637**). **0.738** is `phase_ab_baseline.json` (2026-07-18), not `real_structure_latest.json` and not a freeze.
- `g3_industry.json` is **INVALID** / revoked - never cite as PASS.
- Landed `docs/design-zero-regression-hardening.md` + `docs/ARCHITECTURE.md` stub.
- `impl Deref` on `TableOptions` -> `TableAdvancedOptions` is **already gone**; use `opts.advanced`.

### Phase 4-5 -- cells + industry polish (2026-07-18) -- GATE-4/5 claims retracted

- Work landed: densify_x right-edge + NIPA glue; `TablePreset::Fast` (never full-page render); classic stream off on product Auto (`allow_classic_stream`); HQ skip full-page render when vector lattice already rich; CI `real-track-gates` job; harness `run_latency_probe.py` / `run_hq_vs_auto.py`; T3 gold files on disk = 25.
- **API:** `TableOptions` product surface ≤12 fields; knobs on `TableAdvancedOptions` (serde flatten). **Deref already removed.**
- **Not green:** prior Unreleased text claimed GATE-4/5 PASS and freeze `g3_industry.json`. Those claims were false. `g3_industry.json` phase4/5 are INVALID. Core freeze stays `g2.json`.
- **Docs:** gate labels live in `docs/STATUS.md` only (not `AUTONOMOUS_PROGRESS.md`).

### Phase 2 — detection completeness / borderless architecture (2026-07-16)
- **Dense multi-col stream exemption** in form disc + borderless recall: campaign donors (56×7)
  and liabilities (~30×10) no longer hard-dropped as “giant IRS worksheets”.
- **`form_likeness` size penalty** only when fill is sparse (dense data lists ≠ forms).
- **IRS keyword veto** still rejects Schedule C / OMB field grids (FP strict zero_rate **1.0**).
- **GATE-1 + GATE-2** (historical 2026-07-16 snapshot; numbers != `docs/STATUS.md`):
  discipline exact **0.971**, under **0.029**, multi exact **1.0**.
  ICDAR F1 **0.761** in that snapshot is external honesty only (not a CI gate).
- **T3 structure golds expanded to n=23** (`R021` left two-col multi, `R022` superscripts)
  with `phase3_tracking` metadata for Phase-3 shape/cell work (core freeze still n=15).

### Production readiness — table engine review (2026-07-12)

- **Identity emit** after partition: tables kept by proposal `source_indices` (not loose center-in-bbox); K26 merge → one best table
- **Real structure signals:** `Table.joint_count`, `ProposalOrigin` (no `whitespace_est` sentinel hacks)
- **Page media-box** into `area_frac` from extract path
- **Nested gates:** min area ratio + min child area/side (reject tiny corners; keep real nested grids)
- **Contour seeds** diagnostic-only (no hard-own; fixes flaky stream drops under opportunistic render)
- **Empty-column collapse:** preserve colspans; skip pure image lattices (all cells text-empty)
- **`ProposalPolicy::from_options`**, `lattice_text_densify`, densify helpers in `builders/densify.rs`
- Rename **`legacy` → `orchestrator`** (compat alias); crate docs + CONTRIBUTING match Auto = Engine V2

### Phase 17–19 closeout (2026-07-12)
- Steady freeze `freezes/g2.json`; product Auto = Engine V2 confirmed
- K25 opportunistic full-page render probe; CLI `--dump-evidence`
- Lattice: collapse fully empty interior columns (R003 cell F1 large gain)
- Network glued: `-N` as missing marker + unsigned (37 improved)
- README real_structure primary board; H3 closed; phases 17–19 status PASS
- real_structure micro cell F1 **~0.64**, det **~0.96**, score **~73**


### Engine V2 product flip + nested multi-table (2026-07-12)
- **Nested lattices:** `is_nested_table_pair` — exclusive partition / ownership / K26 merge
  and legacy NMS keep outer form + inner rate table (Italian insurance 42 → **2 tables**, cell **0.95**).
- **Engine V2 finalize:** post-partition exclusive cleanup (stream under ruled, weak 2-col prose FP).
- **Product flip:** `TablePreset::Auto` / `Full` now use `use_engine_v2=true` + exclusive router
  (same quality as EngineV2 on G1: cell F1 **0.588**, det **0.964**). Rollback: `legacy_router=true`.
- real_structure Auto ≡ EngineV2 on all 15 docs after nested fix.

### Geometry + stream quality (2026-07-12 cont.)
- **Critical:** text rendering matrix is `Tm × CTM` (ISO 32000 §9.4.4), not `CTM × Tm`.
  Fixes Quartz/Tabula coordinates (campaign_donors, many real fixtures) — positions and
  glyph advances were wrong when `cm` scaled user space.
- **IR:** `TextRun.font_size` is user-space (`Trm.linear_scale() × Tf`); matrix helpers
  `scale_x` / `scale_y` / `linear_scale` + unit tests for Quartz `cm`/`Tm`.
- **Orchestrator:** prefer dense multi-col stream/network over sparse over-wide hybrid
  (drops 56×27 hybrid when 56×7 network covers same region).
- **Network:** glued label+numeric single-run body rows (RBI/Excel stream) — area multi,
  rich-line column anchors, financial-tail tokenization (≤2 decimals with `4.42.62`→`4.4`+`2.62`).
- **Lattice:** expand X anchors for exterior multi-row stub/line-number columns
  (BEA GDP-style tables left of ruled number grid); softer densify under-rule gate.
- **real_structure (n=15 Auto):** micro cell F1 **~0.49 → ~0.57**, det F1 **~0.78 → ~0.94**,
  score **~58 → ~68**. **45 donors 0.005→0.94** shape-exact 56×7; **R003 0.09→0.24**
  (32×12 with Line+stub recovery); 37 liabilities 0→0.13 (30×10 stream).

### Table engine quality (2026-07-12)
- Lattice: fix false overdense-H collapse; long-H recovery when under-dense; text densify gates
- Cell assign: split multi-token tabular runs; redistribute full-row dumps into columns
- Content: thin-fill multi-subpath `f` (Word/Excel grid rules) → lattice on painted rects
- real_structure micro cell F1 ~0.32 → ~0.47 (R018/43/31/44 strong wins); synthetic still leads peers



### Table engine / real_structure (2026-07-12)
- G1 corpus ready: n=15 reviewed T3 golds, struggle 10/15; nesting metadata on 42
- Phase 16: boot freeze candidate `freezes/g2_candidate.json`; Auto remains legacy until H3
- Rollback tests for `legacy_router` force path



### Added

- **Phase 15:** multi-table demote fix — **32 census detects 2 tables**; prefer lattice over hybrid

- **Phase 14 multi-table:** wide_ruled form path + gutter strip guard; **36 detects 2 tables**

- **Phase 13:** structure eval page-local filter; strong lattice needs ≥3 cols; network y_tol loosen; real_structure n=8 (31/32 vision+stream struggle golds human-confirmed)
- **Peer structure gold pipeline** (Camelot/pdfplumber/optional vision); self-golds quarantined; real_structure n=0 until human confirm

- **Phase 9–12 (autonomous chain α):**
  - `run_real_structure.py` cell/shape/det harness (stitch off, multi-preset)
  - CLI `--table-preset`, `--no-stitch`, `--page-tables`; page-range filters root tables
  - Shadow baseline Auto vs EngineV2; detect_smoke ≥20; `real_fp_smoke` first-page suite
  - Lattice column-slice demotion (fixes census over-detect 2→1)
  - H1 human gold review package at `benchmark/real_track/results/H1_REVIEW_PACKAGE.md`
- **PR3 render spike (external CLI):** `ExternalCliPageRenderer` (pdftoppm / mutool / gs), fail-soft; `Document.source_path` + page/document tables wire when `enable_full_page_render`; CLI `--tables-hq`
- **real_structure progressive T3 (n=5):** soft-gold reals `31/32/33/35/36` with reviewed cell grids; struggle tags on 32/36; `g1_ready=false` (G1 still locked)
- **Table engine V2 foundations** (`docs/design-table-engine-v2.md` rev 2.3): evidence layer, legacy orchestrator extract, `TablePreset::EngineV2` / `HighQuality` opt-in (still legacy builders until Gate G1)
- Shadow diagnostics: `detect_tables_page_with_diagnostics`, `PageEvidence` / `LineEvidence`
- **PR2a FormContentResolver:** expand Form XObject content for rules/text on table path (`form_resolve.rs`, `interpret_page_with_resolver`)
- **PR2b dash rules:** graphics state `d` expands ON segments into lattice `RuleSegment`s
- **PR4a:** `builders/ruled` extract; `detect_lattice_tables` thin re-export (parity)
- **PR2c clip:** axis-aligned clip stack; rules clipped to clip rect
- **EngineV2 contour proposals:** raster contour seeds enter router partition
- **Structure drafts:** `export_structure_draft.py` + geom practice T3 (`geom_unit_structure`, not G1)
- **Phase verify:** requires min cargo test counts; Phase 6 structure track suite (23+ cases)
- **PR4c wire:** ruled detect uses `rules_from_raster_combined` (K28); K29 text-baseline H suppress after raster
- **K34:** hybrid free page-union fallback removed
- **PR5c:** multi-run exclusive cell merge unit test
- **PR3 scaffold:** `PageRenderer` / `NullPageRenderer` / `RenderSafety` (no native backend yet)
- **Structure track:** `real_structure_v0.json` progressive T3 (n=5 soft-gold seed) + `validate_structure_gold.py` — G1 still locked
- Phase verify fails on cargo warnings; fixed form/hybrid unused_mut
- **PR4c (K28):** stamp vector rules into raster ink; `rules_from_raster_combined` / contour seeds (helpers + tests)
- **PR5b:** EngineV2-only AutoRouter finalize (`engine_v2_router` notes); product Auto remains legacy
- **Live real smoke:** `run_real_detect_smoke.py` extract + `real_track/results/real_detect_smoke_latest.json`
- Fixed unused_mut test warnings in form/hybrid; phase verify fails on any cargo warning
- **PR5 v1:** network table-area hard-gap split; no mega-table fallback; area unit tests
- **Router skeleton (K26/K27):** `pdfparser-tables::router` vertical merge + partition + emit order (not wired into Auto yet)
- Real-PDF track: `benchmark/real_track/` (coverage matrix, demotion list, SOURCES, smoke runner)
- IoU metrics tests + `run_real_detect_smoke.py`; `assert_no_icdar.py` + CI job
- **Network borderless tables** (textline + column alignments, gap split, same-schema re-merge, prose/list FP reject)
- **Thin-fill rule sensing** improvements (painted rect rules → lattice)
- **False-underline / overdense-H collapse** via text bands
- **Compete / compete_hard / sensing** owned regression suites + generators
- **Public real PDF fixtures** under `benchmark/corpus/compete_real/` (soft gold)
- Integrity check `benchmark/scripts/check_compete_suite.py`
- Production table pipeline docs and expanded README multi-lib comparison tables

### Changed

- Product default table path: `TablePreset::Auto` / `Full` — lattice → hybrid residual → network residual; strong lattice excludes overlapping borderless
- Removed probe/expert-registry ceremony; single production orchestration flag `exclusive_under_strong_lattice`
- Hard struggle suite (C100–C180) baseline cell F1 ~0.38 → ~0.45 after production path (still open struggle)

### Honest competitive note

- Owned multi-lib scoreboard: pdfparser leads cell F1 / overall on synthetic+hard suites
- ICDAR-2013 external: **not CI**. Multi-peer board (2026-07-19) has pdfparser **#2** on detection F1 (0.825 vs Camelot auto 0.864). Latest `camelot_icdar_headtohead.json` dump is **peers=1** (pdfparser only) — solo board ≠ rank #1.
- Product Auto is **Engine V2** (as-built). Rollback: `legacy_router=true`.


## [0.1.0] - 2026-07-10

### Added

- **Phase 15:** multi-table demote fix — **32 census detects 2 tables**; prefer lattice over hybrid

- **Phase 14 multi-table:** wide_ruled form path + gutter strip guard; **36 detects 2 tables**

- **Phase 13:** structure eval page-local filter; strong lattice needs ≥3 cols; network y_tol loosen; real_structure n=8 (31/32 vision+stream struggle golds human-confirmed)
- **Peer structure gold pipeline** (Camelot/pdfplumber/optional vision); self-golds quarantined; real_structure n=0 until human confirm

- **Phase 9–12 (autonomous chain α):**
  - `run_real_structure.py` cell/shape/det harness (stitch off, multi-preset)
  - CLI `--table-preset`, `--no-stitch`, `--page-tables`; page-range filters root tables
  - Shadow baseline Auto vs EngineV2; detect_smoke ≥20; `real_fp_smoke` first-page suite
  - Lattice column-slice demotion (fixes census over-detect 2→1)
  - H1 human gold review package at `benchmark/real_track/results/H1_REVIEW_PACKAGE.md`
- Initial workspace and library façade (`pdfparser`)
- Core open path: page tree, stream filters, resource governor
- Font encodings: WinAnsi/MacRoman/Standard, `/Differences`, ToUnicode (bfchar/bfrange)
- Content-stream text VM (`Tj` / `TJ` / text state)
- Layout: page `/Rotate` normalize, space insertion, multi-column reading order
- CLI: `pdfparser extract` (text/JSON), `pdfparser info`
- Competitive benchmark harness and accuracy scoreboard
- Design documentation under `docs/`
- Phase T integration tests on corpus fixtures

### Security

- Encrypted PDFs rejected in v0.1 (`Error::Encryption`)
- Stream expansion charged against resource limits

[Unreleased]: https://github.com/gauticse2015/pdfparser/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/gauticse2015/pdfparser/releases/tag/v0.1.0
