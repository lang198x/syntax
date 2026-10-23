# lang198x-syntax

[![CI](https://github.com/lang198x/syntax/actions/workflows/ci.yml/badge.svg)](https://github.com/lang198x/syntax/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/lang198x-syntax.svg)](https://crates.io/crates/lang198x-syntax)

The shared lossless syntax tree for [Lang198x](https://github.com/lang198x)
language front ends.

Lang198x builds parsers and language tools for vintage languages: assemblers,
BASICs, and others such as C, Pascal and Forth. Every front end parses source
into the same kind of tree. The tree is *lossless*: joining the text of its
tokens gives back the source exactly, byte for byte, including whitespace,
comments and original spelling. So one parse can serve an assembler, a
formatter and an editor.

This crate holds the shared parts only. Each language keeps its own grammar
and meaning. The [shared syntax layer decision](https://github.com/lang198x/lang198x/blob/main/decisions/shared-syntax-layer.md)
records the design and the reasons for it.

## Status

Early. Text positions and ranges are in place, and the tree itself is being
built, following the [tree core plan](docs/plans/2026-10-23-tree-core.md).

## Design in brief

- **No runtime dependencies,** and no `unsafe` code.
- **A flat, immutable tree.** Nodes and tokens sit in one array in source
  order, and the source text is stored once.
- **Parsing never fails.** Broken source becomes error nodes, with diagnostics
  kept beside the tree.
- **Provenance in side tables.** Every token produced by a macro, an include or
  a decoded native file can be traced back to the text a person wrote.

## Licence

GPL-2.0-or-later. See [LICENSE](LICENSE).
