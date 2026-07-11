# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Phase U lattice table extraction (`pdfparser-tables`, S2 + R9 cell assign)
- CLI `--tables` flag; JSON table export
- Phase U corpus tests (`phase_u_tables`)
- Phase V multi-strategy tables: stream, hybrid, side-by-side split, multipage stitch, form FP scrub
- **Lattice multi-region (Phase A):** connected components of crossing H/V rules → multiple tables per page
- **Line prep (Phase C):** collinear coalesce + joint gap tolerance for broken corners
- **Edge/span light (Phase B):** per-cell edge presence + simple rowspan/colspan merge
- **Stream multi-region (Phase E):** split borderless tables on large vertical gaps between multi-col bands
- Hard synthetic regression suite (`corpus/hard` 50–62) + Camelot/img2table adapters in accuracy scoreboard

### Changed

- Lattice no longer snaps all page lines into one mega-grid (fixes multi-table under-detect)
- Orchestration: hybrid only outside strong lattice bboxes; NMS by quality score (conf/fill/edges)
- Hard-suite table quality: cell F1 ~0.72 → **~0.99**, shape exact **1.0** (post A–E)
- Review harden: typed `weak_edges`/`edge_score`/`fill_rate`; anchors without orthogonal endpoints;
  single joint-gap model; multi-CC-safe global fallback; lattice/stream knobs on `TableOptions`
### Planned

- Optional encryption (user-password) subset
- Stronger span-aware structure export / ICDAR competitive re-check

## [0.1.0] - 2026-07-10

### Added

- Initial workspace and library façade (`pdfparser`)
- Core open path: page tree, stream filters, resource governor
- Font encodings: WinAnsi/MacRoman/Standard, `/Differences`, ToUnicode (bfchar/bfrange)
- Content-stream text VM (`Tj` / `TJ` / text state)
- Layout: page `/Rotate` normalize, space insertion, multi-column reading order
- CLI: `pdfparser extract` (text/JSON), `pdfparser info`
- Competitive benchmark harness and accuracy scoreboard
- Design documentation under `docs/`
- Phase T integration tests on corpus fixtures

### Security

- Encrypted PDFs rejected in v0.1 (`Error::Encryption`)
- Stream expansion charged against resource limits

[Unreleased]: https://github.com/gauticse2015/pdfparser/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/gauticse2015/pdfparser/releases/tag/v0.1.0
