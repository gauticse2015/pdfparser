# Structure gold build policy (normative)

## Authority order (hard docs)

1. **Vision extract** (in-session Grok multimodal on `pdftoppm` PNG) → cell grid  
2. **Peer validate** (Camelot lattice/stream, pdfplumber) → agree / dispute  
3. **Human** spot-check → `status=reviewed`  

**Forbidden:** pdfparser extract as gold values.  
**Forbidden:** offering peer-only garbage as “focused_disputes” for human full rewrite.

## When peer-only is OK
Clean ruled lattices where Camelot accuracy ≥95% **and** vision spot-check of header + 2 data rows passes (e.g. fuel table). Still label builder honestly.

## Scripts
- `prepare_vision_gold_pages.py` — render PNGs  
- `build_structure_gold_peers.py` — peer draft only (validation / secondary)  
- Agent vision in chat — primary extract for hard pages  

# Peer-built structure gold (authoritative process)

## Problem

Gold generated from **pdfparser extract** is circular (self-approval) and produced
wrong grids (column shifts, merged cells, skipped rows). That path is **forbidden**
for `real_structure` promotion.

## Authority stack

| Rank | Source | Role |
|-----:|--------|------|
| 1 | **Camelot lattice** | Primary cell *values* for ruled PDFs |
| 2 | **Camelot stream** | Primary when lattice empty/weak |
| 3 | **pdfplumber** | Confidence vote only (same shape) |
| 4 | **In-session Grok vision** (preferred when agent is active) | Render PNG → agent `read_file` (multimodal) — **no API key** |
| 5 | **Vision HTTP API** (optional, headless/CI only) | `XAI_API_KEY` / `OPENAI_API_KEY` if no interactive agent |
| — | **pdfparser** | Shadow only — never gold authority |

**Value policy:** primary peer / vision supplies cells; Camelot cross-check; humans spot-check disputes.

### Why “extra API key” appeared

The script’s `--vision` flag calls an **HTTP** chat API so gold can be built in **CI without a chat agent**.  
In a **live Grok Build / Grok session**, that is unnecessary: the agent already has multimodal vision when it reads a PNG via tools. Use the in-session path below.

## Pipeline A — In-session Grok vision (this chat, no key)

```bash
# 1) Render pages
python3 benchmark/scripts/prepare_vision_gold_pages.py --all-soft-gold
# → benchmark/real_track/gold/drafts/vision_renders/*_p1.png

# 2) Agent reads PNGs with vision (read_file), writes vision gold drafts
# 3) Cross-check Camelot:
source .venv/bin/activate
python3 benchmark/scripts/build_structure_gold_peers.py --all-soft-gold
# 4) Human spot-check HTML / disputes only
```

Ask the agent: *“Vision-gold doc 30/35/36 from renders + Camelot validate.”*

## Pipeline B — Peer script only (no vision)

```bash
python3 -m venv .venv && source .venv/bin/activate
pip install pdfplumber camelot-py opencv-python-headless
# brew install ghostscript poppler  # recommended

python3 benchmark/scripts/build_structure_gold_peers.py --all-soft-gold
```

Outputs: `benchmark/real_track/gold/drafts/peer/<id>.peer.json` + `.review.html`

## Pipeline C — Headless vision API (only if no agent)

```bash
export XAI_API_KEY=...   # only for non-interactive CI
python3 benchmark/scripts/build_structure_gold_peers.py --pdf path.pdf --vision
```

## Human review

1. Open HTML next to PDF
2. Spot-check green; edit yellow/red disputes only
3. Promote to `gold/<id>.json` with `status=reviewed`
4. Agent updates `real_structure_v0.json`

## Quarantine

Self-golds: `gold/quarantine/` (`status=quarantined_self_gold`).
