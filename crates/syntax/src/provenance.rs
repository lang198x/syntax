//! Where expanded text came from.
//!
//! When a front end expands a macro or an include, it writes new text and
//! parses it into an expansion tree. Each piece of that text was copied from
//! somewhere: a source file, an included file, or a macro body. An
//! [`Expansion`] keeps the text together with a table of those origins, one
//! entry per piece, in order. Following the origins back leads to the text a
//! person wrote, so a diagnostic in an expansion can point at the macro body
//! and at the call that used it.
//!
//! The table is built with the text, by an [`ExpansionBuilder`], so it always
//! covers the text exactly: every byte belongs to one piece, and the pieces do
//! not overlap.
//!
//! ```
//! use lang198x_syntax::{ExpansionBuilder, FileId, Origin, Span, TextId, TextRange, TextSize};
//!
//! let main = FileId::new(0);
//! let span = |start, end| {
//!     let range = TextRange::new(TextSize::new(start), TextSize::new(end))?;
//!     Some(Span::new(TextId::File(main), range))
//! };
//! // `border 2` at 30..38 calls a macro whose body `ld a,` is at 8..13.
//! let call_site = span(30, 38).expect("a range");
//! let mut builder = ExpansionBuilder::new();
//! builder.push("ld a,", Origin::Expanded { from: span(8, 13).expect("a range"), call_site });
//! builder.push("2", Origin::Expanded { from: span(37, 38).expect("a range"), call_site });
//! let expansion = builder.finish()?;
//! assert_eq!(expansion.text(), "ld a,2");
//! let origin = expansion.origin_at(TextSize::new(5));
//! assert_eq!(origin.and_then(|o| o.copied_from()), span(37, 38));
//! # Ok::<(), lang198x_syntax::ProvenanceError>(())
//! ```

use std::fmt;

use crate::{TextRange, TextSize};

/// A source file, numbered by the front end that reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(u32);

impl FileId {
    /// Makes the ID for file number `raw`. The front end chooses the numbers.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the file's number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// An expansion, numbered in the order expansions are made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExpansionId(u32);

impl ExpansionId {
    /// Makes the ID for expansion number `raw`.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the expansion's number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A text that spans point into: a source file or an expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TextId {
    /// A source file, as a person wrote it.
    File(FileId),
    /// The text of an expansion.
    Expansion(ExpansionId),
}

/// A range in a particular text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    text: TextId,
    range: TextRange,
}

impl Span {
    /// Makes a span covering `range` in `text`.
    #[must_use]
    pub const fn new(text: TextId, range: TextRange) -> Self {
        Self { text, range }
    }

    /// Returns the text the span is in.
    #[must_use]
    pub const fn text(self) -> TextId {
        self.text
    }

    /// Returns the span's range in its text.
    #[must_use]
    pub const fn range(self) -> TextRange {
        self.range
    }
}

/// Where a piece of expansion text came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Origin {
    /// Copied from `from`, unchanged.
    Source {
        /// Where the piece was copied from.
        from: Span,
    },
    /// Copied from an included file.
    Included {
        /// Where the piece was copied from, in the included file.
        from: Span,
        /// The `INCLUDE` that brought the file in.
        site: Span,
    },
    /// Copied from a macro body, or from an argument, while expanding a call.
    Expanded {
        /// Where the piece was copied from: the body or the argument.
        from: Span,
        /// The call being expanded.
        call_site: Span,
    },
}

impl Origin {
    /// Returns where the piece was copied from, if it was copied unchanged.
    /// Every origin so far is a copy; text made up by a token function will
    /// not be.
    #[must_use]
    pub const fn copied_from(self) -> Option<Span> {
        match self {
            Self::Source { from } | Self::Included { from, .. } | Self::Expanded { from, .. } => {
                Some(from)
            }
        }
    }

    /// Returns the include or call that brought the piece in, if any.
    #[must_use]
    pub const fn site(self) -> Option<Span> {
        match self {
            Self::Source { .. } => None,
            Self::Included { site, .. } => Some(site),
            Self::Expanded { call_site, .. } => Some(call_site),
        }
    }
}

/// Why an expansion could not be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProvenanceError {
    /// A copied piece is not the same length as the span it came from.
    LengthMismatch {
        /// Where the piece would have started in the expansion.
        offset: TextSize,
        /// The piece's length in bytes.
        piece: usize,
        /// The length of the span it says it came from.
        span: TextSize,
    },
    /// The expansion's text is longer than 4 GiB.
    TextTooLong,
}

impl fmt::Display for ProvenanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthMismatch {
                offset,
                piece,
                span,
            } => write!(
                f,
                "the piece at offset {offset} is {piece} bytes long, \
                 but the span it was copied from is {span}"
            ),
            Self::TextTooLong => f.write_str("the expansion is longer than 4 GiB"),
        }
    }
}

impl std::error::Error for ProvenanceError {}

/// The text of an expansion, with where each piece of it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expansion {
    text: String,
    /// Each piece's start and origin, in order. Each piece runs to the next
    /// one's start, and the last to the end of the text. No piece is empty.
    pieces: Vec<(TextSize, Origin)>,
}

impl Expansion {
    /// Returns the expansion's text, ready to lex and parse.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the origin of the piece holding `offset`, or `None` at or past
    /// the end of the text.
    #[must_use]
    pub fn origin_at(&self, offset: TextSize) -> Option<Origin> {
        self.piece_at(offset).map(|(_, origin)| origin)
    }

    /// Returns the pieces that overlap `range`, in order, each with its range
    /// in the expansion. An empty range overlaps the piece that holds its
    /// offset.
    pub fn pieces(&self, range: TextRange) -> impl Iterator<Item = (TextRange, Origin)> + '_ {
        let first = self.index_at(range.start()).unwrap_or(self.pieces.len());
        (first..self.pieces.len())
            .map(|index| self.piece(index))
            .take_while(move |(piece, _)| {
                piece.start() < range.end() || (range.is_empty() && piece.contains(range.start()))
            })
    }

    /// The piece holding `offset`, with its range.
    fn piece_at(&self, offset: TextSize) -> Option<(TextRange, Origin)> {
        self.index_at(offset).map(|index| self.piece(index))
    }

    /// The index of the piece holding `offset`.
    fn index_at(&self, offset: TextSize) -> Option<usize> {
        if offset >= self.len() {
            return None;
        }
        let after = self.pieces.partition_point(|&(start, _)| start <= offset);
        after.checked_sub(1)
    }

    /// The piece at `index`, with its range.
    fn piece(&self, index: usize) -> (TextRange, Origin) {
        let (start, origin) = self.pieces[index];
        let end = self
            .pieces
            .get(index + 1)
            .map_or(self.len(), |&(next, _)| next);
        let range = TextRange::new(start, end).unwrap_or(TextRange::empty(start));
        (range, origin)
    }

    fn len(&self) -> TextSize {
        // The builder refuses text that does not fit, so this never falls back.
        TextSize::of(&self.text).unwrap_or(TextSize::ZERO)
    }
}

/// Builds an expansion's text and its table together, one piece at a time.
///
/// Like the tree builder, it keeps going after a mistake: the first error is
/// recorded, later pushes are ignored, and [`finish`](Self::finish) returns
/// the error.
#[derive(Debug, Clone, Default)]
pub struct ExpansionBuilder {
    text: String,
    pieces: Vec<(TextSize, Origin)>,
    error: Option<ProvenanceError>,
}

impl ExpansionBuilder {
    /// Starts an empty expansion.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends `piece`, which came from `origin`. A copied piece must be as
    /// long as the span it was copied from. An empty piece is skipped.
    pub fn push(&mut self, piece: &str, origin: Origin) {
        if self.error.is_some() || piece.is_empty() {
            return;
        }
        let Some(offset) = TextSize::of(&self.text) else {
            self.error = Some(ProvenanceError::TextTooLong);
            return;
        };
        if let Some(from) = origin.copied_from() {
            let span = from.range().len();
            if TextSize::of(piece) != Some(span) {
                self.error = Some(ProvenanceError::LengthMismatch {
                    offset,
                    piece: piece.len(),
                    span,
                });
                return;
            }
        }
        self.text.push_str(piece);
        if TextSize::of(&self.text).is_none() {
            self.error = Some(ProvenanceError::TextTooLong);
            return;
        }
        self.pieces.push((offset, origin));
    }

    /// Returns the finished expansion, or the first error.
    ///
    /// # Errors
    ///
    /// Returns the first piece that did not match its span's length, or
    /// [`ProvenanceError::TextTooLong`].
    pub fn finish(self) -> Result<Expansion, ProvenanceError> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(Expansion {
                text: self.text,
                pieces: self.pieces,
            }),
        }
    }
}

#[cfg(test)]
mod tests;
