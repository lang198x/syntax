//! The rules every finished tree must keep, checked through the public API.
//!
//! The integration tests and the fuzz targets share this file. It sees only
//! what a language front end sees, so it cannot rely on how trees are stored.

use lang198x_syntax::{SyntaxElement, SyntaxNode, SyntaxTree, TextRange, TextSize, TokenAtOffset};

/// Panics, with the reason, if `tree` breaks any rule for a tree built from
/// `text`.
pub fn check_tree(tree: &SyntaxTree, text: &str) {
    assert_eq!(tree.text(), text, "the tree holds a different text");
    let root = tree.root();
    let len = TextSize::of(text).expect("fuzz inputs are far below 4 GiB");
    assert_eq!(
        root.text_range(),
        TextRange::empty(TextSize::ZERO).cover(TextRange::empty(len))
    );
    assert_eq!(root.parent(), None, "the root has a parent");

    let mut joined = String::new();
    let mut elements = 0;
    for element in root.descendants() {
        elements += 1;
        if let SyntaxElement::Token(token) = &element {
            assert_eq!(
                token.text_range().start(),
                TextSize::of(&joined).expect("short"),
                "tokens do not follow on from each other"
            );
            joined.push_str(token.text());
        }
        if let Some(parent) = element.parent() {
            assert!(
                parent.text_range().contains_range(element.text_range()),
                "{element:?} spills out of {parent:?}"
            );
        }
        if let SyntaxElement::Node(node) = &element {
            check_children(node);
        }
    }
    assert_eq!(joined, text, "the tokens do not join back into the text");
    assert_eq!(
        root.dump(|_| None).lines().count(),
        elements,
        "the dump has the wrong number of lines"
    );

    // Every character boundary has a token at it, unless the text is empty.
    for (offset, _) in text.char_indices().chain([(text.len(), ' ')]) {
        let offset = TextSize::try_from(offset).expect("short");
        match root.token_at_offset(offset) {
            TokenAtOffset::None => assert!(text.is_empty(), "no token at offset {offset}"),
            TokenAtOffset::Single(token) => {
                let range = token.text_range();
                assert!(range.start() <= offset && offset <= range.end());
            }
            TokenAtOffset::Between(left, right) => {
                assert_eq!(left.text_range().end(), offset);
                assert_eq!(right.text_range().start(), offset);
            }
            _ => panic!("a new kind of answer from token_at_offset"),
        }
        let cover = root.covering_element(TextRange::empty(offset));
        assert!(cover.is_some_and(|c| c.text_range().contains_range(TextRange::empty(offset))));
    }
}

/// A node's children fill it exactly, know it as their parent, and link to
/// each other as siblings.
fn check_children(node: &SyntaxNode) {
    let children: Vec<SyntaxElement> = node.children().collect();
    let mut offset = node.text_range().start();
    for (i, child) in children.iter().enumerate() {
        assert_eq!(
            child.parent().as_ref(),
            Some(node),
            "{child:?} has the wrong parent"
        );
        assert_eq!(
            child.text_range().start(),
            offset,
            "{child:?} does not follow on"
        );
        offset = child.text_range().end();
        let (next, prev) = match child {
            SyntaxElement::Node(n) => (n.next_sibling(), n.prev_sibling()),
            SyntaxElement::Token(t) => (t.next_sibling(), t.prev_sibling()),
            _ => panic!("a new kind of element"),
        };
        assert_eq!(next.as_ref(), children.get(i + 1), "wrong next sibling");
        assert_eq!(
            prev.as_ref(),
            i.checked_sub(1).and_then(|p| children.get(p)),
            "wrong previous sibling"
        );
    }
    assert_eq!(
        offset,
        node.text_range().end(),
        "{node:?}'s children do not fill it"
    );
}
