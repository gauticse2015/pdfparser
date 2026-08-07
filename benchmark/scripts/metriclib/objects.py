"""pdfparser benchmark metric primitives (split from metrics.py)."""
from __future__ import annotations

from typing import Any, Optional

def _count_metrics(pred: Optional[int], exp: Optional[int]) -> dict[str, Any]:
    if exp is None:
        return {"skipped": True, "predicted": pred}
    if pred is None:
        # Library adapter has no image/object API — do not score as 0 (would bias overall).
        return {
            "skipped": True,
            "expected": exp,
            "predicted": None,
            "unavailable": True,
        }
    exact = pred == exp
    abs_error = abs(pred - exp)
    recall = min(pred, exp) / exp if exp > 0 else (1.0 if pred == 0 else 0.0)
    # score: 1 if exact else decay by relative error
    if exp == 0:
        score = 1.0 if pred == 0 else 0.0
    else:
        score = max(0.0, 1.0 - abs_error / max(exp, 1))
    return {
        "skipped": False,
        "expected": exp,
        "predicted": pred,
        "exact": exact,
        "abs_error": abs_error,
        "recall": recall,
        "score": score,
    }


def _set_metrics(pred: list[str], exp: list[str]) -> dict[str, Any]:
    if exp is None:
        return {"skipped": True}
    exp_set = {normalize_cell(x) for x in exp if x}
    pred_set = {normalize_cell(x) for x in (pred or []) if x}
    if not exp_set and not pred_set:
        return {"precision": 1.0, "recall": 1.0, "f1": 1.0, "tp": 0, "fp": 0, "fn": 0, "skipped": False}
    tp = len(exp_set & pred_set)
    fp = len(pred_set - exp_set)
    fn = len(exp_set - pred_set)
    prec = tp / (tp + fp) if (tp + fp) else 0.0
    rec = tp / (tp + fn) if (tp + fn) else 0.0
    f1 = (2 * prec * rec / (prec + rec)) if (prec + rec) else 0.0
    return {
        "skipped": False,
        "precision": prec,
        "recall": rec,
        "f1": f1,
        "tp": tp,
        "fp": fp,
        "fn": fn,
        "expected": sorted(exp_set),
        "predicted": sorted(pred_set),
    }


def objects_accuracy(
    *,
    image_count: Optional[int],
    links: list,
    form_fields: list,
    outline: list,
    gold: dict,
) -> dict[str, Any]:
    images = _count_metrics(image_count, gold.get("expected_images"))
    # links: gold may be list of URI substrings or full URIs
    exp_links = gold.get("expected_links")
    if exp_links is None and gold.get("expected_link_uri_contains"):
        exp_links = [gold["expected_link_uri_contains"]]
    # soft link match: pred hits if any pred contains gold or equals
    link_metrics = {"skipped": True}
    if exp_links is not None:
        pred_l = [str(x) for x in (links or [])]
        tp = 0
        missing = []
        for g in exp_links:
            if any(g in p or p in g for p in pred_l):
                tp += 1
            else:
                missing.append(g)
        rec = tp / len(exp_links) if exp_links else 1.0
        # precision soft: if we have preds and all gold found, 1.0; else tp/len(pred)
        prec = 1.0 if not pred_l and not exp_links else (tp / max(len(pred_l), 1) if pred_l else 0.0)
        # better: precision = 1 if every pred matches some gold loosely when gold exists
        if exp_links and pred_l:
            matched_preds = sum(1 for p in pred_l if any(g in p or p in g for g in exp_links))
            prec = matched_preds / len(pred_l)
        f1 = (2 * prec * rec / (prec + rec)) if (prec + rec) else 0.0
        link_metrics = {
            "skipped": False,
            "precision": prec,
            "recall": rec,
            "f1": f1,
            "missing": missing,
            "predicted": pred_l,
            "expected": exp_links,
        }

    exp_forms = gold.get("expected_form_fields") or gold.get("expected_form_field_names")
    # forms: extract names before '='
    pred_form_names = []
    for f in form_fields or []:
        s = str(f)
        pred_form_names.append(s.split("=")[0].strip())
    forms = _set_metrics(pred_form_names, exp_forms) if exp_forms is not None else {"skipped": True}

    exp_outline = gold.get("expected_outline_titles")
    outline_m = _set_metrics(outline or [], exp_outline) if exp_outline is not None else {"skipped": True}

    parts, weights = [], []
    if not images.get("skipped") and images.get("score") is not None:
        parts.append(images["score"])
        weights.append(1.0)
    if not link_metrics.get("skipped") and link_metrics.get("f1") is not None:
        parts.append(link_metrics["f1"])
        weights.append(1.0)
    if not forms.get("skipped") and forms.get("f1") is not None:
        parts.append(forms["f1"])
        weights.append(1.0)
    if not outline_m.get("skipped") and outline_m.get("f1") is not None:
        parts.append(outline_m["f1"])
        weights.append(1.0)
    score = 100.0 * sum(p * w for p, w in zip(parts, weights)) / sum(weights) if parts else None

    return {
        "images": images,
        "links": link_metrics,
        "forms": forms,
        "outline": outline_m,
        "score_0_100": score,
    }


def overall_accuracy(text_m: dict, table_m: dict, obj_m: dict, gold: dict) -> dict[str, Any]:
    """Weighted overall 0-100 for scoreboard."""
    # weights by what's gold-available and category emphasis
    tw = gold.get("weight_text", 0.40)
    tbw = gold.get("weight_tables", 0.40)
    ow = gold.get("weight_objects", 0.20)

    scores = []
    weights = []
    if text_m.get("score_0_100") is not None:
        scores.append(text_m["score_0_100"])
        weights.append(tw)
    if table_m.get("score_0_100") is not None:
        scores.append(table_m["score_0_100"])
        weights.append(tbw)
    if obj_m.get("score_0_100") is not None:
        scores.append(obj_m["score_0_100"])
        weights.append(ow)

    if not scores:
        return {"score_0_100": None, "components_used": []}
    overall = sum(s * w for s, w in zip(scores, weights)) / sum(weights)
    return {
        "score_0_100": overall,
        "components_used": {
            "text": text_m.get("score_0_100"),
            "tables": table_m.get("score_0_100"),
            "objects": obj_m.get("score_0_100"),
        },
        "weights": {"text": tw, "tables": tbw, "objects": ow},
    }


# ─────────────────────────── IoU table matching (real track) ─────────────────
