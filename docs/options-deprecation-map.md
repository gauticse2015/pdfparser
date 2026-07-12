# TableOptions deprecation map (post Auto=EngineV2)

| Field | Status | Notes |
|-------|--------|-------|
| `use_engine_v2` | **Active** | Auto/Full set true |
| `legacy_router` | **Active rollback** | Force soup NMS when true |
| `enable_full_page_render` | Active | HighQuality / explicit |
| `allow_auto_render` | Active | K25 opportunistic gate |
| `shadow_diagnostics` | Active | EngineV2 preset / dump-evidence |
| Legacy soup NMS path | Retained | Until M4 (≥1 minor after flip) |

No field removals in this release.
