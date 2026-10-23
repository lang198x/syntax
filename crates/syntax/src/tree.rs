//! A finished syntax tree, and how it is stored.
//!
//! The tree is one flat array of entries, one per node or token, in source
//! order: each node comes before everything inside it. Every entry records its
//! kind, where it sits in the text, its parent and how many entries its subtree
//! holds. The source text is stored once, in the tree, and tokens point into it.
//!
//! None of this is visible outside the crate. Languages see the tree only
//! through this crate's own types, so the storage can change without touching
//! any language.

use std::fmt;

use crate::{SyntaxKind, TextSize};

/// One node or token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) kind: SyntaxKind,
    pub(crate) is_token: bool,
    pub(crate) start: TextSize,
    pub(crate) len: TextSize,
    /// The index of the parent node, or `None` for the root.
    pub(crate) parent: Option<u32>,
    /// How many entries this subtree holds, counting this one.
    pub(crate) subtree: u32,
}

/// A finished syntax tree.
///
/// A tree is built with a [`TreeBuilder`](crate::TreeBuilder) and never
/// changes afterwards, so it can be shared between threads freely. It holds the
/// source text, and its tokens cover that text exactly, in order.
#[derive(Clone, PartialEq, Eq)]
pub struct SyntaxTree {
    text: Box<str>,
    entries: Box<[Entry]>,
}

impl SyntaxTree {
    pub(crate) fn from_parts(text: &str, entries: Vec<Entry>) -> Self {
        Self {
            text: text.into(),
            entries: entries.into_boxed_slice(),
        }
    }

    /// Returns the source text the tree was built from.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[cfg(test)]
    pub(crate) fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Checks every rule a finished tree must keep, and says which one broke.
    ///
    /// The builder only produces trees that pass. Tests use this to prove it.
    #[cfg(test)]
    pub(crate) fn check(&self) -> Result<(), String> {
        let entries = &self.entries;
        let n = u32::try_from(entries.len()).map_err(|_| "too many entries")?;
        let Some(root) = entries.first() else {
            return Err("the tree has no root".into());
        };
        let text_len = TextSize::of(&self.text).ok_or("the text is too long")?;
        if root.is_token || root.parent.is_some() {
            return Err("entry 0 is not a root node".into());
        }
        if root.start != TextSize::ZERO || root.len != text_len || root.subtree != n {
            return Err("the root does not span the whole tree".into());
        }
        for (i, entry) in entries.iter().enumerate().skip(1) {
            let i = u32::try_from(i).map_err(|_| "too many entries")?;
            let parent = entry.parent.ok_or(format!("entry {i} has no parent"))?;
            if parent >= i {
                return Err(format!("entry {i} comes before its parent"));
            }
            let p = &entries[parent as usize];
            if p.is_token {
                return Err(format!("entry {i}'s parent is a token"));
            }
            if entry.subtree == 0 || (entry.is_token && entry.subtree != 1) {
                return Err(format!("entry {i} has a bad subtree size"));
            }
            if u64::from(i) + u64::from(entry.subtree) > u64::from(parent) + u64::from(p.subtree) {
                return Err(format!("entry {i} runs past its parent's subtree"));
            }
        }
        // Each node's children follow on from one another and exactly fill it.
        for (i, node) in entries.iter().enumerate().filter(|(_, e)| !e.is_token) {
            let end = i + node.subtree as usize;
            let mut child = i + 1;
            let mut offset = node.start;
            while child < end {
                let c = &entries[child];
                if c.parent != u32::try_from(i).ok() {
                    return Err(format!("entry {child} is not a child of {i}"));
                }
                if c.start != offset {
                    return Err(format!("entry {child} does not follow on"));
                }
                offset = offset.checked_add(c.len).ok_or("an offset overflowed")?;
                child += c.subtree as usize;
            }
            if node.start.checked_add(node.len) != Some(offset) {
                return Err(format!("node {i}'s children do not fill it"));
            }
        }
        // Tokens cover the text in order, and each can be sliced from it.
        let mut offset = TextSize::ZERO;
        for (i, token) in entries.iter().enumerate().filter(|(_, e)| e.is_token) {
            let end = token.start.checked_add(token.len).ok_or("overflow")?;
            if token.start != offset
                || self
                    .text
                    .get(token.start.to_usize()..end.to_usize())
                    .is_none()
            {
                return Err(format!("token {i} does not slice the text in order"));
            }
            offset = end;
        }
        if offset != text_len {
            return Err("the tokens do not cover the whole text".into());
        }
        Ok(())
    }
}

impl fmt::Debug for SyntaxTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Shows the size, not the storage, which stays private.
        f.debug_struct("SyntaxTree")
            .field("text_len", &self.text.len())
            .field("elements", &self.entries.len())
            .finish()
    }
}
