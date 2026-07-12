# Gold table schema v1 (real track)

```json
{
  "id": "doc_id",
  "schema_version": 1,
  "track": "real_detect_smoke | real_structure",
  "license": "short note + URL",
  "source_url": "https://...",
  "failure_classes": ["partial_rules", "multi_table"],
  "expected_table_count": 2,
  "expected_table_count_tol": 0,
  "expected_tables": [
    {
      "page": 0,
      "bbox": { "x0": 0, "y0": 0, "x1": 100, "y1": 200 },
      "rows": 5,
      "cols": 3,
      "cells": [["H1", "H2", "H3"], ["a", "b", "c"]]
    }
  ]
}
```

- `bbox` required for IoU detection metrics; omit only for detect-count soft gold.
- `cells` required for structure core; never auto-accepted from extractor output without human edit.
- Do not put ICDAR gold XML here.

## Nested tables (optional)

When a table is drawn **inside a cell** of another table (e.g. `42_real_insurance_italian`):

**Preferred for current pdfparser:** emit the nested grid as a **separate** `expected_tables[]` entry and record the parent link:

```json
{
  "page": 0,
  "rows": 4,
  "cols": 2,
  "cells": [["…"], ["…"]],
  "role": "nested_table",
  "nested_in": {
    "parent_table_index": 0,
    "parent_row": 4,
    "parent_row_1based": 5,
    "parent_col": 1,
    "parent_label": "CASO MORTE",
    "relationship": "drawn_inside_cell"
  }
}
```

On the parent table, optional:

```json
"cell_annotations": [
  {
    "row": 4,
    "row_1based": 5,
    "col": 1,
    "kind": "contains_nested_table",
    "nested_table_index": 1
  }
]
```

- **Parser requirement today:** detecting both grids as separate tables is enough for structure scoring.
- **True nested cell JSON** (`cells[r][c] = { text, nested_table: {...} }`) is optional future schema — not required until nested extraction ships.
- Do **not** flatten the nested grid into the parent cell string as if it were prose (loses structure).
