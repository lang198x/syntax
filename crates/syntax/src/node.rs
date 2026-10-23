//! Moving around a finished tree.
//!
//! A [`SyntaxNode`] or [`SyntaxToken`] is a handle: a shared handle to the
//! tree and a position in it. Handles are cheap to clone and carry no
//! lifetime, so they can be stored and cached freely; the tree lives as long
//! as any handle to it. A handle can only point at something that exists, so
//! every accessor returns a plain value or an `Option`, never an error.

use std::fmt;
use std::hash::{Hash, Hasher};

use crate::tree::{Entry, SyntaxTree};
use crate::{SyntaxKind, TextRange, TextSize};

/// A node in a syntax tree: a span of source made of other nodes and tokens.
#[derive(Clone)]
pub struct SyntaxNode {
    tree: SyntaxTree,
    index: u32,
}

/// A token in a syntax tree: a span of source text with a kind, such as a
/// mnemonic, a number or a comment. Tokens are the leaves of the tree.
#[derive(Clone)]
pub struct SyntaxToken {
    tree: SyntaxTree,
    index: u32,
}

/// Either a node or a token.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SyntaxElement {
    /// A node.
    Node(SyntaxNode),
    /// A token.
    Token(SyntaxToken),
}

/// The tokens at a text offset, from [`SyntaxNode::token_at_offset`].
///
/// An offset can fall inside one token, or on the boundary between two.
/// Empty tokens are never returned, because they cover no text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TokenAtOffset {
    /// The offset is outside the node, or the node holds no text.
    None,
    /// The offset is inside this token, or at the start or end of the node.
    Single(SyntaxToken),
    /// The offset is where the first token ends and the second begins.
    Between(SyntaxToken, SyntaxToken),
}

/// Returns the entry a handle points at.
///
/// Handles are only made for entries that exist, and a tree never changes, so
/// the index is always in range.
fn entry(tree: &SyntaxTree, index: u32) -> &Entry {
    &tree.entries()[index as usize]
}

fn range_of(entry: &Entry) -> TextRange {
    // The builder only stores ranges that fit, so the fallback is never used.
    TextRange::at(entry.start, entry.len).unwrap_or(TextRange::empty(entry.start))
}

fn element(tree: &SyntaxTree, index: u32) -> SyntaxElement {
    let tree = tree.clone();
    if entry(&tree, index).is_token {
        SyntaxElement::Token(SyntaxToken { tree, index })
    } else {
        SyntaxElement::Node(SyntaxNode { tree, index })
    }
}

fn text_of(tree: &SyntaxTree, index: u32) -> &str {
    range_of(entry(tree, index))
        .slice(tree.text())
        .unwrap_or("")
}

fn parent_of(tree: &SyntaxTree, index: u32) -> Option<SyntaxNode> {
    entry(tree, index).parent.map(|index| SyntaxNode {
        tree: tree.clone(),
        index,
    })
}

fn next_sibling_of(tree: &SyntaxTree, index: u32) -> Option<SyntaxElement> {
    let parent = entry(tree, index).parent?;
    let next = index + entry(tree, index).subtree;
    (next < parent + entry(tree, parent).subtree).then(|| element(tree, next))
}

fn prev_sibling_of(tree: &SyntaxTree, index: u32) -> Option<SyntaxElement> {
    let parent = entry(tree, index).parent?;
    let mut child = parent + 1;
    let mut prev = None;
    while child < index {
        prev = Some(child);
        child += entry(tree, child).subtree;
    }
    prev.map(|prev| element(tree, prev))
}

impl SyntaxNode {
    pub(crate) fn new(tree: SyntaxTree, index: u32) -> Self {
        Self { tree, index }
    }

    /// Returns the node's kind.
    #[must_use]
    pub fn kind(&self) -> SyntaxKind {
        entry(&self.tree, self.index).kind
    }

    /// Returns the span of source the node covers.
    #[must_use]
    pub fn text_range(&self) -> TextRange {
        range_of(entry(&self.tree, self.index))
    }

    /// Returns the source text the node covers, exactly as written.
    #[must_use]
    pub fn text(&self) -> &str {
        text_of(&self.tree, self.index)
    }

    /// Returns the node this one sits in, or `None` for the root.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        parent_of(&self.tree, self.index)
    }

    /// Returns the nodes and tokens directly inside this node, in order.
    pub fn children(&self) -> impl Iterator<Item = SyntaxElement> + use<> {
        let tree = self.tree.clone();
        let end = self.index + entry(&tree, self.index).subtree;
        let mut next = self.index + 1;
        std::iter::from_fn(move || {
            (next < end).then(|| {
                let child = next;
                next += entry(&tree, child).subtree;
                element(&tree, child)
            })
        })
    }

    /// Returns the node or token after this one in the same parent.
    #[must_use]
    pub fn next_sibling(&self) -> Option<SyntaxElement> {
        next_sibling_of(&self.tree, self.index)
    }

    /// Returns the node or token before this one in the same parent.
    #[must_use]
    pub fn prev_sibling(&self) -> Option<SyntaxElement> {
        prev_sibling_of(&self.tree, self.index)
    }

    /// Returns this node, then its parent, and so on up to the root.
    pub fn ancestors(&self) -> impl Iterator<Item = Self> + use<> {
        std::iter::successors(Some(self.clone()), Self::parent)
    }

    /// Returns this node and everything inside it, in source order. Each node
    /// comes before the nodes and tokens it holds.
    pub fn descendants(&self) -> impl Iterator<Item = SyntaxElement> + use<> {
        let tree = self.tree.clone();
        let end = self.index + entry(&tree, self.index).subtree;
        (self.index..end).map(move |index| element(&tree, index))
    }

    /// Returns the token or tokens at `offset`.
    ///
    /// An offset inside a token gives that token. An offset where one token
    /// ends and the next begins gives both, which is what an editor needs for
    /// a cursor between them. The start and end of the node give the first
    /// and last token.
    #[must_use]
    pub fn token_at_offset(&self, offset: TextSize) -> TokenAtOffset {
        let left = self.descend_to_token(|range| range.start() < offset && offset <= range.end());
        let right = self.descend_to_token(|range| range.contains(offset));
        match (left, right) {
            (Some(left), Some(right)) if left == right => TokenAtOffset::Single(left),
            (Some(left), Some(right)) => TokenAtOffset::Between(left, right),
            (Some(token), None) | (None, Some(token)) => TokenAtOffset::Single(token),
            (None, None) => TokenAtOffset::None,
        }
    }

    /// Returns the deepest node or token that covers all of `range`, or
    /// `None` if `range` is not inside this node.
    ///
    /// Only an empty range can sit inside two siblings at once, on the
    /// boundary between them. The first of them is chosen.
    #[must_use]
    pub fn covering_element(&self, range: TextRange) -> Option<SyntaxElement> {
        if !self.text_range().contains_range(range) {
            return None;
        }
        let mut node = self.clone();
        loop {
            match node
                .children()
                .find(|child| child.text_range().contains_range(range))
            {
                None => return Some(SyntaxElement::Node(node)),
                Some(SyntaxElement::Token(token)) => return Some(SyntaxElement::Token(token)),
                Some(SyntaxElement::Node(child)) => node = child,
            }
        }
    }

    /// Walks down to the token whose range passes `test`, always taking the
    /// first child that passes. A node that passes always has such a child,
    /// because its children exactly fill it.
    fn descend_to_token(&self, test: impl Fn(TextRange) -> bool) -> Option<SyntaxToken> {
        if !test(self.text_range()) {
            return None;
        }
        let mut node = self.clone();
        loop {
            match node.children().find(|child| test(child.text_range()))? {
                SyntaxElement::Token(token) => return Some(token),
                SyntaxElement::Node(child) => node = child,
            }
        }
    }
}

impl SyntaxToken {
    /// Returns the token's kind.
    #[must_use]
    pub fn kind(&self) -> SyntaxKind {
        entry(&self.tree, self.index).kind
    }

    /// Returns the span of source the token covers.
    #[must_use]
    pub fn text_range(&self) -> TextRange {
        range_of(entry(&self.tree, self.index))
    }

    /// Returns the token's source text, exactly as written.
    #[must_use]
    pub fn text(&self) -> &str {
        text_of(&self.tree, self.index)
    }

    /// Returns the node the token sits in. Every token has one.
    #[must_use]
    pub fn parent(&self) -> SyntaxNode {
        // The builder puts every token inside a node; the root is a fallback
        // that is never used.
        parent_of(&self.tree, self.index).unwrap_or_else(|| self.tree.root())
    }

    /// Returns the node or token after this one in the same parent.
    #[must_use]
    pub fn next_sibling(&self) -> Option<SyntaxElement> {
        next_sibling_of(&self.tree, self.index)
    }

    /// Returns the node or token before this one in the same parent.
    #[must_use]
    pub fn prev_sibling(&self) -> Option<SyntaxElement> {
        prev_sibling_of(&self.tree, self.index)
    }

    /// Returns the token's parent, then its parent, and so on up to the root.
    pub fn ancestors(&self) -> impl Iterator<Item = SyntaxNode> + use<> {
        self.parent().ancestors()
    }
}

impl SyntaxElement {
    /// Returns the element's kind.
    #[must_use]
    pub fn kind(&self) -> SyntaxKind {
        match self {
            Self::Node(node) => node.kind(),
            Self::Token(token) => token.kind(),
        }
    }

    /// Returns the span of source the element covers.
    #[must_use]
    pub fn text_range(&self) -> TextRange {
        match self {
            Self::Node(node) => node.text_range(),
            Self::Token(token) => token.text_range(),
        }
    }

    /// Returns the node the element sits in, or `None` for the root.
    #[must_use]
    pub fn parent(&self) -> Option<SyntaxNode> {
        match self {
            Self::Node(node) => node.parent(),
            Self::Token(token) => Some(token.parent()),
        }
    }

    /// Returns the node, or `None` if this is a token.
    #[must_use]
    pub fn into_node(self) -> Option<SyntaxNode> {
        match self {
            Self::Node(node) => Some(node),
            Self::Token(_) => None,
        }
    }

    /// Returns the token, or `None` if this is a node.
    #[must_use]
    pub fn into_token(self) -> Option<SyntaxToken> {
        match self {
            Self::Node(_) => None,
            Self::Token(token) => Some(token),
        }
    }
}

// Two handles are equal when they point at the same place in the same tree.
// Equal-looking trees built separately are different trees.

impl PartialEq for SyntaxNode {
    fn eq(&self, other: &Self) -> bool {
        self.tree.is_same_tree(&other.tree) && self.index == other.index
    }
}

impl Eq for SyntaxNode {}

impl Hash for SyntaxNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tree.hash_identity(state);
        self.index.hash(state);
    }
}

impl PartialEq for SyntaxToken {
    fn eq(&self, other: &Self) -> bool {
        self.tree.is_same_tree(&other.tree) && self.index == other.index
    }
}

impl Eq for SyntaxToken {}

impl Hash for SyntaxToken {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tree.hash_identity(state);
        self.index.hash(state);
    }
}

impl fmt::Debug for SyntaxNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyntaxNode")
            .field("kind", &self.kind())
            .field("range", &format_args!("{}", self.text_range()))
            .finish()
    }
}

impl fmt::Debug for SyntaxToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyntaxToken")
            .field("kind", &self.kind())
            .field("range", &format_args!("{}", self.text_range()))
            .field("text", &self.text())
            .finish()
    }
}

#[cfg(test)]
mod tests;
