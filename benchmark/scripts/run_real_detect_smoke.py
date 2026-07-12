#!/usr/bin/env python3
"""Lightweight real_detect_smoke runner (Phase 3 / PR6a harness).

Reads ``benchmark/real_track/manifests/real_detect_smoke_v0.json``.

Live mode runs:

  pdfparser extract --tables --format json <pdf>

Compares predicted table counts vs gold ``expected_table_count`` when set,
computes a simple count-based detection precision/recall proxy, and writes
``benchmark/real_track/results/real_detect_smoke_latest.json``.

Notes
-----
* Structure eval must force ``stitch_multipage=false`` (K27). The CLI may not
  yet expose that flag; this smoke runner records
  ``stitch_multipage="unknown_or_default"`` and a structure-eval note.
* Soft count gold only for v0 — not a market SOTA claim.
* Never uses ICDAR paths.

Usage
-----
  python3 benchmark/scripts/run_real_detect_smoke.py --help
  python3 benchmark/scripts/run_real_detect_smoke.py --dry-run
  python3 benchmark/scripts/run_real_detect_smoke.py --build
  python3 benchmark/scripts/run_real_detect_smoke.py --limit 3
  python3 benchmark/scripts/run_real_detect_smoke.py --strict
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

BENCH = Path(__file__).resolve().parents[1]
REPO = BENCH.parent
MANIFEST_DEFAULT = BENCH / "real_track" / "manifests" / "real_detect_smoke_v0.json"
RESULTS_DIR = BENCH / "real_track" / "results"
OUT_DEFAULT = RESULTS_DIR / "real_detect_smoke_latest.json"
RELEASE_BIN = REPO / "target" / "release" / "pdfparser"

STITCH_MULTIPAGE = "unknown_or_default"
STITCH_NOTE = (
    "Structure eval (real_structure) MUST set stitch_multipage=false so page-local "
    "tables are scored independently (design K27). Product Auto may keep stitch on "
    "by default. CLI flag may not exist yet — this smoke runner records "
    "stitch_multipage=unknown_or_default, prefers document-level tables when present "
    "else sum of page tables, and does NOT claim structure F1."
)
STRUCTURE_EVAL_NEEDS_STITCH_FALSE = True


def find_binary() -> Optional[Path]:
    for cand in (
        RELEASE_BIN,
        REPO / "target" / "debug" / "pdfparser",
    ):
        if cand.is_file():
            return cand
    which = shutil.which("pdfparser")
    return Path(which) if which else None


def try_build_release(timeout: float = 600.0) -> tuple[bool, str]:
    """Run cargo build --release -p pdfparser-cli. Returns (ok, message)."""
    cargo = shutil.which("cargo")
    if not cargo:
        return False, "cargo not found on PATH"
    cmd = [cargo, "build", "--release", "-p", "pdfparser-cli"]
    print(f"building release CLI: {' '.join(cmd)}", flush=True)
    try:
        proc = subprocess.run(
            cmd,
            cwd=str(REPO),
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        return False, f"cargo build timed out after {timeout}s"
    if proc.returncode != 0:
        tail = (proc.stderr or proc.stdout or "").strip().splitlines()[-20:]
        return False, "cargo build failed:\n" + "\n".join(tail)
    if not RELEASE_BIN.is_file():
        return False, f"build finished but binary missing: {RELEASE_BIN}"
    return True, f"built {RELEASE_BIN}"


def load_manifest(path: Path) -> dict:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data.get("documents"), list):
        raise SystemExit(f"manifest missing documents[]: {path}")
    return data


def load_gold_expected(gold_rel: Optional[str], manifest_expected: Any) -> tuple[Any, Any]:
    """Return (expected_table_count, gold_tol) preferring manifest, else gold file."""
    expected = manifest_expected
    tol = 0
    if gold_rel:
        gold_path = (
            REPO / "benchmark" / gold_rel
            if not gold_rel.startswith("benchmark/")
            else REPO / gold_rel
        )
        if not gold_path.exists():
            gold_path = BENCH / gold_rel
        if not gold_path.exists():
            gold_path = REPO / gold_rel
        if gold_path.exists():
            try:
                g = json.loads(gold_path.read_text(encoding="utf-8"))
            except Exception:
                g = {}
            if expected is None and "expected_table_count" in g:
                expected = g.get("expected_table_count")
            if "expected_table_count_tol" in g:
                tol = g.get("expected_table_count_tol") or 0
            if expected is None and isinstance(g.get("expected_tables"), list):
                expected = len(g["expected_tables"])
            if expected is None and g.get("expected_tables_min") is not None:
                # soft lower bound only — not a hard expected count
                pass
    return expected, tol


def resolve_pdf(pdf_rel: str) -> Path:
    # manifest paths like corpus/real/foo.pdf relative to benchmark/
    for base in (BENCH, REPO, REPO / "benchmark"):
        p = base / pdf_rel
        if p.is_file():
            return p
    return BENCH / pdf_rel


def _as_table_list(obj: Any) -> list:
    if isinstance(obj, list):
        return obj
    if isinstance(obj, dict):
        # occasional wrappers: {"items": [...]} / {"tables": [...]}
        for k in ("tables", "items", "data"):
            if isinstance(obj.get(k), list):
                return obj[k]
    return []


def count_tables_from_payload(payload: dict) -> tuple[int, str, list[dict], list[dict]]:
    """Return (n_pred, source_note, method_mix, per_page_counts).

    Handles flexible CLI JSON shapes:
      - root ``tables`` / ``table_count``
      - ``pages[].tables``
      - nested experimental wrappers when present
    """
    method_mix: list[dict] = []
    per_page: list[dict] = []

    def _entry(tab: dict, page: Any) -> dict:
        method = (
            tab.get("method")
            or tab.get("flavor")
            or tab.get("detector")
            or tab.get("source")
            or "unknown"
        )
        return {
            "page": page,
            "method": method,
            "rows": tab.get("rows"),
            "cols": tab.get("cols"),
        }

    # Prefer explicit document-level tables (stitched / logical) when non-empty.
    root_tables = _as_table_list(payload.get("tables"))
    if root_tables:
        for t in root_tables:
            if isinstance(t, dict):
                method_mix.append(_entry(t, t.get("page")))
        n = len(root_tables)
        # Cross-check table_count if present
        tc = payload.get("table_count")
        if isinstance(tc, int) and tc != n:
            # trust list length; record discrepancy in source note
            return n, f"document_tables_root(table_count={tc})", method_mix, per_page
        return n, "document_tables_root", method_mix, per_page

    # experimental.tables fallback
    exp = payload.get("experimental")
    if isinstance(exp, dict):
        exp_tables = _as_table_list(exp.get("tables"))
        if exp_tables:
            for t in exp_tables:
                if isinstance(t, dict):
                    method_mix.append(_entry(t, t.get("page")))
            return len(exp_tables), "experimental_tables", method_mix, per_page

    pages = payload.get("pages") or []
    n = 0
    if isinstance(pages, list):
        for pi, page in enumerate(pages):
            if not isinstance(page, dict):
                continue
            page_idx = page.get("index", page.get("page", pi))
            tabs = _as_table_list(page.get("tables"))
            n_page = len(tabs)
            n += n_page
            per_page.append({"page": page_idx, "n_tables": n_page})
            for t in tabs:
                if isinstance(t, dict):
                    method_mix.append(_entry(t, page_idx))

    if n == 0:
        # bare table_count with empty lists
        tc = payload.get("table_count")
        if isinstance(tc, int):
            return tc, "table_count_field", method_mix, per_page

    return n, "page_tables_sum", method_mix, per_page


def run_extract(bin_path: Path, pdf: Path, timeout: float) -> tuple[dict, float]:
    # Prefer --tables --format json (README order); equivalent to --format json --tables.
    t0 = time.perf_counter()
    proc = subprocess.run(
        [str(bin_path), "extract", "--tables", "--format", "json", str(pdf)],
        capture_output=True,
        text=True,
        timeout=timeout,
    )
    wall = (time.perf_counter() - t0) * 1000.0
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or f"exit {proc.returncode}")
    try:
        payload = json.loads(proc.stdout or "{}")
    except json.JSONDecodeError as e:
        raise RuntimeError(f"invalid json: {e}") from e
    return payload, wall


def count_match(n_pred: int, expected: Any, tol: int) -> Optional[bool]:
    if expected is None:
        return None
    try:
        exp = int(expected)
    except (TypeError, ValueError):
        return None
    return abs(n_pred - exp) <= int(tol or 0)


def count_based_det_proxy(n_pred: int, n_exp: int) -> dict[str, Any]:
    """Simple set-size proxy for detection: TP=min, FP/FN=surplus/deficit."""
    tp = min(n_pred, n_exp)
    fp = max(0, n_pred - n_exp)
    fn = max(0, n_exp - n_pred)
    precision = (tp / (tp + fp)) if (tp + fp) > 0 else (1.0 if n_exp == 0 and n_pred == 0 else 0.0)
    recall = (tp / (tp + fn)) if (tp + fn) > 0 else (1.0 if n_exp == 0 and n_pred == 0 else 0.0)
    f1 = (
        (2 * precision * recall / (precision + recall))
        if (precision + recall) > 0
        else 0.0
    )
    return {
        "tp": tp,
        "fp": fp,
        "fn": fn,
        "precision": round(precision, 4),
        "recall": round(recall, 4),
        "f1": round(f1, 4),
    }


def aggregate_det_proxy(rows: list[dict]) -> Optional[dict[str, Any]]:
    """Micro-average count-based P/R over docs with numeric n_exp."""
    tp = fp = fn = 0
    n = 0
    for r in rows:
        proxy = r.get("det_proxy")
        if not isinstance(proxy, dict):
            continue
        if r.get("n_exp") is None:
            continue
        tp += int(proxy.get("tp") or 0)
        fp += int(proxy.get("fp") or 0)
        fn += int(proxy.get("fn") or 0)
        n += 1
    if n == 0:
        return None
    precision = (tp / (tp + fp)) if (tp + fp) > 0 else 1.0
    recall = (tp / (tp + fn)) if (tp + fn) > 0 else 1.0
    f1 = (
        (2 * precision * recall / (precision + recall))
        if (precision + recall) > 0
        else 0.0
    )
    return {
        "n_docs_scored": n,
        "tp": tp,
        "fp": fp,
        "fn": fn,
        "precision": round(precision, 4),
        "recall": round(recall, 4),
        "f1": round(f1, 4),
        "note": "count-based micro proxy (not IoU/bbox); soft gold only",
    }


def main(argv: Optional[list[str]] = None) -> int:
    ap = argparse.ArgumentParser(
        description=(
            "Run real_detect_smoke: soft table-count smoke on licensed real PDFs. "
            "Live extract + baseline artifact under real_track/results/."
        )
    )
    ap.add_argument(
        "--manifest",
        type=Path,
        default=MANIFEST_DEFAULT,
        help=f"Path to suite manifest (default: {MANIFEST_DEFAULT})",
    )
    ap.add_argument(
        "--out",
        type=Path,
        default=OUT_DEFAULT,
        help=f"Results JSON path (default: {OUT_DEFAULT})",
    )
    ap.add_argument(
        "--dry-run",
        action="store_true",
        help="List docs and gold expectations; do not invoke pdfparser.",
    )
    ap.add_argument("--limit", type=int, default=0, help="Process at most N docs (0 = all).")
    ap.add_argument("--timeout", type=float, default=180.0, help="Per-doc CLI timeout seconds.")
    ap.add_argument(
        "--build",
        action="store_true",
        help=(
            "If target/release/pdfparser is missing, run "
            "`cargo build --release -p pdfparser-cli` before extract."
        ),
    )
    ap.add_argument(
        "--strict",
        action="store_true",
        help=(
            "Exit 1 when any scored doc mismatches expected_table_count "
            "(or extract/pdf failures). Default: exit 0 with summary."
        ),
    )
    ap.add_argument(
        "--require-binary",
        action="store_true",
        help=argparse.SUPPRESS,  # legacy alias: same as default live behavior (exit 2)
    )
    args = ap.parse_args(argv)

    manifest = load_manifest(args.manifest)
    docs = list(manifest.get("documents") or [])
    if args.limit and args.limit > 0:
        docs = docs[: args.limit]

    binary = find_binary()
    print(f"manifest: {args.manifest}")
    print(f"docs: {len(docs)} (suite={manifest.get('suite')})")
    print(f"binary: {binary if binary else 'NOT FOUND'}")
    print(f"stitch_multipage: {STITCH_MULTIPAGE}")
    print(f"structure_eval_needs_stitch_false: {STRUCTURE_EVAL_NEEDS_STITCH_FALSE}")
    print(f"stitch note: {STITCH_NOTE[:80]}...")

    if args.dry_run:
        for d in docs:
            pdf = resolve_pdf(d["pdf"])
            exp, tol = load_gold_expected(d.get("gold"), d.get("expected_table_count"))
            print(
                f"  {d['id']}: pdf={'OK' if pdf.is_file() else 'MISSING'} "
                f"expected={exp} tol={tol} path={pdf}"
            )
        print("dry-run complete (no extract).")
        return 0

    # --- live mode: binary required (or --build) ---
    if binary is None and args.build:
        ok, msg = try_build_release()
        print(msg)
        if not ok:
            print(
                "ERROR: failed to build release pdfparser. "
                "Fix the build or install a binary, then re-run.",
                file=sys.stderr,
            )
            return 2
        binary = find_binary()

    if binary is None:
        msg = (
            "pdfparser binary not found at target/release/pdfparser "
            "(also checked target/debug and PATH).\n"
            "  Build with:  cargo build --release -p pdfparser-cli\n"
            "  Or re-run:   python3 benchmark/scripts/run_real_detect_smoke.py --build\n"
            "  Dry-run:     python3 benchmark/scripts/run_real_detect_smoke.py --dry-run"
        )
        print(msg, file=sys.stderr)
        return 2

    rows: list[dict] = []
    n_scored = 0
    n_match = 0
    n_mismatch = 0
    n_unscored = 0
    n_fail = 0

    for d in docs:
        doc_id = d["id"]
        pdf = resolve_pdf(d["pdf"])
        exp, tol = load_gold_expected(d.get("gold"), d.get("expected_table_count"))
        n_exp: Optional[int]
        try:
            n_exp = int(exp) if exp is not None else None
        except (TypeError, ValueError):
            n_exp = None

        row: dict[str, Any] = {
            "id": doc_id,
            "pdf": d.get("pdf"),
            "pdf_resolved": str(pdf),
            "n_exp": n_exp,
            "expected_table_count": exp,
            "expected_table_count_tol": tol,
            "failure_classes": d.get("failure_classes") or [],
            "method_mix": [],
            "method_mix_placeholder": True,
            "n_pred": None,
            "n_pred_source": None,
            "per_page_table_counts": [],
            "count_match": None,
            "ok": None,
            "det_proxy": None,
            "wall_ms": None,
            "status": "pending",
            "error": None,
        }
        if not pdf.is_file():
            row["status"] = "missing_pdf"
            row["error"] = f"pdf not found: {pdf}"
            row["ok"] = False
            n_fail += 1
            rows.append(row)
            print(f"  MISS  {doc_id}: {pdf}")
            continue
        try:
            payload, wall = run_extract(binary, pdf, args.timeout)
            n_pred, src, mix, per_page = count_tables_from_payload(payload)
            row["n_pred"] = n_pred
            row["n_pred_source"] = src
            row["per_page_table_counts"] = per_page
            row["method_mix"] = mix or [
                {"page": None, "method": "unknown", "note": "CLI JSON lacked method field"}
            ]
            row["wall_ms"] = round(wall, 2)
            matched = count_match(n_pred, exp, tol)
            row["count_match"] = matched
            row["status"] = "ok"
            if n_exp is not None:
                row["det_proxy"] = count_based_det_proxy(n_pred, n_exp)
            if matched is None:
                # unscored: extract succeeded; ok=True (no hard expectation)
                row["ok"] = True
                n_unscored += 1
                tag = "soft"
            elif matched:
                row["ok"] = True
                n_scored += 1
                n_match += 1
                tag = "MATCH"
            else:
                row["ok"] = False
                n_scored += 1
                n_mismatch += 1
                tag = "DIFF"
            print(
                f"  {tag:5} {doc_id}: n_pred={n_pred} n_exp={n_exp} "
                f"ok={row['ok']} ({wall:.0f} ms)"
            )
        except Exception as e:
            row["status"] = "error"
            row["error"] = str(e)
            row["ok"] = False
            n_fail += 1
            print(f"  ERR   {doc_id}: {e}", file=sys.stderr)
        rows.append(row)

    det_proxy_summary = aggregate_det_proxy(rows)
    summary = {
        "n_docs": len(docs),
        "n_ok": sum(1 for r in rows if r.get("ok") is True),
        "n_not_ok": sum(1 for r in rows if r.get("ok") is False),
        "n_extract_ok": sum(1 for r in rows if r["status"] == "ok"),
        "n_fail": n_fail,
        "n_count_scored": n_scored,
        "n_count_match": n_match,
        "n_count_mismatch": n_mismatch,
        "n_count_unscored": n_unscored,
        "count_match_rate": (n_match / n_scored) if n_scored else None,
        "det_proxy": det_proxy_summary,
    }

    out_obj = {
        "suite": manifest.get("suite", "real_detect_smoke"),
        "version": manifest.get("version"),
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "binary": str(binary),
        "status": "ran",
        "stitch_multipage": STITCH_MULTIPAGE,
        "stitch_multipage_note": STITCH_NOTE,
        "structure_eval_needs_stitch_false": STRUCTURE_EVAL_NEEDS_STITCH_FALSE,
        "method_mix_placeholder": True,
        "method_mix_note": (
            "CLI may not yet emit per-table method; entries use method/flavor/detector "
            "when present, else 'unknown'. EngineV2 shadow will fill real method_mix."
        ),
        "det_proxy_note": (
            "Count-based detection proxy only: TP=min(n_pred,n_exp), "
            "FP=max(0,n_pred-n_exp), FN=max(0,n_exp-n_pred). Not IoU/bbox matching."
        ),
        "summary": summary,
        "documents": rows,
    }

    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(out_obj, indent=2) + "\n", encoding="utf-8")
    print(f"summary: {json.dumps(summary)}")
    print(f"wrote: {args.out}")

    if args.strict:
        hard = n_mismatch + n_fail
        if hard:
            print(
                f"strict: {hard} hard failure(s) "
                f"(mismatch={n_mismatch}, fail={n_fail}) → exit 1",
                file=sys.stderr,
            )
            return 1
    # Default: never fail the suite hard on count mismatches — report via summary.
    return 0


if __name__ == "__main__":
    sys.exit(main())
