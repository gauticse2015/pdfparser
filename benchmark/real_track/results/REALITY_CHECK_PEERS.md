# Reality check — G1 real_structure vs peers

Generated: `2026-07-12T11:28:45.266099+00:00`

Shared **human/vision T3 gold**. Same metrics as `run_real_structure` (grids).

## Mean scores (equal-weight per doc)

| library | mean cell F1 | mean det F1 | mean shape | mean score | wins (best cell F1) | n_err |
|---------|-------------:|------------:|-----------:|-----------:|--------------------:|------:|
| **pdfparser_auto** | 0.6372 | 0.9644 | 0.5333 | 72.5782 | 1 | 0 |
| **pdfparser_engine_v2** | 0.6372 | 0.9644 | 0.5333 | 72.5782 | 0 | 0 |
| **camelot_lattice** | 0.5781 | 0.8444 | 0.5333 | 66.0114 | 8 | 0 |
| **camelot_stream** | 0.3465 | 0.9222 | 0.2000 | 51.1384 | 3 | 0 |
| **pdfplumber** | 0.5832 | 0.8121 | 0.5667 | 65.9184 | 3 | 0 |

## Per-doc cell F1

| id | S | pdfparser_auto | pdfparser_engine_v2 | camelot_lattice | camelot_stream | pdfplumber | best |
|----|---|----------:|----------:|----------:|----------:|----------:|------|
| `30_real_ca_warn_report` | False | 0.839 | 0.839 | 1.000 | 0.053 | 0.839 | camelot_lattice |
| `33_real_argentina_votes` | False | 0.597 | 0.597 | 1.000 | 0.593 | 1.000 | camelot_lattice |
| `34_real_schools_contributions` | True | 0.460 | 0.460 | 0.944 | 0.341 | 0.991 | pdfplumber |
| `35_real_camelot_fuel` | False | 0.716 | 0.716 | 1.000 | 0.000 | 1.000 | camelot_lattice |
| `36_real_two_tables` | True | 0.421 | 0.421 | 1.000 | 0.416 | 0.629 | camelot_lattice |
| `37_real_liabilities_superscript` | False | 0.247 | 0.247 | 0.000 | 1.000 | 0.000 | camelot_stream |
| `31_real_background_checks` | True | 0.821 | 0.821 | 0.012 | 0.258 | 0.023 | pdfparser_auto |
| `32_real_census_table324` | True | 0.000 | 0.000 | 0.037 | 0.004 | 0.004 | camelot_lattice |
| `R018_camelot_health` | False | 0.995 | 0.995 | 1.000 | 0.040 | 0.127 | camelot_lattice |
| `R003_bea_gdp_sample` | True | 0.868 | 0.868 | 0.090 | 0.030 | 0.994 | pdfplumber |
| `R010_bea_nipa_highlights` | True | 0.000 | 0.000 | 0.036 | 1.000 | 0.166 | camelot_stream |
| `42_real_insurance_italian` | True | 0.950 | 0.950 | 0.552 | 0.000 | 0.976 | pdfplumber |
| `43_real_row_span_gmc` | True | 0.995 | 0.995 | 1.000 | 0.013 | 1.000 | camelot_lattice |
| `44_real_spanning_cells` | True | 0.706 | 0.706 | 1.000 | 0.451 | 1.000 | camelot_lattice |
| `45_real_campaign_donors` | True | 0.943 | 0.943 | 0.000 | 1.000 | 0.000 | camelot_stream |

JSON: `benchmark/real_track/results/reality_check_peers_g1.json`
