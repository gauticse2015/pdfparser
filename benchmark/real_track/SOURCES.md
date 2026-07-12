# Real-track PDF sources (license + URL)

**Policy:** Every real PDF used for `real_detect_smoke` / `real_structure` / `real_fp_smoke`
must be listed here with license class, source URL, and retrieved note.
Never add ICDAR-2013 competition PDFs (`eu-###` / `us-###`).

**Template for new docs** (copy a block and fill):

```markdown
## <doc_id>
- **PDF path:** `corpus/real/<file>.pdf`
- **Source URL:** https://...
- **License:** US government work | public sample | arXiv + paper license | other (specify)
- **Retrieved:** YYYY-MM-DD
- **Use:** detect_smoke | structure_core | fp_smoke
- **Notes:** short provenance / redistribution caution
```

---

## 30_real_ca_warn_report
- **PDF path:** `corpus/real/30_real_ca_warn_report.pdf`
- **Source URL:** https://github.com/jsvine/pdfplumber/blob/stable/examples/pdfs/ca-warn-report.pdf
- **License:** pdfplumber project public demo / example data
- **Retrieved:** (pre-existing corpus; re-verify before redistribution claims)
- **Use:** detect_smoke
- **Notes:** CA WARN multi-page layoff notices — wide tables, many rows

## 31_real_background_checks
- **PDF path:** `corpus/real/31_real_background_checks.pdf`
- **Source URL:** https://github.com/jsvine/pdfplumber/blob/stable/examples/pdfs/background-checks.pdf
- **License:** pdfplumber examples
- **Retrieved:** (pre-existing corpus)
- **Use:** detect_smoke | structure_core
- **Notes:** NICS-style wide header table; T3 gold in `real_track/gold/31_real_background_checks.json`

## 32_real_census_table324
- **PDF path:** `corpus/real/32_real_census_table324.pdf`
- **Source URL:** https://github.com/tabulapdf/tabula-java/raw/master/src/test/resources/technology/tabula/12s0324.pdf
- **License:** US government work / Tabula test fixture
- **Retrieved:** (pre-existing corpus)
- **Use:** detect_smoke | structure_core (struggle)
- **Notes:** Statistical Abstract lattice sample (not ICDAR); partial T3 + over_detect struggle tags

## 33_real_argentina_votes
- **PDF path:** `corpus/real/33_real_argentina_votes.pdf`
- **Source URL:** https://github.com/tabulapdf/tabula-java/raw/master/src/test/resources/technology/tabula/argentina_diputados_voting_record.pdf
- **License:** Tabula test fixture (public sample)
- **Retrieved:** (pre-existing corpus)
- **Use:** detect_smoke | structure_core
- **Notes:** Irregular multi-section voting grid; Spanish; T3 gold reviewed

## 34_real_schools_contributions
- **PDF path:** `corpus/real/34_real_schools_contributions.pdf`
- **Source URL:** tabula-java test resources (`schools.pdf` family)
- **License:** Tabula test fixture
- **Retrieved:** (pre-existing corpus)
- **Use:** detect_smoke
- **Notes:** Multi-page stream-like contribution rows

## 35_real_camelot_fuel
- **PDF path:** `corpus/real/35_real_camelot_fuel.pdf`
- **Source URL:** Camelot project sample (`docs/_static/pdf/foo.pdf` / fuel research pages)
- **License:** Camelot project sample
- **Retrieved:** (pre-existing corpus)
- **Use:** detect_smoke | structure_core
- **Notes:** Lattice / scientific multi-table layout; T3 gold with cleaned percent cells

## 36_real_two_tables
- **PDF path:** `corpus/real/36_real_two_tables.pdf`
- **Source URL:** camelot `tests/files/twotables_1.pdf`
- **License:** Camelot test fixture
- **Retrieved:** (pre-existing corpus)
- **Use:** detect_smoke | structure_core (struggle)
- **Notes:** Two tables on one page; under_detect struggle T3 (partial grids)

## 37_real_liabilities_superscript
- **PDF path:** `corpus/real/37_real_liabilities_superscript.pdf`
- **Source URL:** camelot `tests/files/superscript.pdf`
- **License:** Camelot test fixture
- **Retrieved:** (pre-existing corpus)
- **Use:** detect_smoke
- **Notes:** Financial continued table with superscripts

## 38_real_irs_f1040
- **PDF path:** `corpus/real/38_real_irs_f1040.pdf`
- **Source URL:** https://www.irs.gov/pub/irs-pdf/f1040.pdf
- **License:** US government work (public domain)
- **Retrieved:** (pre-existing corpus)
- **Use:** detect_smoke / fp_smoke
- **Notes:** Form chrome ≠ data table (`form_not_table`)

## 39_real_fed_beigebook
- **PDF path:** `corpus/real/39_real_fed_beigebook.pdf`
- **Source URL:** https://www.federalreserve.gov/monetarypolicy/files/BeigeBook_20240306.pdf
- **License:** US government work
- **Retrieved:** (pre-existing corpus)
- **Use:** detect_smoke / fp_smoke
- **Notes:** Long narrative report; expected low table count

## 40_real_arxiv_tensorflow
- **PDF path:** `corpus/real/40_real_arxiv_tensorflow.pdf`
- **Source URL:** https://arxiv.org/abs/1603.04467
- **License:** arXiv paper — check paper license for redistribution
- **Retrieved:** (pre-existing corpus)
- **Use:** detect_smoke / fp_smoke
- **Notes:** Multi-column scientific prose; sparse tables possible

## 41_real_nist_withdrawn_notice
- **PDF path:** `corpus/real/41_real_nist_withdrawn_notice.pdf`
- **Source URL:** NIST publication portal (FIPS 197 family)
- **License:** US government work
- **Retrieved:** (pre-existing corpus)
- **Use:** detect_smoke / fp_smoke
- **Notes:** Long formal technical series PDF

## 42_real_insurance_italian
- **PDF path:** `corpus/real/42_real_insurance_italian.pdf`
- **Source URL:** Camelot sample / related open-source test fixture
- **License:** Third-party sample used in OSS tests; verify before shipping redistributed bundles
- **Retrieved:** (pre-existing corpus)
- **Use:** detect_smoke
- **Notes:** Non-English prose + embedded table regions

---

## Related inventories

| Path | Role |
|------|------|
| `benchmark/sources.json` | Legacy download provenance for `corpus/real` |
| `benchmark/corpus/compete_real/SOURCES.md` | R00x compete_real licenses |
| `benchmark/real_track/manifests/demotion_list.json` | Synthetic sets demoted from primary (PDFs kept) |
| `benchmark/real_track/manifests/coverage_matrix.json` | Mode → real doc coverage + waivers |

ICDAR-2013 competition set is **external only** — never listed here as an in-repo fixture.

## 42_real_insurance_italian
- **PDF path:** `corpus/real/42_real_insurance_italian.pdf`
- **Source URL:** Italian PIR unit-linked prospectus sample (pre-existing corpus)
- **License:** public prospectus sample
- **Retrieved:** pre-existing; structure gold 2026-07-12
- **Use:** structure_core (struggle: nested + non_english)
- **Notes:** Nested age/% table + outer key-value blocks

## 43_real_row_span_gmc
- **PDF path:** `corpus/real/43_real_row_span_gmc.pdf`
- **Source URL:** https://github.com/camelot-dev/camelot/raw/master/tests/files/row_span_1.pdf
- **License:** Camelot test fixture
- **Retrieved:** 2026-07-12
- **Use:** structure_core (struggle: row_span)
- **Notes:** CA GMC/COHS enrollment grid with merged Plan Type/County/Plan Name cells

## 44_real_spanning_cells
- **PDF path:** `corpus/real/44_real_spanning_cells.pdf`
- **Source URL:** https://github.com/tabulapdf/tabula-java/raw/master/src/test/resources/technology/tabula/spanning_cells.pdf
- **License:** Tabula test fixture
- **Retrieved:** 2026-07-12
- **Use:** structure_core (struggle: col_span multi_table)
- **Notes:** Server UEC scenario tables A4-12 / A4-13

## 45_real_campaign_donors
- **PDF path:** `corpus/real/45_real_campaign_donors.pdf`
- **Source URL:** https://github.com/tabulapdf/tabula-java/raw/master/src/test/resources/technology/tabula/campaign_donors.pdf
- **License:** Tabula test fixture
- **Retrieved:** 2026-07-12
- **Use:** structure_core (struggle: stream spanish dense)
- **Notes:** Argentine campaign donor list
