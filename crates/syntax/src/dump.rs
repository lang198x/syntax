//! A plain-text picture of a tree, for tests and debugging.
//!
//! Each node and token gets one line, indented two spaces per level. A line
//! shows the kind and range, and a token adds its text in quotes:
//!
//! ```text
//! LINE@0..4
//!   MNEMONIC@0..3 "nop"
//!   NEWLINE@3..4 "\n"
//! ```
//!
//! Text is quoted the way Rust writes string literals, so newlines, tabs and
//! other invisible characters show up as escapes. Every language uses this
//! one format.

use std::fmt::Write as _;

use crate::{SyntaxElement, SyntaxKind, SyntaxNode};

impl SyntaxNode {
    /// Returns a plain-text picture of this node and everything inside it.
    ///
    /// `names` gives the name of a language's own kinds. A kind it returns
    /// `None` for is shown by its shared name, such as `WHITESPACE`, or by its
    /// raw number.
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
    /// let tree = builder.finish().expect("a valid tree");
    ///
    /// let names = |kind| match kind {
    ///     LINE => Some("LINE"),
    ///     MNEMONIC => Some("MNEMONIC"),
    ///     _ => None,
    /// };
    /// assert_eq!(
    ///     tree.root().dump(names),
    ///     "LINE@0..4\n  MNEMONIC@0..3 \"nop\"\n  NEWLINE@3..4 \"\\n\"\n",
    /// );
    /// ```
    #[must_use]
    pub fn dump<'n>(&self, names: impl Fn(SyntaxKind) -> Option<&'n str>) -> String {
        let base = self.ancestors().count();
        let mut out = String::new();
        for element in self.descendants() {
            let depth = match &element {
                SyntaxElement::Node(node) => node.ancestors().count(),
                SyntaxElement::Token(token) => token.ancestors().count() + 1,
            } - base;
            let kind = element.kind();
            // Writing to a String cannot fail.
            let _ = write!(out, "{:indent$}", "", indent = depth * 2);
            let _ = match names(kind).or(kind.shared_name()) {
                Some(name) => write!(out, "{name}"),
                None => write!(out, "{}", kind.get()),
            };
            let _ = write!(out, "@{}", element.text_range());
            if let SyntaxElement::Token(token) = &element {
                let _ = write!(out, " {:?}", token.text());
            }
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests;
