# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Phase U lattice table extraction (`pdfparser-tables`, S2 + R9 cell assign)
- CLI `--tables` flag; JSON table export
- Phase U corpus tests (`phase_u_tables`)

### Planned

- Stream / hybrid table strategies (Phase V)
- Multi-page table stitch, form FP control
- Optional encryption (user-password) subset
- Forms, annotations, outline extraction

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
