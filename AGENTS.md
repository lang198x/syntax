# lang198x-syntax

Read [PRINCIPLES.md](PRINCIPLES.md) first; [MANIFESTO.md](MANIFESTO.md) explains
why and [PRACTICE.md](PRACTICE.md) explains the shared method.

This crate is the shared lossless syntax tree that every Lang198x front end
builds on. Its design is binding and lives in Lang198x's
[shared syntax layer decision](https://github.com/lang198x/lang198x/blob/main/decisions/shared-syntax-layer.md).
Read its *Drift triggers* before changing anything here.

## What belongs here

Shared building blocks only: source text and positions, the tree and its
builder, navigation, provenance tables, diagnostics and test helpers.
Grammars, typed views, profiles and meaning belong to each language family's
own repository.

## Rules that are easy to break

- **No runtime dependencies.** Not even small ones. Test tools follow the
  approvals recorded in the decision.
- **Nothing outside the crate sees how the tree is stored.** The public API
  uses this crate's own types, so the storage can change without touching any
  language.
- **No input can cause a panic.** Library code uses no `unwrap`, and errors are
  values. `unsafe` is forbidden.
- **The tree never changes once built,** and parsing never fails: broken source
  becomes `ERROR` nodes plus diagnostics.
- **Provenance lives in side tables,** never in the tree.
- **Add an accessor only when a front end needs it.**
- **Documentation is plain English, at about Flesch-Kincaid grade 10.** Every
  public item is documented (`missing_docs` is denied), and examples run as
  tests.

## Working here

- Rust 1.99.1, pinned in `rust-toolchain.toml`. In sessions that set
  `RUSTUP_TOOLCHAIN`, run cargo with `env -u RUSTUP_TOOLCHAIN` so the pin
  applies.
- Before pushing: `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo llvm-cov --workspace`. CI fails below 95% line coverage.
- Land work through pull requests, rebase-merged. Every commit subject and PR
  title is a conventional commit.
- Plans live in [`docs/plans/`](docs/plans/).
