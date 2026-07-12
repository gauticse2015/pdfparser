#!/usr/bin/env python3
"""Render PDF pages to PNG for *in-session* Grok multimodal gold building.

No external API key. Workflow:
  1. This script renders pages (pdftoppm)
  2. Agent reads PNGs with read_file (vision)
  3. Agent writes gold/drafts/peer/<id>.vision.json
  4. Cross-check with build_structure_gold_peers.py (Camelot)

Usage:
  python3 benchmark/scripts/prepare_vision_gold_pages.py --pdf path.pdf --pages 1
  python3 benchmark/scripts/prepare_vision_gold_pages.py --all-soft-gold
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

BENCH = Path(__file__).resolve().parents[1]
OUT = BENCH / "real_track" / "gold" / "drafts" / "vision_renders"
SOFT = [
    "30_real_ca_warn_report",
    "31_real_background_checks",
    "32_real_census_table324",
    "33_real_argentina_votes",
    "34_real_schools_contributions",
    "35_real_camelot_fuel",
    "36_real_two_tables",
    "37_real_liabilities_superscript",
]


def render(pdf: Path, page_1based: int, dpi: int, out_dir: Path) -> Path:
    out_dir.mkdir(parents=True, exist_ok=True)
    prefix = out_dir / f"{pdf.stem}_p{page_1based}"
    subprocess.run(
        [
            "pdftoppm",
            "-png",
            "-r",
            str(dpi),
            "-f",
            str(page_1based),
            "-l",
            str(page_1based),
            "-singlefile",
            str(pdf),
            str(prefix),
        ],
        check=True,
        capture_output=True,
    )
    png = prefix.with_suffix(".png")
    if not png.is_file():
        raise SystemExit(f"missing render {png}")
    return png


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--pdf", type=Path)
    ap.add_argument("--pages", default="1", help="1-based pages, e.g. 1 or 1-2")
    ap.add_argument("--dpi", type=int, default=150)
    ap.add_argument("--all-soft-gold", action="store_true")
    ap.add_argument("--out-dir", type=Path, default=OUT)
    args = ap.parse_args()

    jobs = []
    if args.all_soft_gold:
        for sid in SOFT:
            p = BENCH / "corpus" / "real" / f"{sid}.pdf"
            if p.is_file():
                jobs.append(p)
    elif args.pdf:
        jobs.append(args.pdf)
    else:
        ap.error("--pdf or --all-soft-gold")

    pages = []
    for part in args.pages.split(","):
        if "-" in part:
            a, b = part.split("-", 1)
            pages.extend(range(int(a), int(b) + 1))
        else:
            pages.append(int(part))

    manifest = []
    for pdf in jobs:
        for pg in pages:
            png = render(pdf, pg, args.dpi, args.out_dir)
            print(f"rendered {png}")
            manifest.append(
                {
                    "id": pdf.stem,
                    "pdf": str(pdf),
                    "page_1based": pg,
                    "png": str(png),
                    "agent_instruction": (
                        "Read this PNG with multimodal vision; extract full cell grids "
                        "as T3 gold draft. Cross-check Camelot peer draft. Do not use pdfparser."
                    ),
                }
            )
    idx = args.out_dir / "VISION_MANIFEST.json"
    idx.write_text(json.dumps({"renders": manifest}, indent=2), encoding="utf-8")
    print(f"wrote {idx} n={len(manifest)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
