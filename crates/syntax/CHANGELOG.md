# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.3](https://github.com/lang198x/syntax/compare/lang198x-syntax-v0.1.2...lang198x-syntax-v0.1.3) - 2026-10-27

### Added

- add a Synthesised origin for made-up text

## [0.1.2](https://github.com/lang198x/syntax/compare/lang198x-syntax-v0.1.1...lang198x-syntax-v0.1.2) - 2026-10-23

Provenance arrives. When a front end expands a macro or an include, it can
now record where each piece of the new text came from, and follow those
records back to the file a person wrote. A diagnostic inside an expansion can
then point at the macro body and at every call that led there. The crate
still has no runtime dependencies, and nothing changes for existing users.

### Added

- `FileId`, `ExpansionId`, `TextId` and `Span`, which name a range in a source
  file or in an expansion.
- `Origin`, which says where a piece of expanded text came from: `Source`
  (copied from a text), `Included` (with the `INCLUDE` that brought it in) or
  `Expanded` (from a macro body or argument, with the call). More kinds can be
  added later without a breaking change.
- `ExpansionBuilder` and `Expansion`. The builder appends pieces of text, each
  with its origin, and returns the text with a table that covers it exactly.
  `origin_at` and `pieces` look up what came from where. A copied piece must
  be as long as the span it came from.
- `Provenance`, a registry that numbers expansions as they are added. It
  refuses an expansion that points at itself or at a later one, so every chain
  ends at a file. `trace` follows a span back to the file it was written in,
  and `sites` lists the calls and includes that brought it in, innermost
  first.
- `ProvenanceError`, for each of those refusals.

A property test builds random files, then random nested expansions that copy
real text from them, and checks that every byte traces back to the same
character in a file.

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
