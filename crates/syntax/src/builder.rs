//! Building a syntax tree, one node and token at a time.
//!
//! A parser walks the source from start to end. It opens a node with
//! [`TreeBuilder::start_node`], adds tokens with [`TreeBuilder::token`], and
//! closes the node with [`TreeBuilder::finish_node`]. Each token takes the next
//! bytes of the text, so the tokens cover the text in order by construction.
//!
//! Misuse never panics. The builder remembers the first mistake, ignores every
//! call after it, and [`TreeBuilder::finish`] returns it as a [`BuildError`].
//! A parser can therefore make its calls without checking each one.

use std::fmt;

use crate::tree::{Entry, SyntaxTree};
use crate::{SyntaxKind, TextSize};

/// Builds a [`SyntaxTree`] from a parser's calls.
///
/// A tree has exactly one root node, and every token sits inside a node.
///
/// ```
/// use lang198x_syntax::{SyntaxKind, TextSize, TreeBuilder};
///
/// const fn kind(raw: u16) -> SyntaxKind {
///     match SyntaxKind::language(raw) {
///         Some(kind) => kind,
///         None => panic!("language kinds start at SyntaxKind::FIRST_LANGUAGE"),
///     }
/// }
/// const LINE: SyntaxKind = kind(SyntaxKind::FIRST_LANGUAGE);
/// const MNEMONIC: SyntaxKind = kind(SyntaxKind::FIRST_LANGUAGE + 1);
///
/// let mut builder = TreeBuilder::new("nop\n");
/// builder.start_node(LINE);
/// builder.token(MNEMONIC, TextSize::new(3));
/// builder.token(SyntaxKind::NEWLINE, TextSize::new(1));
/// builder.finish_node();
/// let tree = builder.finish().expect("one root node covers the text");
/// assert_eq!(tree.text(), "nop\n");
/// ```
pub struct TreeBuilder<'t> {
    text: &'t str,
    text_len: TextSize,
    /// How much of the text the tokens so far have taken.
    pos: TextSize,
    entries: Vec<Entry>,
    /// The nodes still open, innermost last.
    open: Vec<u32>,
    error: Option<BuildError>,
}

/// A saved place in the tree, for wrapping what follows it in a new node.
///
/// A hand-written parser often only knows a node is needed after it has parsed
/// the node's first part. In `1+2`, the parser reads `1` before it sees `+` and
/// knows it has a sum. It takes a checkpoint before `1`, and on seeing `+`
/// calls [`TreeBuilder::start_node_at`] to open the sum there.
///
/// A checkpoint stays usable while the node that was open when it was taken
/// is still the innermost open node, and until something before the
/// checkpoint is wrapped in a new node. Wrapping moves what followed inside
/// the wrapper, so the saved place no longer exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checkpoint {
    index: u32,
    parent: Option<u32>,
    depth: usize,
}

/// A mistake in the calls made to a [`TreeBuilder`].
///
/// Offsets say how far through the text the builder had got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BuildError {
    /// The text is longer than 4 GiB, the most a 32-bit offset can address.
    TextTooLong,
    /// The tree would hold more than about four billion nodes and tokens.
    TooManyElements,
    /// A token was added with no node open to hold it.
    TokenOutsideNode {
        /// Where the token would have started.
        offset: TextSize,
    },
    /// A token would run past the end of the text.
    TokenPastEnd {
        /// Where the token would have started.
        offset: TextSize,
    },
    /// A token would end partway through a character.
    TokenSplitsCharacter {
        /// Where the token would have ended.
        offset: TextSize,
    },
    /// `finish_node` was called with no node open.
    ExtraFinishNode {
        /// Where the builder had got to.
        offset: TextSize,
    },
    /// A second root node was started after the first one closed.
    MultipleRoots {
        /// Where the second root would have started.
        offset: TextSize,
    },
    /// A checkpoint was used after the node it relied on had closed, after
    /// something before it was wrapped in a new node, or it came from a
    /// different builder.
    StaleCheckpoint {
        /// Where the builder had got to.
        offset: TextSize,
    },
    /// `finish` was called with nodes still open.
    UnclosedNodes {
        /// How many nodes were still open.
        count: usize,
    },
    /// `finish` was called before any node was started.
    NoRoot,
    /// `finish` was called before the tokens had covered the whole text.
    TextNotCovered {
        /// Where the tokens stopped.
        offset: TextSize,
    },
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TextTooLong => f.write_str("the text is longer than 4 GiB"),
            Self::TooManyElements => f.write_str("the tree has too many nodes and tokens"),
            Self::TokenOutsideNode { offset } => {
                write!(f, "a token at offset {offset} has no node to hold it")
            }
            Self::TokenPastEnd { offset } => {
                write!(
                    f,
                    "a token at offset {offset} runs past the end of the text"
                )
            }
            Self::TokenSplitsCharacter { offset } => {
                write!(
                    f,
                    "a token ends at offset {offset}, partway through a character"
                )
            }
            Self::ExtraFinishNode { offset } => {
                write!(f, "a node was finished at offset {offset} with none open")
            }
            Self::MultipleRoots { offset } => {
                write!(f, "a second root node was started at offset {offset}")
            }
            Self::StaleCheckpoint { offset } => {
                write!(f, "a checkpoint used at offset {offset} is no longer valid")
            }
            Self::UnclosedNodes { count } => write!(f, "{count} node(s) were never finished"),
            Self::NoRoot => f.write_str("the tree has no root node"),
            Self::TextNotCovered { offset } => {
                write!(
                    f,
                    "the tokens stop at offset {offset}, before the end of the text"
                )
            }
        }
    }
}

impl std::error::Error for BuildError {}

impl fmt::Debug for TreeBuilder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Shows progress, not the storage, which stays private.
        f.debug_struct("TreeBuilder")
            .field("offset", &self.pos)
            .field("open_nodes", &self.open.len())
            .field("error", &self.error)
            .finish()
    }
}

impl<'t> TreeBuilder<'t> {
    /// Starts building a tree for `text`.
    ///
    /// Text longer than 4 GiB cannot be built into a tree. The builder still
    /// accepts calls, and [`TreeBuilder::finish`] reports
    /// [`BuildError::TextTooLong`].
    #[must_use]
    pub fn new(text: &'t str) -> Self {
        let (text_len, error) = match TextSize::of(text) {
            Some(len) => (len, None),
            None => (TextSize::ZERO, Some(BuildError::TextTooLong)),
        };
        Self {
            text,
            text_len,
            pos: TextSize::ZERO,
            entries: Vec::new(),
            open: Vec::new(),
            error,
        }
    }

    /// Opens a node of the given kind. Everything added until the matching
    /// [`TreeBuilder::finish_node`] goes inside it.
    pub fn start_node(&mut self, kind: SyntaxKind) {
        if self.error.is_some() {
            return;
        }
        if self.open.is_empty() && !self.entries.is_empty() {
            return self.fail(BuildError::MultipleRoots { offset: self.pos });
        }
        let Some(index) = self.next_index() else {
            return;
        };
        self.entries.push(Entry {
            kind,
            is_token: false,
            start: self.pos,
            len: TextSize::ZERO,
            parent: self.open.last().copied(),
            subtree: 0,
        });
        self.open.push(index);
    }

    /// Adds a token of the given kind, taking the next `len` bytes of the
    /// text.
    ///
    /// A token can be empty, which marks a position without taking any text.
    pub fn token(&mut self, kind: SyntaxKind, len: TextSize) {
        if self.error.is_some() {
            return;
        }
        let Some(&parent) = self.open.last() else {
            return self.fail(BuildError::TokenOutsideNode { offset: self.pos });
        };
        let end = match self.pos.checked_add(len) {
            Some(end) if end <= self.text_len => end,
            _ => return self.fail(BuildError::TokenPastEnd { offset: self.pos }),
        };
        if !self.text.is_char_boundary(end.to_usize()) {
            return self.fail(BuildError::TokenSplitsCharacter { offset: end });
        }
        if self.next_index().is_none() {
            return;
        }
        self.entries.push(Entry {
            kind,
            is_token: true,
            start: self.pos,
            len,
            parent: Some(parent),
            subtree: 1,
        });
        self.pos = end;
    }

    /// Closes the innermost open node.
    pub fn finish_node(&mut self) {
        if self.error.is_some() {
            return;
        }
        let Some(index) = self.open.pop() else {
            return self.fail(BuildError::ExtraFinishNode { offset: self.pos });
        };
        let count = self.entries.len();
        let pos = self.pos;
        if let Some(node) = self.entries.get_mut(index as usize) {
            // Both always succeed: the node started at or before `pos`, and
            // its subtree holds fewer entries than the whole tree.
            node.len = pos.checked_sub(node.start).unwrap_or(TextSize::ZERO);
            node.subtree = u32::try_from(count - index as usize).unwrap_or(u32::MAX);
        }
    }

    /// Saves the current place, to wrap what follows in a node later with
    /// [`TreeBuilder::start_node_at`].
    #[must_use]
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            // The builder never holds more than u32::MAX entries.
            index: u32::try_from(self.entries.len()).unwrap_or(u32::MAX),
            parent: self.open.last().copied(),
            depth: self.open.len(),
        }
    }

    /// Opens a node at a saved checkpoint. Everything added since the
    /// checkpoint moves inside the new node, which stays open like one from
    /// [`TreeBuilder::start_node`].
    pub fn start_node_at(&mut self, checkpoint: Checkpoint, kind: SyntaxKind) {
        if self.error.is_some() {
            return;
        }
        let at = checkpoint.index as usize;
        // The checkpoint must still mark a boundary between children of the
        // innermost open node: either nothing has been added since, or the
        // entry there is one of that node's children. Wrapping something in
        // front of the checkpoint moves its entries inside the wrapper, which
        // breaks that. A checkpoint from another builder must not be able to
        // break the tree either.
        if checkpoint.depth != self.open.len()
            || checkpoint.parent != self.open.last().copied()
            || checkpoint
                .parent
                .is_some_and(|parent| parent >= checkpoint.index)
            || self
                .entries
                .get(at)
                .map_or(at != self.entries.len(), |entry| {
                    entry.parent != checkpoint.parent
                })
        {
            return self.fail(BuildError::StaleCheckpoint { offset: self.pos });
        }
        // At the top level, only a node wrapping the whole tree keeps one root.
        if checkpoint.parent.is_none() && at != 0 {
            return self.fail(BuildError::MultipleRoots { offset: self.pos });
        }
        if self.next_index().is_none() {
            return;
        }
        let start = self.entries.get(at).map_or(self.pos, |entry| entry.start);
        self.entries.insert(
            at,
            Entry {
                kind,
                is_token: false,
                start,
                len: TextSize::ZERO,
                parent: checkpoint.parent,
                subtree: 0,
            },
        );
        // Everything after the new node moved along one place. Entries that
        // were children of the checkpoint's parent now belong to the new node.
        for entry in &mut self.entries[at + 1..] {
            entry.parent = match entry.parent {
                Some(parent) if parent >= checkpoint.index => Some(parent + 1),
                parent if parent == checkpoint.parent => Some(checkpoint.index),
                parent => parent,
            };
        }
        self.open.push(checkpoint.index);
    }

    /// Finishes building and returns the tree, or the first mistake made.
    ///
    /// # Errors
    ///
    /// Returns the first misuse recorded by an earlier call. Otherwise it
    /// returns an error if nodes are still open, if no node was ever started,
    /// or if the tokens do not reach the end of the text.
    pub fn finish(self) -> Result<SyntaxTree, BuildError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if !self.open.is_empty() {
            return Err(BuildError::UnclosedNodes {
                count: self.open.len(),
            });
        }
        if self.entries.is_empty() {
            return Err(BuildError::NoRoot);
        }
        if self.pos != self.text_len {
            return Err(BuildError::TextNotCovered { offset: self.pos });
        }
        Ok(SyntaxTree::from_parts(self.text, self.entries))
    }

    /// Returns the index the next entry will take, or records an error if the
    /// tree is full.
    fn next_index(&mut self) -> Option<u32> {
        // u32::MAX itself is never used, so subtree sizes always fit.
        match u32::try_from(self.entries.len()) {
            Ok(index) if index < u32::MAX => Some(index),
            _ => {
                self.fail(BuildError::TooManyElements);
                None
            }
        }
    }

    fn fail(&mut self, error: BuildError) {
        self.error.get_or_insert(error);
    }
}

#[cfg(test)]
mod tests;
