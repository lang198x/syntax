# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2](https://github.com/lang198x/syntax/compare/lang198x-syntax-v0.1.1...lang198x-syntax-v0.1.2) - 2026-10-23

### Added

- trace expanded text back to the file a person wrote
- record where each piece of expanded text came from

## [0.1.1](https://github.com/lang198x/syntax/compare/lang198x-syntax-v0.1.0...lang198x-syntax-v0.1.1) - 2026-10-23

The tree itself arrives. A parser builds a lossless syntax tree through
`TreeBuilder`, and front ends move around it with `SyntaxNode` and
`SyntaxToken` handles, which are cheap to clone and carry no lifetime, so they
can be stored and sent between threads. Diagnostics sit beside the tree, and
trees dump to text for snapshot tests. The crate still has no runtime
dependencies.

### Added

- `SyntaxKind`, the label on every node and token. Each language numbers its
  own kinds above a small reserved range for the shared ones: whitespace,
  newline, comment, error and verbatim.
- `TreeBuilder`, `Checkpoint` and `SyntaxTree`, to build a tree from a
  parser's calls. Misuse never panics: `finish` returns the first mistake as a
  `BuildError`, and a checkpoint that is no longer valid is refused.
- `SyntaxNode`, `SyntaxToken`, `SyntaxElement` and `TokenAtOffset`, for
  parents, children, siblings, ancestors, descendants, `token_at_offset` and
  `covering_element`.
- `Diagnostic`, `DiagnosticKind` and `Parse`. Problems are *invalid*,
  *unsupported* or *unknown*, and every diagnostic's range must fit the text.
- `SyntaxNode::dump` and `check_snapshot`, for one-line-per-element text dumps
  compared with `.tree` files. `LANG198X_BLESS=1` rewrites the files instead.

Property tests and two fuzz targets check that every tree keeps its rules: the
tokens join back into the source, and every node, token and offset lookup
agrees with every other.

## [0.1.0](https://github.com/lang198x/syntax/releases/tag/lang198x-syntax-v0.1.0) - 2026-10-23

The first release. It provides `TextSize` and `TextRange`, the byte offsets and
spans that every token and node in the tree will be measured in. Arithmetic on
them is checked, and slicing text with a range never panics.
