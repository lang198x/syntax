//! The shared lossless syntax tree for Lang198x language front ends.
//!
//! Every Lang198x front end, whether for an assembler, a BASIC or another
//! language, parses source into the same kind of tree. The tree is
//! *lossless*: joining the text of its tokens gives back the source exactly,
//! byte for byte, including whitespace, comments and original spelling. That
//! is what lets one parse serve an assembler, a formatter and an editor alike.
//!
//! This crate holds the shared building blocks only. Each language keeps its
//! own grammar, typed views and meaning. The design, and the reasons for it,
//! are in the [shared syntax layer decision].
//!
//! The crate is at an early stage. Today it provides text positions and
//! ranges, which everything else is measured in, the kinds that label every
//! node and token, and the tree with its builder. Navigating a finished tree
//! follows.
//!
//! [shared syntax layer decision]: https://github.com/lang198x/lang198x/blob/main/decisions/shared-syntax-layer.md

mod builder;
mod kind;
mod text;
mod tree;

pub use builder::{BuildError, Checkpoint, TreeBuilder};
pub use kind::SyntaxKind;
pub use text::{TextRange, TextSize};
pub use tree::SyntaxTree;
