# Table extraction (production)

**Default:** `TablePreset::Auto` (== `Full`)

```
rules + text
    → lattice (vector + thin-fill painted rules)
    → hybrid only outside strong lattice
    → network borderless only outside strong lattice
    → form scrub + NMS
```

No expert registry / page-class framework. Exclusive under strong lattice is one flag:
`exclusive_under_strong_lattice`.

## Capabilities

| Piece | Role |
|-------|------|
| Lattice + thin-fill | Ruled tables (vector sensing) |
| Overdense-H collapse | False underlines |
| densify-Y | Sparse intermediate H |
| Network | Textline + column alignments (borderless) |
| Hybrid | Partial borders |

## Next hard work (not architecture)

1. Stronger network (center/right alignments, better multi-table split)
2. Real raster ROI when vector rules empty but text is gridded (needs render)
