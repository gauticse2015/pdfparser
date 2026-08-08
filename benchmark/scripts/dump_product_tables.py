#!/usr/bin/env python3
"""Dump product Auto tables for freeze compare (P0.6).

Resolves g2 freeze ids via real_structure manifest (does NOT invent g2_core.json).
One CLI extract per PDF: ``--format json --dump-evidence``.

  python3 benchmark/scripts/dump_product_tables.py \\
    --binary target/release/pdfparser \\
    --freeze benchmark/real_track/freezes/g2.json \\
    --structure-manifest benchmark/real_track/manifests/real_structure_v0.json \\
    --out /tmp/after.json
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any, Optional

SCRIPTS = Path(__file__).resolve().parent
BENCH = SCRIPTS.parent
REPO = BENCH.parent
RELEASE_BIN = REPO / "target" / "release" / "pdfparser"

SCHEMA = "pdfparser_table_dump_v1"

# Product Auto: use_engine_v2 && !legacy_router => engine_v2. Do not parse stderr.
ENGINE_PATH_BY_PRESET = {
    "auto": "engine_v2",
    "engine-v2": "engine_v2",
    "high-quality": "engine_v2",
    "fast": "engine_v2",
    "full": "engine_v2",
    "lattice-only": "legacy",
}


def find_binary() -> Optional[Path]:
    for cand in (RELEASE_BIN, REPO / "target" / "debug" / "pdfparser"):
        if cand.is_file():
            return cand
    which = shutil.which("pdfparser")
    return Path(which) if which else None


def resolve_pdf(pdf_rel: str) -> Path:
    for base in (BENCH, REPO, REPO / "benchmark"):
        p = base / pdf_rel
        if p.is_file():
            return p
    return BENCH / pdf_rel


def table_sort_key(t: dict) -> tuple:
    """K27 emit order: (-bbox.y1, bbox.x0, page)."""
    bb = t.get("bbox") or {}
    y1 = float(bb.get("y1", 0.0))
    x0 = float(bb.get("x0", 0.0))
    page = int(t.get("page", 0))
    return (-y1, x0, page)


def freeze_ids(freeze: dict) -> list[str]:
    ids: list[str] = []
    for d in freeze.get("documents_auto") or []:
        did = d.get("id")
        if did:
            ids.append(str(did))
    return ids


def manifest_by_id(man: dict) -> dict[str, dict]:
    out: dict[str, dict] = {}
    for d in man.get("documents") or []:
        did = d.get("id")
        if did:
            out[str(did)] = d
    return out


def pages_from_payload(payload: dict) -> list[dict[str, Any]]:
    pages_out: list[dict[str, Any]] = []
    pages = payload.get("pages") or []
    if pages:
        for i, page in enumerate(pages):
            if not isinstance(page, dict):
                continue
            idx = int(page.get("index", i))
            tabs = [t for t in (page.get("tables") or []) if isinstance(t, dict)]
            for t in tabs:
                t.setdefault("page", idx)
            tabs.sort(key=table_sort_key)
            pages_out.append({"index": idx, "tables": tabs})
        pages_out.sort(key=lambda p: p["index"])
        return pages_out
    grouped: dict[int, list[dict]] = {}
    for t in payload.get("tables") or []:
        if not isinstance(t, dict):
            continue
        idx = int(t.get("page", 0))
        grouped.setdefault(idx, []).append(t)
    for idx in sorted(grouped):
        tabs = grouped[idx]
        tabs.sort(key=table_sort_key)
        pages_out.append({"index": idx, "tables": tabs})
    return pages_out


def extract_one(binary: Path, pdf: Path, preset: str, timeout: float) -> dict:
    cmd = [
        str(binary),
        "extract",
        "--tables",
        "--no-stitch",
        "--page-tables",
        "--format",
        "json",
        "--dump-evidence",
        "--table-preset",
        preset,
        str(pdf),
    ]
    proc = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        timeout=timeout,
        cwd=str(REPO),
    )
    if proc.returncode != 0:
        err = (proc.stderr or proc.stdout or "extract failed")[:500]
        raise RuntimeError(f"extract rc={proc.returncode}: {err}")
    try:
        return json.loads(proc.stdout)
    except json.JSONDecodeError as e:
        raise RuntimeError(f"json decode: {e}") from e


def build_dump(
    binary: Path,
    freeze: dict,
    manifest: dict,
    preset: str,
    timeout: float,
    dry_run: bool = False,
) -> dict[str, Any]:
    ids = freeze_ids(freeze)
    by_id = manifest_by_id(manifest)
    missing = [i for i in ids if i not in by_id]
    if missing:
        raise SystemExit(f"freeze ids missing from structure manifest: {missing}")
    engine_path = ENGINE_PATH_BY_PRESET.get(preset, "engine_v2")
    documents: list[dict[str, Any]] = []
    for did in ids:
        entry = by_id[did]
        pdf_rel = entry.get("pdf") or f"corpus/real/{did}.pdf"
        pdf = resolve_pdf(pdf_rel)
        rec: dict[str, Any] = {
            "id": did,
            "pdf": pdf_rel,
        }
        if dry_run:
            rec["pages"] = []
            rec["pdf_exists"] = pdf.is_file()
            documents.append(rec)
            print(f"  {did}: pdf={'OK' if pdf.is_file() else 'MISS'} {pdf_rel}")
            continue
        if not pdf.is_file():
            raise SystemExit(f"missing PDF for {did}: {pdf}")
        print(f"  extract {did} ...", flush=True)
        payload = extract_one(binary, pdf, preset, timeout)
        rec["pages"] = pages_from_payload(payload)
        documents.append(rec)
    return {
        "schema": SCHEMA,
        "binary": str(binary),
        "preset": preset,
        "engine_path": engine_path,
        "stitch_multipage": False,
        "documents": documents,
    }


def main() -> int:
    ap = argparse.ArgumentParser(description="Dump product Auto tables (pdfparser_table_dump_v1).")
    ap.add_argument(
        "--binary",
        type=Path,
        default=None,
        help="pdfparser binary (default: discover target/release/pdfparser)",
    )
    ap.add_argument(
        "--freeze",
        type=Path,
        default=BENCH / "real_track" / "freezes" / "g2.json",
        help="g2.json (core ids = documents_auto[].id)",
    )
    ap.add_argument(
        "--structure-manifest",
        type=Path,
        default=BENCH / "real_track" / "manifests" / "real_structure_v0.json",
        help="real_structure_v0.json (pdf paths)",
    )
    ap.add_argument("--out", type=Path, required=True, help="Output dump.json")
    ap.add_argument("--preset", default="auto", help="CLI --table-preset (default auto)")
    ap.add_argument("--timeout", type=float, default=180.0)
    ap.add_argument("--dry-run", action="store_true", help="Resolve ids/pdfs only; no extract")
    args = ap.parse_args()

    if not args.freeze.is_file():
        print(f"missing freeze {args.freeze}", file=sys.stderr)
        return 2
    if not args.structure_manifest.is_file():
        print(f"missing structure manifest {args.structure_manifest}", file=sys.stderr)
        return 2

    if args.binary is not None:
        binary = args.binary.expanduser()
    else:
        binary = find_binary()
    if not args.dry_run:
        if binary is None or not binary.is_file():
            print(
                f"missing pdfparser binary: {binary or RELEASE_BIN}; "
                "pass --binary or cargo build --release -p pdfparser-cli",
                file=sys.stderr,
            )
            return 2
    elif binary is None:
        binary = RELEASE_BIN

    freeze = json.loads(args.freeze.read_text(encoding="utf-8"))
    man = json.loads(args.structure_manifest.read_text(encoding="utf-8"))
    try:
        dump = build_dump(
            binary=binary,
            freeze=freeze,
            manifest=man,
            preset=args.preset,
            timeout=args.timeout,
            dry_run=args.dry_run,
        )
    except subprocess.TimeoutExpired:
        print("extract timeout", file=sys.stderr)
        return 2
    except RuntimeError as e:
        print(str(e), file=sys.stderr)
        return 2

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(dump, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {args.out} schema={SCHEMA} n_docs={len(dump['documents'])} engine_path={dump['engine_path']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
