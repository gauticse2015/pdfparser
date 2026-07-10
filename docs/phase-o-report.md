# Phase O Implementation Report — Document Objects

**Date:** 2026-07-10  
**Status:** Complete — images, URI links, AcroForm fields, outline titles.

## Mandate

After text (Phase T) and tables (U/V), the remaining scoreboard gap was **objects_score = 0**.
Phase O ships a generic ISO 32000 object walk (no fixture hardcoding):

| Object | Source | Metric |
|--------|--------|--------|
| Images | Page `Resources` / `XObject` with `/Subtype /Image` (incl. nested Form XObjects) | count vs `expected_images` |
| Links | Page `Annots` `/Subtype /Link` + `/A /URI` | URI set F1 |
| Forms | Catalog `AcroForm` / `Fields` tree (`/T`, `/V`) | field name set F1 |
| Outline | Catalog `Outlines` linked list (`/Title`, `/First`, `/Next`) | title set F1 |

## API

```rust
let doc = Document::open("file.pdf")?;
let objs = doc.objects()?;
objs.image_count();
objs.link_uris();
objs.form_field_labels(); // name or name=value
objs.outline_titles;
```

CLI JSON (always included):

```json
{
  "image_count": 4,
  "images": [{"name": "...", "width": 320, "height": 200, "page": 0}],
  "links": ["https://..."],
  "form_fields": ["customer_name=...", "agree_terms=Yes"],
  "outline": ["Section 1 Special Objects", "Section 2 Continuation"]
}
```

## Gates

| Doc | Expectation | Result |
|-----|-------------|--------|
| `04_image_heavy` | 4 images | **4 / score 1.0** |
| `05_special_objects` | link + forms + outline | **all F1 1.0** |
| `10_mixed_document` | ≥1 image | **1 / score 1.0** |

## Scoreboard impact

| Metric | Before (V) | After (O) |
|--------|------------|-----------|
| objects mean | 0.0 | **100.0** |
| overall mean | ~90.9 | **~93.1** |
| vs pymupdf overall | 84.5 | **still lead** |

## Tests

```bash
cargo test -p pdfparser --test phase_o_objects --release
```

## Not in Phase O (honest)

| Item | Status |
|------|--------|
| Pixel decode / image export | Metadata only |
| Named destinations / non-URI links | URI Link only |
| Password decryption | Still rejected (K15) |
| Full structure tree / tagged PDF | Future |

*Phase O closed 2026-07-10*
