# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Network borderless tables** (textline + column alignments, gap split, same-schema re-merge, prose/list FP reject)
- **Thin-fill rule sensing** improvements (painted rect rules → lattice)
- **False-underline / overdense-H collapse** via text bands
- **Compete / compete_hard / sensing** owned regression suites + generators
- **Public real PDF fixtures** under `benchmark/corpus/compete_real/` (soft gold)
- Integrity check `benchmark/scripts/check_compete_suite.py`
- Production table pipeline docs and expanded README multi-lib comparison tables

### Changed

- Product default table path: `TablePreset::Auto` / `Full` — lattice → hybrid residual → network residual; strong lattice excludes overlapping borderless
- Removed probe/expert-registry ceremony; single production orchestration flag `exclusive_under_strong_lattice`
- Hard struggle suite (C100–C180) baseline cell F1 ~0.38 → ~0.45 after production path (still open struggle)

### Honest competitive note

- Owned multi-lib scoreboard: pdfparser leads cell F1 / overall on synthetic+hard suites
- ICDAR-2013 external: pdfparser rank #4 (F1 0.67, TEDS 0.33) vs camelot auto F1 0.86 TEDS 0.79 — raster ROI still outstanding


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
