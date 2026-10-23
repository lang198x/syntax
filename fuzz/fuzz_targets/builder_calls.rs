//! Any sequence of builder calls gives a tree that keeps every rule, or a
//! clean error. Nothing panics.
//!
//! The input's first byte picks the text's length and whether to complete the
//! sequence (open a root first, then tokenise the rest of the text and close
//! every node), as the crate's property test does, so that random calls
//! often produce real trees. The bytes after it are the text, then the calls.
#![no_main]

#[path = "../../crates/syntax/tests/support/invariants.rs"]
mod invariants;

use lang198x_syntax::{SyntaxKind, TextSize, TreeBuilder};

fn kind(byte: u8) -> SyntaxKind {
    let raw = SyntaxKind::FIRST_LANGUAGE + u16::from(byte % 8);
    SyntaxKind::language(raw).unwrap_or(SyntaxKind::ERROR)
}

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let Some((&first, rest)) = data.split_first() else {
        return;
    };
    let complete = first & 0x80 != 0;
    let text_len = usize::from(first & 0x1f).min(rest.len());
    let (text_bytes, mut calls) = rest.split_at(text_len);
    let text = String::from_utf8_lossy(text_bytes);

    let mut builder = TreeBuilder::new(&text);
    // The harness's own guess at how far the tokens have got and how many
    // nodes are open, for completing the sequence. If the guess is ever
    // wrong, the builder reports an error and this input is not checked;
    // it cannot cause a false failure.
    let (mut pos, mut depth) = (0usize, 0usize);
    if complete {
        builder.start_node(kind(0));
        depth += 1;
    }
    let mut checkpoints = Vec::new();
    // Each call is an opcode byte and an argument byte.
    while let [op, arg, tail @ ..] = calls {
        calls = tail;
        match op % 5 {
            0 => {
                builder.start_node(kind(*arg));
                depth += 1;
            }
            1 => {
                let len = usize::from(arg % 8);
                builder.token(kind(*arg >> 3), TextSize::new(u32::from(arg % 8)));
                if text.is_char_boundary(pos + len) {
                    pos += len;
                }
            }
            2 => {
                builder.finish_node();
                depth = depth.saturating_sub(1);
            }
            3 => checkpoints.push(builder.checkpoint()),
            _ => {
                if !checkpoints.is_empty() {
                    let checkpoint = checkpoints[usize::from(*arg) % checkpoints.len()];
                    builder.start_node_at(checkpoint, kind(*arg >> 3));
                    depth += 1;
                }
            }
        }
    }
    if complete {
        let rest = text.len().saturating_sub(pos);
        builder.token(kind(1), TextSize::new(u32::try_from(rest).unwrap_or(0)));
        for _ in 0..depth {
            builder.finish_node();
        }
    }
    if let Ok(tree) = builder.finish() {
        invariants::check_tree(&tree, &text);
    }
});
