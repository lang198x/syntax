//! Problems found in source, kept beside the tree.
//!
//! Parsing never fails. Broken source still becomes a tree, with `ERROR`
//! nodes where the parser could not make sense of it, and a list of
//! diagnostics says what went wrong and where. The list is kept apart from the
//! tree, so the tree stays a faithful copy of the source.

use std::fmt;

use crate::{SyntaxTree, TextRange};

/// What kind of problem a diagnostic reports.
///
/// The three kinds tell a person different things. *Invalid* source is wrong
/// and needs fixing. *Unsupported* source is fine, but this front end cannot
/// handle it yet. *Unknown* source cannot be judged from the text alone, which
/// is not a fault in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DiagnosticKind {
    /// The original tool would reject this source.
    Invalid,
    /// The original tool accepts this source, but this front end does not
    /// handle it yet.
    Unsupported,
    /// The meaning cannot be decided from the source as written. A computed
    /// `GOTO` target is one example, and a region that only macro expansion
    /// would explain is another. This is not an error.
    Unknown,
}

/// A problem found in the source, pointing at the text it concerns.
///
/// ```
/// use lang198x_syntax::{Diagnostic, DiagnosticKind, TextRange, TextSize};
///
/// let range = TextRange::new(TextSize::new(0), TextSize::new(4))
///     .expect("the end is not before the start");
/// let diagnostic = Diagnostic::new(DiagnosticKind::Unsupported, range, "`.foo` is not supported yet");
/// assert_eq!(diagnostic.kind(), DiagnosticKind::Unsupported);
/// assert_eq!(diagnostic.range(), range);
/// assert_eq!(diagnostic.message(), "`.foo` is not supported yet");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Diagnostic {
    kind: DiagnosticKind,
    range: TextRange,
    message: String,
}

impl Diagnostic {
    /// Makes a diagnostic of the given kind, about the text in `range`.
    ///
    /// An empty range points at a position, such as where a missing operand
    /// should have been.
    #[must_use]
    pub fn new(kind: DiagnosticKind, range: TextRange, message: impl Into<String>) -> Self {
        Self {
            kind,
            range,
            message: message.into(),
        }
    }

    /// Returns what kind of problem this is.
    #[must_use]
    pub fn kind(&self) -> DiagnosticKind {
        self.kind
    }

    /// Returns the span of source the diagnostic concerns.
    #[must_use]
    pub fn range(&self) -> TextRange {
        self.range
    }

    /// Returns the message for a person to read.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// The result of parsing: a tree and the diagnostics found while building it.
///
/// Every diagnostic's range lies inside the tree's text, on character
/// boundaries, so it can always be shown against the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parse {
    tree: SyntaxTree,
    diagnostics: Vec<Diagnostic>,
}

/// A diagnostic whose range does not fit the tree's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticOutsideText {
    /// Where the diagnostic sits in the list.
    pub index: usize,
    /// Its range.
    pub range: TextRange,
}

impl fmt::Display for DiagnosticOutsideText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "diagnostic {} covers {}, which is not a span of the source text",
            self.index, self.range
        )
    }
}

impl std::error::Error for DiagnosticOutsideText {}

impl Parse {
    /// Puts a tree and its diagnostics together, keeping the diagnostics in
    /// the order given.
    ///
    /// # Errors
    ///
    /// Returns the first diagnostic whose range runs past the end of the text
    /// or starts or ends partway through a character.
    pub fn new(
        tree: SyntaxTree,
        diagnostics: Vec<Diagnostic>,
    ) -> Result<Self, DiagnosticOutsideText> {
        if let Some((index, diagnostic)) = diagnostics
            .iter()
            .enumerate()
            .find(|(_, diagnostic)| diagnostic.range.slice(tree.text()).is_none())
        {
            return Err(DiagnosticOutsideText {
                index,
                range: diagnostic.range,
            });
        }
        Ok(Self { tree, diagnostics })
    }

    /// Returns the tree.
    #[must_use]
    pub fn tree(&self) -> &SyntaxTree {
        &self.tree
    }

    /// Returns the diagnostics, in the order they were given.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Takes the tree and diagnostics apart.
    #[must_use]
    pub fn into_parts(self) -> (SyntaxTree, Vec<Diagnostic>) {
        (self.tree, self.diagnostics)
    }
}

#[cfg(test)]
mod tests;
