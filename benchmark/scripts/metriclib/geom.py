"""pdfparser benchmark metric primitives (split from metrics.py)."""
from __future__ import annotations

from typing import Any, Optional

from .tables import table_detection_metrics


def rect_iou(a: dict, b: dict) -> float:
    """IoU of axis-aligned rects with keys x0,y0,x1,y1 (page space)."""
    if not a or not b:
        return 0.0
    try:
        ax0, ay0, ax1, ay1 = float(a["x0"]), float(a["y0"]), float(a["x1"]), float(a["y1"])
        bx0, by0, bx1, by1 = float(b["x0"]), float(b["y0"]), float(b["x1"]), float(b["y1"])
    except (KeyError, TypeError, ValueError):
        return 0.0
    ix0, iy0 = max(ax0, bx0), max(ay0, by0)
    ix1, iy1 = min(ax1, bx1), min(ay1, by1)
    iw, ih = max(0.0, ix1 - ix0), max(0.0, iy1 - iy0)
    inter = iw * ih
    if inter <= 0:
        return 0.0
    aa = max(0.0, ax1 - ax0) * max(0.0, ay1 - ay0)
    ba = max(0.0, bx1 - bx0) * max(0.0, by1 - by0)
    union = aa + ba - inter
    return inter / union if union > 0 else 0.0


def table_detection_metrics_iou(
    pred_tables: list,
    gold_tables: list,
    iou_thresh: float = 0.5,
) -> dict[str, Any]:
    """BBox IoU bipartite greedy matching for detection F1.

    Each table is a dict with optional `bbox` {x0,y0,x1,y1} and optional `page`.
    Tables without bbox fall back to count-based metrics only (see match_mode).

    Gold/pred may also be plain objects with .get("bbox").
    """
    if gold_tables is None:
        return {"skipped": True}

    def _bbox(t):
        if not isinstance(t, dict):
            return None
        b = t.get("bbox")
        if isinstance(b, dict) and all(k in b for k in ("x0", "y0", "x1", "y1")):
            return b
        return None

    def _page(t, default=0):
        if isinstance(t, dict) and "page" in t:
            return int(t["page"])
        return default

    g_boxes = [(i, _page(t), _bbox(t)) for i, t in enumerate(gold_tables or [])]
    p_boxes = [(i, _page(t), _bbox(t)) for i, t in enumerate(pred_tables or [])]

    if not g_boxes and not p_boxes:
        return {
            "skipped": False,
            "match_mode": "iou",
            "expected": 0,
            "predicted": 0,
            "tp": 0,
            "fp": 0,
            "fn": 0,
            "precision": 1.0,
            "recall": 1.0,
            "f1": 1.0,
            "pairs": [],
            "iou_thresh": iou_thresh,
        }

    # If any gold lacks bbox, fall back to count metrics for the whole doc.
    if any(b is None for _, _, b in g_boxes):
        n_exp = len(g_boxes)
        n_pred = len(p_boxes)
        base = table_detection_metrics(n_pred, n_exp, tol=0)
        base["match_mode"] = "count_fallback_missing_gold_bbox"
        base["iou_thresh"] = iou_thresh
        return base

    # Greedy: all (pred, gold) pairs sorted by IoU desc; same page required.
    pairs_cand = []
    for pi, pp, pb in p_boxes:
        if pb is None:
            continue
        for gi, gp, gb in g_boxes:
            if pp != gp:
                continue
            iou = rect_iou(pb, gb)
            if iou >= iou_thresh:
                pairs_cand.append((iou, pi, gi))
    pairs_cand.sort(reverse=True)

    used_p, used_g = set(), set()
    pairs = []
    for iou, pi, gi in pairs_cand:
        if pi in used_p or gi in used_g:
            continue
        used_p.add(pi)
        used_g.add(gi)
        pairs.append({"pred_i": pi, "gold_i": gi, "iou": iou})

    # Unmatched preds without bbox count as FP; unmatched gold as FN.
    n_pred = len(p_boxes)
    n_exp = len(g_boxes)
    tp = len(pairs)
    fp = n_pred - tp
    fn = n_exp - tp
    prec = tp / (tp + fp) if (tp + fp) else (1.0 if n_exp == 0 and n_pred == 0 else 0.0)
    rec = tp / (tp + fn) if (tp + fn) else (1.0 if n_exp == 0 else 0.0)
    f1 = (2 * prec * rec / (prec + rec)) if (prec + rec) else 0.0
    return {
        "skipped": False,
        "match_mode": "iou",
        "expected": n_exp,
        "predicted": n_pred,
        "tp": tp,
        "fp": fp,
        "fn": fn,
        "precision": prec,
        "recall": rec,
        "f1": f1,
        "pairs": pairs,
        "iou_thresh": iou_thresh,
    }
