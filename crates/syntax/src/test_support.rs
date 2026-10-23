//! Helpers shared by the crate's own tests.

use crate::{BuildError, Checkpoint, SyntaxKind, SyntaxTree, TextSize, TreeBuilder};

/// A language kind, `offset` places above the reserved range.
pub(crate) fn kind(offset: u16) -> SyntaxKind {
    let Some(kind) = SyntaxKind::language(SyntaxKind::FIRST_LANGUAGE + offset) else {
        panic!("offset {offset} is outside the reserved range");
    };
    kind
}

/// A text size from a raw byte count.
pub(crate) fn size(raw: u32) -> TextSize {
    TextSize::new(raw)
}

/// The text of every token, in order.
pub(crate) fn joined(tree: &SyntaxTree) -> String {
    tree.entries()
        .iter()
        .filter(|entry| entry.is_token)
        .map(|entry| {
            let end = entry.start.to_usize() + entry.len.to_usize();
            &tree.text()[entry.start.to_usize()..end]
        })
        .collect()
}

/// Drives the builder with calls that are always valid, steered by `choices`.
///
/// Tokens take one to three characters, and are sometimes empty. Nodes are
/// opened, closed and wrapped around them at random, through both
/// `start_node` and checkpoints.
pub(crate) fn build_well_formed(text: &str, choices: &[u8]) -> Result<SyntaxTree, BuildError> {
    let mut choices = choices.iter().copied().cycle().chain(std::iter::repeat(0));
    let mut next = move || choices.next().unwrap_or(0);
    let mut builder = TreeBuilder::new(text);
    let root = builder.checkpoint();
    builder.start_node(kind(0));
    // Checkpoints taken at each open depth; closing a node drops its own.
    let mut levels: Vec<Vec<Checkpoint>> = vec![Vec::new()];
    let mut rest = text;
    while !rest.is_empty() {
        match next() {
            0 | 1 => {
                builder.start_node(kind(1));
                levels.push(Vec::new());
            }
            2 if levels.len() > 1 => {
                builder.finish_node();
                levels.pop();
            }
            3 => {
                if let Some(level) = levels.last_mut() {
                    level.push(builder.checkpoint());
                }
            }
            4 => {
                let checkpoint = levels.last().and_then(|level| level.first().copied());
                if let Some(checkpoint) = checkpoint {
                    builder.start_node_at(checkpoint, kind(2));
                    levels.push(Vec::new());
                }
            }
            5 => builder.token(SyntaxKind::ERROR, TextSize::ZERO),
            _ => {}
        }
        let chars = 1 + usize::from(next() % 3);
        let len = rest.chars().take(chars).map(char::len_utf8).sum::<usize>();
        builder.token(kind(3), size(u32::try_from(len).unwrap_or(0)));
        rest = &rest[len..];
    }
    for _ in 0..levels.len() {
        builder.finish_node();
    }
    if next() % 2 == 0 {
        builder.start_node_at(root, kind(4));
        builder.finish_node();
    }
    builder.finish()
}
