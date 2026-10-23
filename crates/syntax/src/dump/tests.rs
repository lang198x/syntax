use proptest::prelude::*;

use crate::test_support::{build_well_formed, kind, size};
use crate::{SyntaxElement, SyntaxKind, SyntaxTree, TreeBuilder};

fn tree() -> SyntaxTree {
    // ROOT(LINE(MNEMONIC "ld", WHITESPACE "\t", STRING "\"é\"" (4 bytes)), NEWLINE)
    let mut b = TreeBuilder::new("ld\t\"é\"\n");
    b.start_node(kind(0));
    b.start_node(kind(1));
    b.token(kind(2), size(2));
    b.token(SyntaxKind::WHITESPACE, size(1));
    b.token(kind(9), size(4));
    b.finish_node();
    b.token(SyntaxKind::NEWLINE, size(1));
    b.finish_node();
    match b.finish() {
        Ok(tree) => tree,
        Err(error) => panic!("the sample tree is valid: {error}"),
    }
}

fn names(kind: SyntaxKind) -> Option<&'static str> {
    match kind.get().checked_sub(SyntaxKind::FIRST_LANGUAGE)? {
        0 => Some("ROOT"),
        1 => Some("LINE"),
        2 => Some("MNEMONIC"),
        _ => None,
    }
}

#[test]
fn a_dump_shows_kinds_ranges_and_escaped_text() {
    let expected = [
        "ROOT@0..8",
        "  LINE@0..7",
        "    MNEMONIC@0..2 \"ld\"",
        "    WHITESPACE@2..3 \"\\t\"",
        "    25@3..7 \"\\\"é\\\"\"",
        "  NEWLINE@7..8 \"\\n\"",
        "",
    ]
    .join("\n");
    assert_eq!(tree().root().dump(names), expected);
}

#[test]
fn a_dump_of_a_subtree_starts_at_no_indent() {
    let tree = tree();
    let Some(SyntaxElement::Node(line)) = tree.root().children().next() else {
        panic!("the root's first child is LINE");
    };
    assert!(line.dump(names).starts_with("LINE@0..7\n  MNEMONIC@0..2"));
}

#[test]
fn kinds_without_a_name_show_their_shared_name_or_number() {
    let dump = tree().root().dump(|_| None);
    assert!(dump.starts_with("16@0..8\n  17@0..7\n"));
    assert!(dump.contains("WHITESPACE@2..3"));
}

proptest! {
    /// A dump has one line per element, indented by its depth, in order.
    #[test]
    fn a_dump_has_one_line_per_element_at_its_depth(
        text in "\\PC{0,24}",
        choices in proptest::collection::vec(0u8..8, 0..200),
    ) {
        let tree = build_well_formed(&text, &choices).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let dump = tree.root().dump(|_| None);
        let lines: Vec<&str> = dump.lines().collect();
        let elements: Vec<SyntaxElement> = tree.root().descendants().collect();
        prop_assert_eq!(lines.len(), elements.len());
        for (line, element) in lines.iter().zip(&elements) {
            let depth = element.parent().map_or(0, |p| p.ancestors().count());
            let indent = line.len() - line.trim_start_matches(' ').len();
            prop_assert_eq!(indent, depth * 2, "line {:?}", line);
            let range = format!("@{} ", element.text_range());
            prop_assert!(line.contains(range.trim_end()), "line {:?}", line);
        }
    }
}
