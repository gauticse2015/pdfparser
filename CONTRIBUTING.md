# Contributing to pdfparser

Thanks for your interest in contributing.

## Development setup

1. Install Rust via [rustup](https://rustup.rs/) (stable toolchain).
2. Clone the repository and build:

```bash
cargo build --workspace
cargo test --workspace
```

3. Optional: set up the Python benchmark environment (see README).

## Project conventions

- **Library-first**: public API lives in `crates/pdfparser`. Prefer embedding-friendly APIs over CLI-only features.
- **No fixture hardcoding**: fixes for PDF behavior must be generic (spec/encodings/layout), not special cases for a single corpus file.
- **No ICDAR in regression**: never copy ICDAR-2013 competition PDFs or gold into `benchmark/corpus`, `benchmark/real_track`, or `benchmark/ground_truth`. Do not tune thresholds on ICDAR docs. Optional external competitive scripts only. Enforced by `benchmark/scripts/assert_no_icdar.py`.
- **Security**: do not weaken the resource governor or encryption gate without an explicit design decision and tests.
- **Table engine**: product Auto/Full use **Engine V2 exclusive AutoRouter** (`docs/design-table-engine-v2.md`). Rollback: `TableOptions.legacy_router = true`. Do not tune thresholds on ICDAR.

## Code style

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

## Commit messages

Use clear, imperative subjects, for example:

- `feat: parse font Differences encodings`
- `fix: preserve multi-column reading order`
- `docs: expand README quick start`
- `test: add Phase T corpus gates`

## Pull requests

1. Keep PRs focused and reviewable.
2. Include tests for behavioral changes.
3. Update docs when public API or user-facing behavior changes.
4. If accuracy metrics change, re-run the scoreboard and note results in the PR.

## Reporting issues

Please include:

- OS and `rustc --version`
- Minimal PDF sample if possible (or public URL)
- CLI/library snippet and full error message

## License

By contributing, you agree that your contributions will be dual-licensed under MIT OR Apache-2.0, the same as the project.
