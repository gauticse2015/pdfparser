#!/usr/bin/env python3
"""Download legal public complex PDFs for compete_real tier.

Sources are government / open data / library samples with public URLs.
We do NOT download ICDAR competition packages.

Each entry: url, id, license note, expected_table_count (manual estimate),
failure_classes, must_contain (filled after download via text extract probe).

  python benchmark/scripts/fetch_compete_real.py
"""
from __future__ import annotations

import json
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REAL = ROOT / "corpus" / "compete_real"
GT = ROOT / "ground_truth"
SUITES = ROOT / "corpus" / "suites.json"
REAL.mkdir(parents=True, exist_ok=True)

# Curated public PDFs (stable government / open URLs). If a URL 404s, skip.
SOURCES = [
    {
        "id": "R001_irs_f1040_schedule_c",
        "url": "https://www.irs.gov/pub/irs-pdf/f1040sc.pdf",
        "license": "US government work (public domain)",
        "failure_classes": ["C1_OVER_DETECT", "C8_MULTI_TABLE_COUNT_ERROR", "C3_UNDER_DETECT"],
        "description": "IRS Schedule C form — dense form grids / false tables",
        "expected_table_count_hint": None,  # form-heavy; gold = soft detect
        "soft_gold": True,
    },
    {
        "id": "R002_irs_f1040",
        "url": "https://www.irs.gov/pub/irs-pdf/f1040.pdf",
        "license": "US government work (public domain)",
        "failure_classes": ["C1_OVER_DETECT", "C14_INVOICE_FOOTER"],
        "description": "IRS Form 1040 — form boxes as lattice FPs",
        "soft_gold": True,
    },
    {
        "id": "R003_bea_gdp_sample",
        "url": "https://apps.bea.gov/national/pdf/SNTables.pdf",
        "license": "US government work",
        "failure_classes": ["C4_ROW_FRAGMENT_LARGE", "C6_COL_SKEW_WIDE", "C9_MULTIPAGE"],
        "description": "BEA NIPA summary tables — large multipage statistical",
        "soft_gold": True,
    },
    {
        "id": "R004_bls_ces_sample",
        "url": "https://www.bls.gov/web/empsit/ceseeb1.pdf",
        "license": "US government work",
        "failure_classes": ["C4_ROW_FRAGMENT_LARGE", "C6_COL_SKEW_WIDE", "C5_HEADER_ONLY_SLICE"],
        "description": "BLS CES employment table — wide multipage stats",
        "soft_gold": True,
    },
    {
        "id": "R005_census_acs_sample",
        "url": "https://www2.census.gov/programs-surveys/acs/tech_docs/subject_definitions/2022_ACSSubjectDefinitions.pdf",
        "license": "US government work",
        "failure_classes": ["C1_OVER_DETECT", "C9_MULTIPAGE"],
        "description": "Census ACS subject definitions — multipage text+tables mix",
        "soft_gold": True,
    },
    {
        "id": "R006_fed_beigebook",
        "url": "https://www.federalreserve.gov/monetarypolicy/files/BeigeBook_20240117.pdf",
        "license": "US government work",
        "failure_classes": ["C1_OVER_DETECT", "C9_MULTIPAGE", "C4_ROW_FRAGMENT_LARGE"],
        "description": "Fed Beige Book — multipage, sparse tables vs prose",
        "soft_gold": True,
    },
    {
        "id": "R007_nist_fips197",
        "url": "https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.197.pdf",
        "license": "US government work",
        "failure_classes": ["C1_OVER_DETECT", "C11_SAME_SHAPE_ZERO_TEDS"],
        "description": "NIST FIPS 197 AES — technical tables + hex matrices",
        "soft_gold": True,
    },
    {
        "id": "R008_cdc_mmwr_sample",
        "url": "https://www.cdc.gov/mmwr/volumes/72/wr/pdfs/mm7226a1-H.pdf",
        "license": "US government work",
        "failure_classes": ["C4_ROW_FRAGMENT_LARGE", "C6_COL_SKEW_WIDE", "C8_MULTI_TABLE_COUNT_ERROR"],
        "description": "CDC MMWR article — scientific multi-table layout",
        "soft_gold": True,
    },
    {
        "id": "R009_sec_10q_sample",
        "url": "https://www.sec.gov/Archives/edgar/data/320193/000032019324000006/a10-q12302023.htm",
        # HTML not PDF — skip if fails; prefer a known open PDF sample
        "url": "https://www.sec.gov/files/form10-q.pdf",
        "license": "US government work (SEC form)",
        "failure_classes": ["C1_OVER_DETECT", "C14_INVOICE_FOOTER"],
        "description": "SEC Form 10-Q blank form — form lattice FPs",
        "soft_gold": True,
    },
    {
        "id": "R010_who_sample",
        "url": "https://iris.who.int/bitstream/handle/10665/331532/9789240003927-eng.pdf",
        "license": "WHO publications — check terms; used for research evaluation only",
        "failure_classes": ["C4_ROW_FRAGMENT_LARGE", "C9_MULTIPAGE", "C8_MULTI_TABLE_COUNT_ERROR"],
        "description": "WHO multi-table report sample",
        "soft_gold": True,
    },
    {
        "id": "R011_eurostat_sample",
        "url": "https://ec.europa.eu/eurostat/documents/3217494/5728433/KS-RA-13-028-EN.PDF",
        "license": "Eurostat — free reuse with attribution",
        "failure_classes": ["C6_COL_SKEW_WIDE", "C9_MULTIPAGE", "C4_ROW_FRAGMENT_LARGE"],
        "description": "Eurostat methodology / tables PDF",
        "soft_gold": True,
    },
    {
        "id": "R012_worldbank_sample",
        "url": "https://thedocs.worldbank.org/en/doc/18ad707266f7740bced755498ae0307a-0350012021/original/mpo-sm21.pdf",
        "license": "World Bank open knowledge — attribution",
        "failure_classes": ["C9_MULTIPAGE", "C6_COL_SKEW_WIDE", "C4_ROW_FRAGMENT_LARGE"],
        "description": "World Bank macro poverty outlook sample",
        "soft_gold": True,
    },
]


def download(url: str, dest: Path, timeout=60) -> bool:
    try:
        req = urllib.request.Request(url, headers={"User-Agent": "pdfparser-compete-fetch/1.0"})
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            data = resp.read()
        if len(data) < 1000 or not data[:5].startswith(b"%PDF"):
            print(f"  skip not pdf or too small: {url} ({len(data)} bytes)")
            return False
        dest.write_bytes(data)
        return True
    except Exception as e:
        print(f"  FAIL {url}: {e}")
        return False


def main():
    # dedupe by id (R009 had duplicate url keys in source)
    seen = set()
    sources = []
    for s in SOURCES:
        if s["id"] in seen:
            continue
        seen.add(s["id"])
        sources.append(s)

    got = []
    print("Fetching compete_real public PDFs…")
    for s in sources:
        dest = REAL / f"{s['id']}.pdf"
        print(" ", s["id"])
        if dest.exists() and dest.stat().st_size > 1000:
            print("   exists", dest.stat().st_size)
            ok = True
        else:
            ok = download(s["url"], dest)
        if not ok:
            continue
        # Soft gold: expected_table_count null → metrics use must_contain + optional count
        # Probe page count via pdfplumber if available
        page_count = 1
        try:
            import pdfplumber

            with pdfplumber.open(dest) as pdf:
                page_count = len(pdf.pages)
        except Exception:
            pass
        gt = {
            "id": s["id"],
            "tier": "compete_real",
            "suite": "regression_compete",
            "source": "public_download",
            "icdar_derived": False,
            "path": f"corpus/compete_real/{s['id']}.pdf",
            "license": s["license"],
            "source_url": s["url"],
            "description": s["description"],
            "failure_classes": s["failure_classes"],
            "challenges": s["failure_classes"],
            "soft_gold": True,
            "weight_text": 0.2,
            "weight_tables": 0.8,
            "weight_objects": 0.0,
            "page_count": page_count,
            # Soft: do not force exact grid; detection scored only if expected_table_count set
            "must_contain": [],
            "notes": (
                "Soft gold: download for stress/manual labeling. "
                "Add expected_table_count and expected_tables after human review."
            ),
            "quality_notes": "OPEN for labeling — baseline extract stored separately",
        }
        (GT / f"{s['id']}.json").write_text(json.dumps(gt, indent=2) + "\n")
        got.append(s["id"])
        print("   ok pages", page_count)

    # merge into suites document_ids
    if SUITES.exists():
        data = json.loads(SUITES.read_text())
        suite = data.setdefault("suites", {}).setdefault("regression_compete", {})
        ids = list(suite.get("document_ids") or [])
        for rid in got:
            if rid not in ids:
                ids.append(rid)
        suite["document_ids"] = ids
        suite["tiers"] = ["compete", "compete_real"]
        SUITES.write_text(json.dumps(data, indent=2) + "\n")
    print(f"Fetched {len(got)} real PDFs:", got)
    # sources manifest
    (REAL / "SOURCES.md").write_text(
        "# compete_real sources\n\n"
        + "\n".join(
            f"- `{s['id']}`: {s['url']}  \n  License: {s['license']}\n" for s in sources if s["id"] in got
        )
        + "\nICDAR competition PDFs are **not** included.\n"
    )


if __name__ == "__main__":
    main()
