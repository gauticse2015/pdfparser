# TableOptions deprecation map (post Auto=EngineV2 / Phase 5)

## Product surface (≤12 top-level fields — GATE-5 G5.3)

| Field | Status | Notes |
|-------|--------|-------|
| `detect_tables` | **Active** | Master switch |
| `modes` | **Active** | Lattice / stream / hybrid bits |
| `min_table_confidence` | **Active** | Global lattice/hybrid floor |
| `max_tables_per_page` | **Active** | Page budget |
| `stitch_multipage` | **Active** | Default true; real_structure uses false |
| `enable_full_page_render` | **Active** | HighQuality / explicit |
| `allow_auto_render` | **Active** | K25 opportunistic; **Fast** sets false |
| `legacy_router` | **Active rollback** | Force soup NMS when true |
| `use_engine_v2` | **Active** | Auto/Full/EngineV2/HQ/Fast set true |
| `allow_classic_stream` | **Active** | Product Auto **false** (G5.4); LatticeStream true |
| `shadow_diagnostics` | **Active** | EngineV2 / HighQuality / dump-evidence |
| `advanced` | **Active nested** | All numeric detector knobs; serde-flattened |

## Nested advanced knobs

All lattice/stream/raster numeric fields live on `TableAdvancedOptions` and are
reachable via `Deref` (`opts.lattice_min_joints` still works). Prefer presets
over hand-tuning advanced knobs.

| Field / path | Status | Notes |
|--------------|--------|-------|
| Classic `detect_stream_tables` export | **Deprecated for product** | Network is borderless path |
| Legacy soup NMS path | Retained | Until M4 (≥1 minor after flip) |
| `advanced.tuning` / `opts.tuning` | **Active** | Document-type settings dict (`TableTuning`); system defaults + per-call override |

## TableTuning (settings dict)

Geometry / densify thresholds that often need document-class tuning live on
`TableTuning` (defaults match production mixed-PDF policy). Override at call
time without forking detectors:

```rust
let mut opts = TableOptions::from_preset(TablePreset::Auto);
opts.apply_tuning_overrides([
    ("densify_y_skip_numeric_frac", 0.10), // densify digit-light grids
    ("densify_y_explode_growth_hi", 3.0),  // allow larger Y growth
])?;
// CLI: --table-setting densify_y_skip_numeric_frac=0.10
```

Keys: `TABLE_TUNING_KEYS` / `TableTuning::keys()`. Examples:
`densify_y_*`, `densify_x_*`, `densify_pitch_cv_max`, `lattice_*_span_frac`,
`solid_lattice_stream_safe_*`.

## Presets

| Preset | Router | Render | Classic stream |
|--------|--------|--------|----------------|
| Auto / Full | Engine V2 | opportunistic (`allow_auto_render`) | off |
| EngineV2 | Engine V2 + diagnostics | opportunistic | off |
| HighQuality | Engine V2 + diagnostics | **explicit on** | off |
| Fast | Engine V2 | **never** | off |
| LatticeStream | legacy-friendly modes | default | **on** (experiment) |
| LatticeOnly | lattice only | default | off |
