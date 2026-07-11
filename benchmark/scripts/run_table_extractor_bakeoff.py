#!/usr/bin/env python3
"""Table extractor bake-off — thin wrapper over the accuracy scoreboard.

Usage:
  cargo build --release -p pdfparser-cli
  source .venv/bin/activate
  python benchmark/scripts/run_table_extractor_bakeoff.py
  python benchmark/scripts/run_table_extractor_bakeoff.py --suite regression_hard

Full adapters (pdfparser, camelot lattice/stream/auto, img2table, pdfplumber,
pymupdf, …) live in run_benchmark.py and are scored by run_accuracy_benchmark.py.
"""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent


def main() -> None:
    args = sys.argv[1:] or ["--suite", "regression"]
    cmd = [sys.executable, str(SCRIPTS / "run_accuracy_benchmark.py"), *args]
    # Default libs: table-capable set
    if not any(a.startswith("--libs") for a in args):
        cmd.extend(
            [
                "--libs",
                "pdfparser,camelot_lattice,camelot_stream,camelot_auto,img2table,pdfplumber,pymupdf",
            ]
        )
    print("Running:", " ".join(cmd))
    raise SystemExit(subprocess.call(cmd))


if __name__ == "__main__":
    main()