"""pdfparser benchmark metric primitives (split from metrics.py)."""
from __future__ import annotations

import re
import unicodedata
from typing import Any, Optional

def normalize_text(s: str) -> str:
    if s is None:
        return ""
    s = unicodedata.normalize("NFKC", str(s))
    s = s.replace("\x00", " ")
    s = s.replace("\r", "\n")
    s = re.sub(r"[ \t]+", " ", s)
    s = re.sub(r"\n{3,}", "\n\n", s)
    return s.strip()


def normalize_cell(s: str) -> str:
    """Aggressive normalize for cell matching."""
    s = normalize_text(s)
    s = s.replace("\n", " ")
    s = re.sub(r"\s+", " ", s)
    # collapse common MuPDF underscore corruption: "TOKEN SIDE L" vs TOKEN_SIDE_L
    s = s.replace(" _", "_").replace("_ ", "_")
    s = s.lower().strip()
    # strip currency/comma noise for numeric soft match handled separately
    return s


def normalize_numeric_soft(s: str) -> str:
    s = normalize_cell(s)
    s = s.replace(",", "").replace("$", "").replace("%", "")
    return s


# ─────────────────────────── text metrics ───────────────────────────

def levenshtein(a: str, b: str) -> int:
    if a == b:
        return 0
    if not a:
        return len(b)
    if not b:
        return len(a)
    if len(a) < len(b):
        a, b = b, a
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            ins = cur[j - 1] + 1
            delete = prev[j] + 1
            sub = prev[j - 1] + (ca != cb)
            cur.append(min(ins, delete, sub))
        prev = cur
    return prev[-1]


def character_error_rate(ref: str, hyp: str) -> Optional[float]:
    ref_n = normalize_text(ref)
    hyp_n = normalize_text(hyp)
    if not ref_n:
        return None
    dist = levenshtein(ref_n, hyp_n)
    return dist / max(len(ref_n), 1)


def word_error_rate(ref: str, hyp: str) -> Optional[float]:
    ref_w = normalize_text(ref).split()
    hyp_w = normalize_text(hyp).split()
    if not ref_w:
        return None
    # classic WER via edit distance on tokens
    dist = _seq_levenshtein(ref_w, hyp_w)
    return dist / max(len(ref_w), 1)


def _seq_levenshtein(a: list, b: list) -> int:
    if a == b:
        return 0
    if not a:
        return len(b)
    if not b:
        return len(a)
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            ins = cur[j - 1] + 1
            delete = prev[j] + 1
            sub = prev[j - 1] + (ca != cb)
            cur.append(min(ins, delete, sub))
        prev = cur
    return prev[-1]


def token_set_metrics(text: str, tokens: list[str]) -> dict[str, Any]:
    """Precision/recall/F1 treating must_contain tokens as bag (each token once)."""
    if not tokens:
        return {
            "precision": None,
            "recall": None,
            "f1": None,
            "hit": 0,
            "total": 0,
            "missing": [],
            "skipped": True,
        }
    text_n = text  # substring match on raw-ish text (tokens are designed as exact)
    missing = [t for t in tokens if t not in text_n]
    hit = len(tokens) - len(missing)
    recall = hit / len(tokens)
    # must_contain is a recall protocol (gold tokens ⊂ pred text). Precision is
    # not defined without a predicted token inventory — do not mirror recall.
    precision = None
    f1 = recall  # recall-only protocol; do not invent precision
    return {
        "precision": precision,
        "recall": recall,
        "f1": f1,
        "hit": hit,
        "total": len(tokens),
        "missing": missing,
        "skipped": False,
    }


def text_accuracy(pred_text: str, gold: dict) -> dict[str, Any]:
    tokens = gold.get("must_contain") or []
    any_tokens = gold.get("must_contain_any") or []
    ref = gold.get("reference_text")

    tok = token_set_metrics(pred_text, tokens)
    any_hit = None
    any_matched = None
    if any_tokens:
        for t in any_tokens:
            if t in pred_text:
                any_hit = True
                any_matched = t
                break
        if any_hit is None:
            any_hit = False

    cer = character_error_rate(ref, pred_text) if ref else None
    wer = word_error_rate(ref, pred_text) if ref else None
    sim = (1.0 - min(cer, 1.0)) if cer is not None else None

    # composite text score 0-100
    parts = []
    weights = []
    if tok.get("f1") is not None and not tok.get("skipped"):
        parts.append(tok["f1"])
        weights.append(0.5 if ref else 1.0)
    if sim is not None:
        parts.append(sim)
        weights.append(0.5)
    if any_hit is True:
        parts.append(1.0)
        weights.append(0.15)
    elif any_hit is False:
        parts.append(0.0)
        weights.append(0.15)

    if parts and sum(weights) > 0:
        score = 100.0 * sum(p * w for p, w in zip(parts, weights)) / sum(weights)
    else:
        score = None

    return {
        "token": tok,
        "must_contain_any_hit": any_hit,
        "must_contain_any_matched": any_matched,
        "cer": cer,
        "wer": wer,
        "normalized_similarity": sim,
        "score_0_100": score,
        "has_reference_text": bool(ref),
    }

