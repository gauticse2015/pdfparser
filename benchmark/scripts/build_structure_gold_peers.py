#!/usr/bin/env python3
"""Build structure-gold DRAFTS from independent peers — NEVER from pdfparser.

Primary sources (in priority order for lattice-like docs):
  1. Camelot lattice  (ruled grids — usually best)
  2. Camelot stream   (borderless / when lattice weak)
  3. pdfplumber extract_tables

Optional:
  4. Vision LLM (XAI_API_KEY / OPENAI_API_KEY + rendered page PNG) — if configured

Output:
  benchmark/real_track/gold/drafts/peer/<id>.peer.json
  with per-cell confidence, peer agreement map, human_focus cells.

Policy:
  - pdfparser may be run only as optional *shadow* for comparison; never as gold authority.
  - status always needs_human_review | peer_consensus_draft (never reviewed).
  - Human reviews high-confidence peer golds quickly; focuses on dispute cells.

Usage:
  source .venv/bin/activate
  python3 benchmark/scripts/build_structure_gold_peers.py \\
    --pdf benchmark/corpus/real/30_real_ca_warn_report.pdf
  python3 benchmark/scripts/build_structure_gold_peers.py --all-soft-gold
  python3 benchmark/scripts/build_structure_gold_peers.py --pdf ... --vision
"""
from __future__ import annotations

import argparse
import base64
import json
import os
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field, asdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

SCRIPTS = Path(__file__).resolve().parent
BENCH = SCRIPTS.parent
REPO = BENCH.parent
OUT_DIR = BENCH / "real_track" / "gold" / "drafts" / "peer"
def _relpath(path: Path, base: Path) -> str:
    try:
        return str(path.resolve().relative_to(base.resolve()))
    except Exception:
        return str(path)


SOFT_GOLD = [
    "30_real_ca_warn_report",
    "31_real_background_checks",
    "32_real_census_table324",
    "33_real_argentina_votes",
    "34_real_schools_contributions",
    "35_real_camelot_fuel",
    "36_real_two_tables",
    "37_real_liabilities_superscript",
]


def normalize_cell(s: Any) -> str:
    if s is None:
        return ""
    t = str(s).replace("\r", "\n")
    t = re.sub(r"[ \t]+", " ", t)
    t = re.sub(r"\n+", "\n", t)
    return t.strip()


def grid_from_df_values(rows: list) -> list[list[str]]:
    return [[normalize_cell(c) for c in row] for row in rows]


def shape(grid: list[list[str]]) -> tuple[int, int]:
    if not grid:
        return 0, 0
    cols = max((len(r) for r in grid), default=0)
    # pad ragged
    out = []
    for r in grid:
        rr = list(r) + [""] * (cols - len(r))
        out.append(rr[:cols])
    return len(out), cols


@dataclass
class PeerTable:
    source: str
    page: int  # 0-based
    rows: int
    cols: int
    cells: list[list[str]]
    bbox: Optional[dict] = None
    accuracy: Optional[float] = None
    whitespace: Optional[float] = None
    flavor: Optional[str] = None
    notes: list[str] = field(default_factory=list)


def extract_camelot(pdf: Path, pages: str, flavor: str) -> list[PeerTable]:
    import camelot

    try:
        tables = camelot.read_pdf(str(pdf), pages=pages, flavor=flavor)
    except Exception as e:
        return []
    out: list[PeerTable] = []
    for i in range(tables.n):
        t = tables[i]
        df = t.df
        cells = grid_from_df_values(df.values.tolist())
        r, c = shape(cells)
        # camelot page is 1-based in report
        page = int(t.parsing_report.get("page") or 1) - 1
        acc = t.parsing_report.get("accuracy")
        try:
            acc_f = float(acc) if acc is not None else None
        except (TypeError, ValueError):
            acc_f = None
        ws = t.parsing_report.get("whitespace")
        try:
            ws_f = float(ws) if ws is not None else None
        except (TypeError, ValueError):
            ws_f = None
        bbox = None
        try:
            # _bbox is (x0, y0, x1, y1) in PDF space sometimes
            bb = getattr(t, "_bbox", None) or getattr(t, "bbox", None)
            if bb and len(bb) == 4:
                bbox = {
                    "x0": float(bb[0]),
                    "y0": float(bb[1]),
                    "x1": float(bb[2]),
                    "y1": float(bb[3]),
                }
        except Exception:
            pass
        out.append(
            PeerTable(
                source=f"camelot_{flavor}",
                page=page,
                rows=r,
                cols=c,
                cells=cells,
                bbox=bbox,
                accuracy=acc_f,
                whitespace=ws_f,
                flavor=flavor,
            )
        )
    return out


def extract_pdfplumber(pdf: Path, page_indices: list[int]) -> list[PeerTable]:
    import pdfplumber

    out: list[PeerTable] = []
    with pdfplumber.open(pdf) as doc:
        for pi in page_indices:
            if pi < 0 or pi >= len(doc.pages):
                continue
            page = doc.pages[pi]
            tables = page.extract_tables() or []
            for ti, tab in enumerate(tables):
                if not tab:
                    continue
                cells = grid_from_df_values(tab)
                r, c = shape(cells)
                # pad
                cells = [row + [""] * (c - len(row)) for row in cells]
                out.append(
                    PeerTable(
                        source="pdfplumber",
                        page=pi,
                        rows=r,
                        cols=c,
                        cells=cells,
                        bbox=None,
                        accuracy=None,
                        flavor="plumber",
                    )
                )
    return out


def render_page_png(pdf: Path, page_1based: int, dpi: int = 150) -> Optional[Path]:
    """Render one page with pdftoppm; return PNG path or None."""
    if not shutil_which("pdftoppm"):
        return None
    td = Path(tempfile.mkdtemp(prefix="gold_render_"))
    prefix = td / f"p{page_1based}"
    try:
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
    except Exception:
        return None
    png = prefix.with_suffix(".png")
    return png if png.is_file() else None


def shutil_which(cmd: str) -> bool:
    from shutil import which

    return which(cmd) is not None


def extract_vision(pdf: Path, page_0: int, api_base: str, api_key: str, model: str) -> list[PeerTable]:
    """Optional vision extraction via OpenAI-compatible chat API (xAI / OpenAI)."""
    import urllib.request

    png = render_page_png(pdf, page_0 + 1)
    if not png:
        return []
    b64 = base64.b64encode(png.read_bytes()).decode("ascii")
    prompt = (
        "Extract ALL data tables from this PDF page image as JSON only. "
        "Return: {\"tables\": [{\"rows\": N, \"cols\": M, \"cells\": [[\"...\"]]}]}. "
        "Preserve every cell; use empty string for blanks; do not invent values. "
        "No markdown fences."
    )
    body = {
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": prompt},
                    {
                        "type": "image_url",
                        "image_url": {"url": f"data:image/png;base64,{b64}"},
                    },
                ],
            }
        ],
        "temperature": 0,
    }
    req = urllib.request.Request(
        f"{api_base.rstrip('/')}/chat/completions",
        data=json.dumps(body).encode("utf-8"),
        headers={
            "Content-Type": "application/json",
            "Authorization": f"Bearer {api_key}",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=120) as resp:
            data = json.loads(resp.read().decode("utf-8"))
        text = data["choices"][0]["message"]["content"]
        text = text.strip()
        if text.startswith("```"):
            text = re.sub(r"^```(?:json)?\n?", "", text)
            text = re.sub(r"\n?```$", "", text)
        parsed = json.loads(text)
    except Exception as e:
        return [
            PeerTable(
                source="vision",
                page=page_0,
                rows=0,
                cols=0,
                cells=[],
                notes=[f"vision_failed: {e}"],
            )
        ]
    out = []
    for t in parsed.get("tables") or []:
        cells = grid_from_df_values(t.get("cells") or [])
        r, c = shape(cells)
        cells = [row + [""] * (c - len(row)) for row in cells]
        out.append(
            PeerTable(
                source="vision",
                page=page_0,
                rows=r,
                cols=c,
                cells=cells,
                notes=["vision_llm"],
            )
        )
    return out


def score_peer_table(t: PeerTable) -> float:
    """Higher is better — used to pick primary table when shapes disagree."""
    s = 0.0
    if t.accuracy is not None:
        s += t.accuracy  # 0-100
    else:
        s += 50.0
    if t.source.startswith("camelot_lattice"):
        s += 15.0
    elif t.source.startswith("camelot_stream"):
        s += 8.0
    elif t.source == "pdfplumber":
        s += 5.0
    elif t.source == "vision":
        s += 10.0
    # prefer filled non-empty
    if t.cells:
        nonempty = sum(1 for row in t.cells for c in row if c)
        total = max(t.rows * t.cols, 1)
        s += 20.0 * (nonempty / total)
    # penalize tiny junk
    if t.rows < 2 or t.cols < 2:
        s -= 30.0
    return s


def cell_agreement(grids: list[list[list[str]]]) -> tuple[list[list[str]], list[list[float]], list[list[str]]]:
    """Majority vote on cells for equal-shape grids.
    Returns consensus grid, confidence 0-1, and dispute notes.
    """
    if not grids:
        return [], [], []
    rows = len(grids[0])
    cols = len(grids[0][0]) if rows else 0
    cons: list[list[str]] = []
    conf: list[list[float]] = []
    notes: list[list[str]] = []
    n = len(grids)
    for r in range(rows):
        crow, cf, cn = [], [], []
        for c in range(cols):
            votes: dict[str, int] = {}
            for g in grids:
                if r < len(g) and c < len(g[r]):
                    v = normalize_cell(g[r][c])
                else:
                    v = ""
                # soft normalize for vote key
                key = re.sub(r"\s+", " ", v).strip().lower()
                votes[key] = votes.get(key, 0) + 1
            best_key = max(votes, key=lambda k: (votes[k], len(k)))
            best_count = votes[best_key]
            # recover original casing from first matching peer
            best_val = ""
            for g in grids:
                if r < len(g) and c < len(g[r]):
                    v = normalize_cell(g[r][c])
                    if re.sub(r"\s+", " ", v).strip().lower() == best_key:
                        best_val = v
                        break
            crow.append(best_val)
            cf.append(best_count / n)
            if best_count < n:
                variants = [normalize_cell(g[r][c]) if r < len(g) and c < len(g[r]) else "" for g in grids]
                cn.append("dispute:" + " | ".join(repr(v[:40]) for v in variants))
            else:
                cn.append("agree")
        cons.append(crow)
        conf.append(cf)
        notes.append(cn)
    return cons, conf, notes


def _cell_key(v: str) -> str:
    return re.sub(r"\s+", " ", normalize_cell(v)).strip().lower()


def pick_primary_and_consensus(candidates: list[PeerTable]) -> dict[str, Any]:
    """Primary peer supplies cell *values*; other peers supply *confidence*.

    Never majority-vote plumber garbage over high-accuracy Camelot lattice.
    """
    if not candidates:
        return {"error": "no candidates"}
    ranked = sorted(candidates, key=score_peer_table, reverse=True)
    primary = ranked[0]
    # Prefer camelot_lattice as primary when present with decent accuracy
    for t in ranked:
        if t.source == "camelot_lattice" and (t.accuracy is None or t.accuracy >= 70):
            if t.rows == primary.rows and t.cols == primary.cols:
                primary = t
                break
            if t.rows >= 2 and t.cols >= 2 and score_peer_table(t) >= score_peer_table(primary) * 0.9:
                primary = t
                break

    same = [t for t in ranked if t.rows == primary.rows and t.cols == primary.cols]
    cons = [list(row) for row in primary.cells]
    conf: list[list[float]] = []
    notes: list[list[str]] = []
    supporters = [t for t in same if t is not primary]
    n_sup = len(supporters)
    for r in range(primary.rows):
        crow_f, crow_n = [], []
        for c in range(primary.cols):
            pv = cons[r][c] if r < len(cons) and c < len(cons[r]) else ""
            pk = _cell_key(pv)
            if n_sup == 0:
                # Single source: use accuracy as confidence proxy
                base = 0.75 if (primary.accuracy or 0) >= 95 else 0.6 if (primary.accuracy or 0) >= 85 else 0.55
                crow_f.append(base)
                crow_n.append("single_source:" + primary.source)
                continue
            agrees = 0
            variants = []
            for t in supporters:
                sv = t.cells[r][c] if r < len(t.cells) and c < len(t.cells[r]) else ""
                variants.append(f"{t.source}={normalize_cell(sv)[:40]!r}")
                if _cell_key(sv) == pk:
                    agrees += 1
            # primary always counts as 1
            ratio = (1 + agrees) / (1 + n_sup)
            crow_f.append(ratio)
            if agrees == n_sup:
                crow_n.append("agree")
            elif agrees == 0:
                crow_n.append("dispute_all:" + " | ".join(variants))
            else:
                crow_n.append("dispute_partial:" + " | ".join(variants))
        conf.append(crow_f)
        notes.append(crow_n)

    mean_conf = sum(sum(row) for row in conf) / max(primary.rows * primary.cols, 1)
    dispute_n = sum(1 for row in notes for n in row if n.startswith("dispute"))
    human_focus = []
    for r, row in enumerate(notes):
        for c, n in enumerate(row):
            if n.startswith("dispute") or conf[r][c] < 0.67:
                human_focus.append(
                    {
                        "row": r,
                        "col": c,
                        "value": cons[r][c],
                        "confidence": conf[r][c],
                        "note": n,
                    }
                )
    return {
        "primary_source": primary.source,
        "primary_accuracy": primary.accuracy,
        "supporting_sources": [primary.source] + [t.source for t in supporters],
        "page": primary.page,
        "bbox": primary.bbox,
        "rows": primary.rows,
        "cols": primary.cols,
        "cells": cons,
        "cell_confidence": conf,
        "cell_notes": notes,
        "mean_cell_confidence": round(mean_conf, 4),
        "dispute_cell_count": dispute_n,
        "human_focus": human_focus[:200],
        "value_policy": "primary_cells_authoritative; peers score confidence only",
        "peer_candidates": [
            {
                "source": t.source,
                "rows": t.rows,
                "cols": t.cols,
                "accuracy": t.accuracy,
                "score": round(score_peer_table(t), 2),
            }
            for t in ranked
        ],
    }


def page_list_spec(pages: Optional[str], n_pages: int) -> tuple[str, list[int]]:
    """Return camelot pages string and 0-based indices."""
    if not pages or pages.lower() == "all":
        return "all", list(range(n_pages))
    # e.g. "1" or "1-3"
    idxs = []
    for part in pages.split(","):
        part = part.strip()
        if "-" in part:
            a, b = part.split("-", 1)
            for p in range(int(a), int(b) + 1):
                idxs.append(p - 1)
        else:
            idxs.append(int(part) - 1)
    camelot_pages = pages
    return camelot_pages, idxs


def pdf_page_count(pdf: Path) -> int:
    try:
        import pdfplumber

        with pdfplumber.open(pdf) as doc:
            return len(doc.pages)
    except Exception:
        return 1


def build_for_pdf(
    pdf: Path,
    pages: Optional[str],
    use_vision: bool,
    shadow_pdfparser: bool,
) -> dict[str, Any]:
    n_pages = pdf_page_count(pdf)
    # Soft-gold structure drafts: page 1 only by default for multipage (human expands)
    if pages is None:
        pages = "1"
    camelot_pages, page_idxs = page_list_spec(pages, n_pages)

    peers: list[PeerTable] = []
    peers.extend(extract_camelot(pdf, camelot_pages, "lattice"))
    peers.extend(extract_camelot(pdf, camelot_pages, "stream"))
    peers.extend(extract_pdfplumber(pdf, page_idxs))

    vision_notes = []
    if use_vision:
        api_key = (
            os.environ.get("XAI_API_KEY")
            or os.environ.get("GROK_API_KEY")
            or os.environ.get("OPENAI_API_KEY")
        )
        api_base = os.environ.get("XAI_API_BASE") or os.environ.get(
            "OPENAI_API_BASE", "https://api.x.ai/v1"
        )
        model = os.environ.get("VISION_MODEL") or os.environ.get(
            "XAI_VISION_MODEL", "grok-2-vision-1212"
        )
        if api_key:
            for pi in page_idxs:
                peers.extend(
                    extract_vision(pdf, pi, api_base, api_key, model)
                )
            vision_notes.append(f"vision_attempted model={model}")
        else:
            vision_notes.append(
                "vision_skipped: set XAI_API_KEY or OPENAI_API_KEY to enable"
            )

    # Group by page
    by_page: dict[int, list[PeerTable]] = {}
    for t in peers:
        if t.rows == 0 and t.cols == 0:
            continue
        by_page.setdefault(t.page, []).append(t)

    # Lattice-first selection (Camelot lattice = primary authority for ruled pages).
    # Other peers only vote for consensus when shape matches; they do not spawn
    # extra tables unless lattice is empty/weak.
    expected_tables = []
    for page, cands in sorted(by_page.items()):
        lattice = [
            t
            for t in cands
            if t.source == "camelot_lattice"
            and t.rows >= 2
            and t.cols >= 2
            and (t.accuracy is None or t.accuracy >= 70.0)
        ]
        stream = [
            t
            for t in cands
            if t.source == "camelot_stream" and t.rows >= 2 and t.cols >= 2
        ]
        plumber = [
            t for t in cands if t.source == "pdfplumber" and t.rows >= 2 and t.cols >= 2
        ]
        vision = [t for t in cands if t.source == "vision" and t.rows >= 2 and t.cols >= 2]

        if lattice:
            primaries = sorted(lattice, key=score_peer_table, reverse=True)
        elif stream:
            # Prefer highest-accuracy stream tables; drop tiny junk
            primaries = sorted(
                [t for t in stream if t.rows >= 3 or t.cols >= 3],
                key=score_peer_table,
                reverse=True,
            )[:6]
        elif plumber:
            primaries = sorted(plumber, key=score_peer_table, reverse=True)[:4]
        elif vision:
            primaries = vision
        else:
            primaries = sorted(cands, key=score_peer_table, reverse=True)[:3]

        # Dedup primaries that are near-identical shapes (keep best score)
        kept: list[PeerTable] = []
        for p in primaries:
            dup = False
            for k in kept:
                if abs(k.rows - p.rows) <= 1 and abs(k.cols - p.cols) <= 1:
                    # same region-ish if bbox overlaps or both missing
                    if p.bbox and k.bbox:
                        # rough IoU
                        ax0, ay0, ax1, ay1 = p.bbox["x0"], p.bbox["y0"], p.bbox["x1"], p.bbox["y1"]
                        bx0, by0, bx1, by1 = k.bbox["x0"], k.bbox["y0"], k.bbox["x1"], k.bbox["y1"]
                        ix0, iy0 = max(ax0, bx0), max(ay0, by0)
                        ix1, iy1 = min(ax1, bx1), min(ay1, by1)
                        inter = max(0, ix1 - ix0) * max(0, iy1 - iy0)
                        ua = max(1e-6, (ax1 - ax0) * (ay1 - ay0) + (bx1 - bx0) * (by1 - by0) - inter)
                        if inter / ua >= 0.5:
                            dup = True
                            break
                    elif abs(k.rows - p.rows) == 0 and abs(k.cols - p.cols) == 0:
                        dup = True
                        break
            if not dup:
                kept.append(p)

        for primary in kept:
            # Consensus pool: same shape from other sources
            pool = [primary]
            for t in cands:
                if t is primary:
                    continue
                if t.rows == primary.rows and t.cols == primary.cols:
                    pool.append(t)
            cons = pick_primary_and_consensus(pool)
            cons["page"] = page
            expected_tables.append(cons)

    # Sort by page then -y bbox if present
    def sort_key(t: dict) -> tuple:
        bb = t.get("bbox") or {}
        return (t.get("page", 0), -float(bb.get("y1", 0)), float(bb.get("x0", 0)))

    expected_tables.sort(key=sort_key)

    shadow = None
    if shadow_pdfparser:
        bin_p = REPO / "target" / "release" / "pdfparser"
        if bin_p.is_file():
            try:
                proc = subprocess.run(
                    [
                        str(bin_p),
                        "extract",
                        "--tables",
                        "--no-stitch",
                        "--page-tables",
                        "--format",
                        "json",
                        str(pdf),
                    ],
                    capture_output=True,
                    text=True,
                    timeout=120,
                )
                if proc.returncode == 0:
                    payload = json.loads(proc.stdout)
                    shadow = {
                        "n_tables": len(payload.get("tables") or []),
                        "tables": [
                            {
                                "page": t.get("page"),
                                "rows": t.get("rows"),
                                "cols": t.get("cols"),
                                "method": t.get("method"),
                            }
                            for t in (payload.get("tables") or [])[:20]
                        ],
                        "note": "pdfparser shadow ONLY — not used as gold authority",
                    }
            except Exception as e:
                shadow = {"error": str(e)}

    mean_confs = [t.get("mean_cell_confidence") or 0 for t in expected_tables]
    overall = sum(mean_confs) / max(len(mean_confs), 1) if mean_confs else 0.0
    total_disputes = sum(int(t.get("dispute_cell_count") or 0) for t in expected_tables)

    doc_id = pdf.stem
    gold = {
        "id": doc_id,
        "schema_version": 1,
        "track": "structure_draft_peer",
        "status": "peer_consensus_draft",
        "policy": (
            "NEVER auto-promote without human confirmation. "
            "Authority: Camelot/pdfplumber(/vision) consensus — NOT pdfparser."
        ),
        "source": _relpath(pdf, REPO),
        "built_at": datetime.now(timezone.utc).isoformat(),
        "builder": "build_structure_gold_peers.py",
        "pages_extracted": camelot_pages,
        "expected_table_count": len(expected_tables),
        "expected_tables": [
            {
                "page": t["page"],
                "bbox": t.get("bbox"),
                "rows": t["rows"],
                "cols": t["cols"],
                "cells": t["cells"],
                "primary_source": t.get("primary_source"),
                "primary_accuracy": t.get("primary_accuracy"),
                "supporting_sources": t.get("supporting_sources"),
                "mean_cell_confidence": t.get("mean_cell_confidence"),
                "dispute_cell_count": t.get("dispute_cell_count"),
                "cell_confidence": t.get("cell_confidence"),
                "human_focus": t.get("human_focus"),
                "peer_candidates": t.get("peer_candidates"),
            }
            for t in expected_tables
        ],
        "confidence_summary": {
            "overall_mean_cell_confidence": round(overall, 4),
            "total_dispute_cells": total_disputes,
            "human_review_estimate": (
                "spot_check"
                if overall >= 0.85 and total_disputes < 20
                else "focused_disputes"
                if overall >= 0.6
                else "careful_review"
            ),
        },
        "vision_notes": vision_notes,
        "pdfparser_shadow": shadow,
        "peers_available": {
            "camelot": True,
            "pdfplumber": True,
            "vision": use_vision and bool(
                os.environ.get("XAI_API_KEY")
                or os.environ.get("OPENAI_API_KEY")
                or os.environ.get("GROK_API_KEY")
            ),
        },
    }
    return gold


def write_review_html(gold: dict, out_html: Path) -> None:
    """Simple HTML: tables with disputed cells highlighted."""
    parts = [
        "<!DOCTYPE html><html><head><meta charset='utf-8'>",
        f"<title>Peer gold review — {gold.get('id')}</title>",
        "<style>",
        "body{font-family:system-ui,sans-serif;margin:24px}",
        "table{border-collapse:collapse;margin:12px 0;font-size:12px}",
        "td,th{border:1px solid #ccc;padding:4px 6px;max-width:180px;word-wrap:break-word}",
        ".hi{background:#ffe08a}",
        ".lo{background:#ffb4b4}",
        ".ok{background:#d4edda}",
        ".meta{color:#444;font-size:13px}",
        "</style></head><body>",
        f"<h1>Peer consensus draft: {gold.get('id')}</h1>",
        f"<p class='meta'>status={gold.get('status')} | "
        f"confidence={gold.get('confidence_summary')} | "
        f"<b>NOT auto-reviewed</b></p>",
    ]
    for ti, t in enumerate(gold.get("expected_tables") or []):
        parts.append(
            f"<h2>Table {ti} page={t.get('page')} "
            f"{t.get('rows')}x{t.get('cols')} primary={t.get('primary_source')} "
            f"acc={t.get('primary_accuracy')} mean_conf={t.get('mean_cell_confidence')}</h2>"
        )
        parts.append("<table>")
        conf = t.get("cell_confidence") or []
        for r, row in enumerate(t.get("cells") or []):
            parts.append("<tr>")
            for c, val in enumerate(row):
                cf = conf[r][c] if r < len(conf) and c < len(conf[r]) else 0
                cls = "ok" if cf >= 0.99 else ("hi" if cf >= 0.67 else "lo")
                esc = (
                    str(val)
                    .replace("&", "&amp;")
                    .replace("<", "&lt;")
                    .replace(">", "&gt;")
                )
                parts.append(f"<td class='{cls}' title='conf={cf:.2f}'>{esc}</td>")
            parts.append("</tr>")
        parts.append("</table>")
        focus = t.get("human_focus") or []
        if focus:
            parts.append(f"<p class='meta'>Human focus cells: {len(focus)} (yellow/red)</p>")
    parts.append("</body></html>")
    out_html.write_text("\n".join(parts), encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser(description="Peer-consensus structure gold drafts")
    ap.add_argument("--pdf", type=Path, help="Single PDF")
    ap.add_argument("--all-soft-gold", action="store_true")
    ap.add_argument("--pages", default=None, help="1-based page range, default page 1")
    ap.add_argument("--vision", action="store_true", help="Enable vision if API key set")
    ap.add_argument(
        "--shadow-pdfparser",
        action="store_true",
        help="Include pdfparser shape shadow (not gold)",
    )
    ap.add_argument("--out-dir", type=Path, default=OUT_DIR)
    args = ap.parse_args()

    pdfs: list[Path] = []
    if args.all_soft_gold:
        for sid in SOFT_GOLD:
            p = BENCH / "corpus" / "real" / f"{sid}.pdf"
            if p.is_file():
                pdfs.append(p)
            else:
                print(f"skip missing {p}", file=sys.stderr)
    elif args.pdf:
        pdfs.append(args.pdf)
    else:
        ap.error("pass --pdf or --all-soft-gold")

    # dependency check
    try:
        import camelot  # noqa: F401
        import pdfplumber  # noqa: F401
    except ImportError as e:
        print(
            "Missing peers. Create venv and install:\n"
            "  python3 -m venv .venv && source .venv/bin/activate\n"
            "  pip install pdfplumber camelot-py opencv-python-headless\n"
            f"Error: {e}",
            file=sys.stderr,
        )
        return 2

    args.out_dir.mkdir(parents=True, exist_ok=True)
    summary = []
    for pdf in pdfs:
        print(f"building peer gold for {pdf.name} ...", flush=True)
        gold = build_for_pdf(
            pdf,
            pages=args.pages,
            use_vision=args.vision,
            shadow_pdfparser=args.shadow_pdfparser,
        )
        out = args.out_dir / f"{gold['id']}.peer.json"
        out.write_text(json.dumps(gold, indent=2, ensure_ascii=False), encoding="utf-8")
        html = args.out_dir / f"{gold['id']}.review.html"
        write_review_html(gold, html)
        cs = gold.get("confidence_summary") or {}
        print(
            f"  wrote {out.name} tables={gold.get('expected_table_count')} "
            f"mean_conf={cs.get('overall_mean_cell_confidence')} "
            f"disputes={cs.get('total_dispute_cells')} "
            f"review={cs.get('human_review_estimate')}"
        )
        summary.append(
            {
                "id": gold["id"],
                "tables": gold.get("expected_table_count"),
                "mean_conf": cs.get("overall_mean_cell_confidence"),
                "disputes": cs.get("total_dispute_cells"),
                "review": cs.get("human_review_estimate"),
                "path": str(out),
                "html": str(html),
            }
        )

    idx = args.out_dir / "INDEX.json"
    idx.write_text(
        json.dumps(
            {
                "built_at": datetime.now(timezone.utc).isoformat(),
                "policy": "peer consensus only; human confirm before real_structure",
                "documents": summary,
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    print(f"index {idx}")
    return 0


if __name__ == "__main__":
    # Path.is_relative_to polyfill-ish: py3.9+
    sys.exit(main())
