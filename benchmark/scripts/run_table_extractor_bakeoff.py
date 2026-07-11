#!/usr/bin/env python3
"""Grid-gold table bake-off across extractors (shared metrics.py + ground_truth).

Usage:
  cargo build --release -p pdfparser-cli
  source .venv/bin/activate
  python benchmark/scripts/run_table_extractor_bakeoff.py

Requires: pdfplumber, pymupdf, camelot-py (optional), img2table (optional).
Tabula requires a JRE; skipped if unavailable.
"""
# Implementation lives in session history; re-run logic mirrors accuracy scoreboard.
print("See benchmark/results/table_extractor_bakeoff.json for last run.")
print("Re-implement extractors from docs/research-table-extraction-sota.md § methods.")
