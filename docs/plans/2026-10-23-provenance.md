# Plan: provenance tables

**Status:** Done. All three steps landed on 2026-10-23, released in 0.1.2.
**Design:** [shared syntax layer decision](https://github.com/lang198x/lang198x/blob/main/decisions/shared-syntax-layer.md),
under *Provenance is tracked by character range*.
**First consumer:** step 4 of the
[PasmoNext 0.1.3 plan](https://github.com/lang198x/z80-asm/blob/main/docs/plans/2026-10-23-pasmonext-profile.md),
which expands macros and includes.

When a front end expands a macro or an include, it produces new text and
parses it into an *expansion tree*. A provenance table records, for each range
of that text, where it came from. Following those records back leads to the
text a person wrote, so a diagnostic inside an expansion can point at the
macro body and at the call that used it.

Each step is one pull request that passes format, Clippy, tests and the 95%
coverage floor on its own. A release follows step 2, before the Z80 expander
uses it.

## Shape

- **Texts and spans.** A `FileId` names a source file and an `ExpansionId`
  names an expansion. Both are numbers. A `TextId` is either one, and a `Span`
  is a `TextId` with a range in that text.
- **Origins.** `Origin` says where a piece of expansion text came from. It
  starts with the three kinds the Z80 family needs:
  - `Source { from }`: copied from `from`.
  - `Included { from, site }`: copied from an included file, where `site` is
    the `INCLUDE` that brought it in.
  - `Expanded { from, call_site }`: copied from a macro body, where
    `call_site` is the call being expanded.

  The enum is non-exhaustive, so `Synthesised`, `Decoded` and `Merged` can be
  added when a language needs them, without breaking anyone.
  - `Synthesised { inputs, call_site }` was added on 2026-10-27 for ca65:
    text a token function made, such as `.concat`'s, or text a front end
    supplies, such as a macro's argument count. `inputs` covers everything it
    was made from. It is not a copy, so tracing stops there, and
    `Origin::made_from` gives the inputs instead.
- **Building.** An `ExpansionBuilder` appends pieces of text one at a time,
  each with its origin, and returns the text and its table together. The table
  therefore covers the text exactly, in order, with no gaps or overlaps. A
  copied piece must be as long as the span it came from.
- **Tracing.** A `Provenance` registry holds every expansion and hands out its
  `ExpansionId`. It refuses an expansion that points at itself or at a later
  expansion, so chains always end. It can:
  - find the origin of any offset;
  - trace a range back to the text a person wrote, when the range sits inside
    one copied piece at every step;
  - list the call and include sites around a range, innermost first.

## What this is not

- It stores no file text. The front end owns its files and knows their IDs.
- It evaluates nothing. The front end decides what to expand and when.
- It does not parse. The front end parses expansion text with its own parser,
  like any other text.

## Steps

### 1. Spans, origins and the table (done)

- **Files:** `crates/syntax/src/provenance.rs`,
  `crates/syntax/src/provenance/tests.rs`.
- **What:** `FileId`, `ExpansionId`, `TextId`, `Span`, `Origin`,
  `ExpansionBuilder`, and `Expansion`, which holds the text and its table. The
  table answers `origin_at(offset)` and lists the pieces that overlap a range.
- **Verified by:** unit tests for each origin and each refusal (a piece longer
  or shorter than its span, text over 4 GiB), and a property that any sequence
  of pushes gives a table that covers the text exactly, with every lookup
  agreeing with a linear search.
- **As built:** each push makes one piece, and an empty push makes none.
  Merging neighbouring pieces that continue each other would shorten the
  table, but no front end needs that yet, and it would make `pieces` harder to
  predict. Errors are sticky, as in the tree builder. `Span` has no ordering,
  because `TextRange` has none. Three deliberately broken lookups and checks
  are each caught by the tests.

### 2. The registry and tracing (done)

- **Files:** the same module.
- **What:** `Provenance`, with `add`, `get`, `trace` and `sites`.
- **Verified by:** unit tests on nested expansions (a macro called inside a
  macro, an include inside a macro), refusals of forward and self references,
  and a property that tracing any offset in a randomly nested set of
  expansions ends in a file, at the offset the pieces imply.
- **As built:** `trace` returns `None` for a span that crosses pieces, since no
  one place wrote it; `Expansion::pieces` splits such a span first. `sites`
  follows each piece's site, or, for a plain copy, where it was copied from,
  so a copy of an expanded text keeps its calls. The property builds random
  files, then random expansions that really copy text from files and earlier
  expansions, and checks that every byte traces to the same character in a
  file. Four deliberately broken versions of `trace`, `add` and `sites` are
  each caught.

### 3. Release (done)

- **What:** release-plz publishes the new version. The changelog is written
  before the release pull request merges.
- **As built:** published as 0.1.2. The Discord announcement went out without
  its notes, as 0.1.1's had: the shared announcement script looked for
  `crates/lang198x-syntax/CHANGELOG.md`, but the crate lives in
  `crates/syntax`. The script now finds a crate by its package name
  (stevehill1981/198x-workflows#9).
