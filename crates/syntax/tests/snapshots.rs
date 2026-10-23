//! Snapshot tests of this crate's own trees, through the public API only.
//!
//! Run with `LANG198X_BLESS=1` to rewrite the `.tree` files after an intended
//! change, then review the difference before committing.

use std::path::PathBuf;

use lang198x_syntax::{SyntaxKind, TextSize, TreeBuilder, check_snapshot};

const fn kind(raw: u16) -> SyntaxKind {
    match SyntaxKind::language(SyntaxKind::FIRST_LANGUAGE + raw) {
        Some(kind) => kind,
        None => panic!("language kinds start at SyntaxKind::FIRST_LANGUAGE"),
    }
}

const PROGRAM: SyntaxKind = kind(0);
const LINE: SyntaxKind = kind(1);
const LABEL: SyntaxKind = kind(2);
const MNEMONIC: SyntaxKind = kind(3);
const OPERANDS: SyntaxKind = kind(4);
const SUM: SyntaxKind = kind(5);
const NUMBER: SyntaxKind = kind(6);
const PLUS: SyntaxKind = kind(7);
const COLON: SyntaxKind = kind(8);
const REGISTER: SyntaxKind = kind(9);
const COMMA: SyntaxKind = kind(10);

fn names(kind: SyntaxKind) -> Option<&'static str> {
    Some(match kind {
        PROGRAM => "PROGRAM",
        LINE => "LINE",
        LABEL => "LABEL",
        MNEMONIC => "MNEMONIC",
        OPERANDS => "OPERANDS",
        SUM => "SUM",
        NUMBER => "NUMBER",
        PLUS => "PLUS",
        COLON => "COLON",
        REGISTER => "REGISTER",
        COMMA => "COMMA",
        _ => return None,
    })
}

fn expected(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(name)
}

#[test]
fn a_two_line_program_with_a_checkpointed_sum() {
    let source = "start: ld a,1+2 ; go\n\tret\n";
    let mut b = TreeBuilder::new(source);
    let len = |n| TextSize::new(n);
    b.start_node(PROGRAM);
    // start: ld a,1+2 ; go
    b.start_node(LINE);
    b.start_node(LABEL);
    b.token(kind(11), len(5));
    b.token(COLON, len(1));
    b.finish_node();
    b.token(SyntaxKind::WHITESPACE, len(1));
    b.token(MNEMONIC, len(2));
    b.token(SyntaxKind::WHITESPACE, len(1));
    b.start_node(OPERANDS);
    b.token(REGISTER, len(1));
    b.token(COMMA, len(1));
    let before_sum = b.checkpoint();
    b.token(NUMBER, len(1));
    b.start_node_at(before_sum, SUM);
    b.token(PLUS, len(1));
    b.token(NUMBER, len(1));
    b.finish_node();
    b.finish_node();
    b.token(SyntaxKind::WHITESPACE, len(1));
    b.token(SyntaxKind::COMMENT, len(4));
    b.token(SyntaxKind::NEWLINE, len(1));
    b.finish_node();
    // \tret
    b.start_node(LINE);
    b.token(SyntaxKind::WHITESPACE, len(1));
    b.token(MNEMONIC, len(3));
    b.token(SyntaxKind::NEWLINE, len(1));
    b.finish_node();
    b.finish_node();
    let tree = match b.finish() {
        Ok(tree) => tree,
        Err(error) => panic!("the program builds: {error}"),
    };
    assert_eq!(tree.text(), source);
    if let Err(error) = check_snapshot(&tree.root().dump(names), expected("two-line-program.tree"))
    {
        panic!("{error}");
    }
}
