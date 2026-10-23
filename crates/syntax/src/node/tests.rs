use std::collections::HashSet;

use proptest::prelude::*;

use super::*;
use crate::TreeBuilder;
use crate::test_support::{build_well_formed, kind, size};

const ROOT: u16 = 0;
const LINE: u16 = 1;
const MNEMONIC: u16 = 2;
const OPERANDS: u16 = 3;
const REGISTER: u16 = 4;
const COMMA: u16 = 5;
const NUMBER: u16 = 6;

/// `ld a,1 ; x` and a newline, as:
///
/// ```text
/// ROOT 0..11
///   LINE 0..10
///     MNEMONIC "ld" 0..2, WHITESPACE 2..3
///     OPERANDS 3..6: REGISTER "a", COMMA ",", NUMBER "1"
///     WHITESPACE 6..7, COMMENT "; x" 7..10
///   NEWLINE 10..11
/// ```
fn sample() -> SyntaxTree {
    let mut b = TreeBuilder::new("ld a,1 ; x\n");
    b.start_node(kind(ROOT));
    b.start_node(kind(LINE));
    b.token(kind(MNEMONIC), size(2));
    b.token(SyntaxKind::WHITESPACE, size(1));
    b.start_node(kind(OPERANDS));
    b.token(kind(REGISTER), size(1));
    b.token(kind(COMMA), size(1));
    b.token(kind(NUMBER), size(1));
    b.finish_node();
    b.token(SyntaxKind::WHITESPACE, size(1));
    b.token(SyntaxKind::COMMENT, size(3));
    b.finish_node();
    b.token(SyntaxKind::NEWLINE, size(1));
    b.finish_node();
    match b.finish() {
        Ok(tree) => tree,
        Err(error) => panic!("the sample tree is valid: {error}"),
    }
}

fn range(start: u32, end: u32) -> TextRange {
    let Some(range) = TextRange::new(size(start), size(end)) else {
        panic!("{start}..{end} is not a range");
    };
    range
}

fn node(element: Option<SyntaxElement>) -> SyntaxNode {
    let Some(SyntaxElement::Node(node)) = element else {
        panic!("expected a node, got {element:?}");
    };
    node
}

fn token(element: Option<SyntaxElement>) -> SyntaxToken {
    let Some(SyntaxElement::Token(token)) = element else {
        panic!("expected a token, got {element:?}");
    };
    token
}

/// The LINE and OPERANDS nodes of the sample.
fn line_and_operands(tree: &SyntaxTree) -> (SyntaxNode, SyntaxNode) {
    let line = node(tree.root().children().next());
    let operands = node(line.children().nth(2));
    (line, operands)
}

fn texts(elements: impl Iterator<Item = SyntaxElement>) -> Vec<String> {
    elements
        .map(|element| match element {
            SyntaxElement::Node(node) => format!("{:?}@{}", node.kind(), node.text_range()),
            SyntaxElement::Token(token) => token.text().to_owned(),
        })
        .collect()
}

#[test]
fn the_root_spans_the_text_and_has_no_parent() {
    let tree = sample();
    let root = tree.root();
    assert_eq!(root.kind(), kind(ROOT));
    assert_eq!(root.text(), "ld a,1 ; x\n");
    assert_eq!(root.text_range(), range(0, 11));
    assert_eq!(root.parent(), None);
    assert_eq!(root.next_sibling(), None);
    assert_eq!(root.prev_sibling(), None);
}

#[test]
fn children_come_in_source_order() {
    let tree = sample();
    let (line, operands) = line_and_operands(&tree);
    assert_eq!(
        texts(tree.root().children()),
        ["SyntaxKind(17)@0..10", "\n"]
    );
    assert_eq!(
        texts(line.children()),
        ["ld", " ", "SyntaxKind(19)@3..6", " ", "; x"]
    );
    assert_eq!(texts(operands.children()), ["a", ",", "1"]);
    assert_eq!(operands.text(), "a,1");
}

#[test]
fn siblings_step_over_whole_subtrees() {
    let tree = sample();
    let (line, operands) = line_and_operands(&tree);
    assert_eq!(token(operands.next_sibling()).text_range(), range(6, 7));
    assert_eq!(token(operands.prev_sibling()).text_range(), range(2, 3));
    let first = token(line.children().next());
    assert_eq!(first.prev_sibling(), None);
    assert_eq!(
        node(
            first
                .next_sibling()
                .and_then(|e| token(Some(e)).next_sibling())
        ),
        operands
    );
    let last = token(line.children().last());
    assert_eq!(last.next_sibling(), None);
    assert_eq!(token(last.prev_sibling()).text(), " ");
}

#[test]
fn parents_and_ancestors_lead_to_the_root() {
    let tree = sample();
    let (line, operands) = line_and_operands(&tree);
    let register = token(operands.children().next());
    assert_eq!(register.parent(), operands);
    assert_eq!(
        register.ancestors().collect::<Vec<_>>(),
        [operands.clone(), line.clone(), tree.root()]
    );
    assert_eq!(
        operands.ancestors().collect::<Vec<_>>(),
        [operands, line, tree.root()]
    );
}

#[test]
fn descendants_include_the_node_itself_first() {
    let tree = sample();
    let (_, operands) = line_and_operands(&tree);
    assert_eq!(
        texts(operands.descendants()),
        ["SyntaxKind(19)@3..6", "a", ",", "1"]
    );
    assert_eq!(tree.root().descendants().count(), 11);
}

#[test]
fn token_at_offset_finds_one_token_or_the_two_either_side() {
    let tree = sample();
    let root = tree.root();
    let at = |offset| match root.token_at_offset(size(offset)) {
        TokenAtOffset::None => "none".to_owned(),
        TokenAtOffset::Single(token) => token.text().to_owned(),
        TokenAtOffset::Between(left, right) => format!("{}|{}", left.text(), right.text()),
    };
    assert_eq!(at(0), "ld");
    assert_eq!(at(1), "ld");
    assert_eq!(at(2), "ld| ");
    assert_eq!(at(4), "a|,");
    assert_eq!(at(8), "; x");
    assert_eq!(at(11), "\n");
    assert_eq!(at(12), "none");
}

#[test]
fn token_at_offset_stays_inside_the_node_it_is_asked_on() {
    let tree = sample();
    let (_, operands) = line_and_operands(&tree);
    assert_eq!(operands.token_at_offset(size(2)), TokenAtOffset::None);
    assert_eq!(
        operands.token_at_offset(size(3)),
        TokenAtOffset::Single(token(operands.children().next()))
    );
    assert_eq!(
        operands.token_at_offset(size(6)),
        TokenAtOffset::Single(token(operands.children().last()))
    );
}

#[test]
fn covering_element_finds_the_deepest_cover() {
    let tree = sample();
    let root = tree.root();
    let (line, operands) = line_and_operands(&tree);
    assert_eq!(node(root.covering_element(range(3, 6))), operands);
    assert_eq!(node(root.covering_element(range(3, 5))), operands);
    assert_eq!(node(root.covering_element(range(0, 4))), line);
    assert_eq!(node(root.covering_element(range(0, 11))), root);
    assert_eq!(token(root.covering_element(range(3, 4))).text(), "a");
    // An empty range on a boundary takes the first token that touches it.
    assert_eq!(token(root.covering_element(range(4, 4))).text(), "a");
    assert_eq!(root.covering_element(range(0, 12)), None);
    assert_eq!(operands.covering_element(range(0, 4)), None);
}

#[test]
fn an_empty_tree_has_no_token_at_any_offset() {
    let mut b = TreeBuilder::new("");
    b.start_node(kind(ROOT));
    b.token(SyntaxKind::ERROR, size(0));
    b.finish_node();
    let Ok(tree) = b.finish() else {
        panic!("an empty root is a valid tree");
    };
    assert_eq!(tree.root().token_at_offset(size(0)), TokenAtOffset::None);
    assert_eq!(
        token(tree.root().covering_element(range(0, 0))).kind(),
        SyntaxKind::ERROR
    );
}

#[test]
fn elements_report_their_node_or_token() {
    let tree = sample();
    let (line, operands) = line_and_operands(&tree);
    let as_node = SyntaxElement::Node(operands.clone());
    let as_token = line.children().next().unwrap_or(as_node.clone());
    assert_eq!(as_node.kind(), kind(OPERANDS));
    assert_eq!(as_node.text_range(), range(3, 6));
    assert_eq!(as_node.parent(), Some(line.clone()));
    assert_eq!(as_node.clone().into_node(), Some(operands));
    assert_eq!(as_node.into_token(), None);
    assert_eq!(as_token.kind(), kind(MNEMONIC));
    assert_eq!(as_token.text_range(), range(0, 2));
    assert_eq!(as_token.parent(), Some(line));
    assert_eq!(as_token.clone().into_node(), None);
    assert!(as_token.into_token().is_some());
}

#[test]
fn handles_are_equal_only_within_one_tree() {
    let tree = sample();
    let twin = sample();
    let (_, operands) = line_and_operands(&tree);
    let register = token(operands.children().next());
    assert_eq!(operands, node(tree.root().covering_element(range(3, 6))));
    assert_ne!(tree.root(), twin.root());
    assert_ne!(
        register,
        token(line_and_operands(&twin).1.children().next())
    );
    let nodes: HashSet<_> = [tree.root(), tree.root(), twin.root()]
        .into_iter()
        .collect();
    assert_eq!(nodes.len(), 2);
    let tokens: HashSet<_> = [
        register.clone(),
        register,
        token(operands.children().last()),
    ]
    .into_iter()
    .collect();
    assert_eq!(tokens.len(), 2);
}

#[test]
fn handles_print_their_kind_range_and_text() {
    let tree = sample();
    let (_, operands) = line_and_operands(&tree);
    assert_eq!(
        format!("{operands:?}"),
        "SyntaxNode { kind: SyntaxKind(19), range: 3..6 }"
    );
    assert_eq!(
        format!("{:?}", token(operands.children().next())),
        "SyntaxToken { kind: SyntaxKind(20), range: 3..4, text: \"a\" }"
    );
}

#[test]
fn handles_outlive_the_tree_variable_and_can_be_stored() {
    // A front end can keep nodes in its own long-lived structures, with no
    // lifetime, after the variable holding the tree has gone.
    struct Cache {
        operands: SyntaxNode,
        register: SyntaxToken,
    }
    let cache = {
        let tree = sample();
        let (_, operands) = line_and_operands(&tree);
        let register = token(operands.children().next());
        Cache { operands, register }
    };
    assert_eq!(cache.operands.text(), "a,1");
    assert_eq!(cache.register.text(), "a");
    assert_eq!(cache.register.parent(), cache.operands);
    assert_eq!(cache.operands.ancestors().count(), 3);
}

#[test]
fn trees_and_handles_can_cross_threads() {
    fn shareable<T: Send + Sync + 'static>() {}
    shareable::<SyntaxTree>();
    shareable::<SyntaxNode>();
    shareable::<SyntaxToken>();
    shareable::<SyntaxElement>();
    let tree = sample();
    let root = tree.root();
    let text = std::thread::spawn(move || root.text().to_owned()).join();
    assert_eq!(text.ok().as_deref(), Some("ld a,1 ; x\n"));
}

#[test]
fn trees_are_equal_by_content_but_handles_by_identity() {
    let tree = sample();
    let twin = sample();
    assert_eq!(tree, twin);
    assert_eq!(tree, tree.clone());
    assert_ne!(tree.root(), twin.root());
    assert_eq!(tree.root(), tree.clone().root());
}

/// Every token in source order, found by walking all the descendants.
fn all_tokens(tree: &SyntaxTree) -> Vec<SyntaxToken> {
    tree.root()
        .descendants()
        .filter_map(SyntaxElement::into_token)
        .collect()
}

proptest! {
    /// Walking the descendants visits every entry exactly once, in source
    /// order, and the tokens join back into the text.
    #[test]
    fn descendants_visit_every_entry_once_in_order(
        text in "\\PC{0,24}",
        choices in proptest::collection::vec(0u8..8, 0..200),
    ) {
        let tree = build_well_formed(&text, &choices).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let walked: Vec<_> = tree.root().descendants().map(|e| (e.kind(), e.text_range())).collect();
        let stored: Vec<_> = tree.entries().iter().map(|e| (e.kind, range_of(e))).collect();
        prop_assert_eq!(walked, stored);
        let joined: String = all_tokens(&tree).iter().map(|t| t.text()).collect();
        prop_assert_eq!(joined, text);
    }

    /// Children fill their parent exactly, know their parent, and link to
    /// each other as siblings. Ancestors lead to the root and enclose.
    #[test]
    fn children_siblings_and_ancestors_agree(
        text in "\\PC{0,24}",
        choices in proptest::collection::vec(0u8..8, 0..200),
    ) {
        let tree = build_well_formed(&text, &choices).map_err(|e| TestCaseError::fail(e.to_string()))?;
        for element in tree.root().descendants() {
            let ancestors: Vec<_> = match &element {
                SyntaxElement::Node(node) => node.ancestors().skip(1).collect(),
                SyntaxElement::Token(token) => token.ancestors().collect(),
            };
            prop_assert_eq!(ancestors.last().cloned().unwrap_or(tree.root()), tree.root());
            for ancestor in &ancestors {
                prop_assert!(ancestor.text_range().contains_range(element.text_range()));
            }
            let SyntaxElement::Node(node) = element else { continue };
            let children: Vec<_> = node.children().collect();
            let mut offset = node.text_range().start();
            for (i, child) in children.iter().enumerate() {
                prop_assert_eq!(child.parent(), Some(node.clone()));
                prop_assert_eq!(child.text_range().start(), offset);
                offset = child.text_range().end();
                let (next, prev) = match child {
                    SyntaxElement::Node(n) => (n.next_sibling(), n.prev_sibling()),
                    SyntaxElement::Token(t) => (t.next_sibling(), t.prev_sibling()),
                };
                prop_assert_eq!(next, children.get(i + 1).cloned());
                prop_assert_eq!(prev, i.checked_sub(1).and_then(|p| children.get(p).cloned()));
            }
            prop_assert_eq!(offset, node.text_range().end());
        }
    }

    /// `token_at_offset` agrees with a search through every token, at every
    /// offset, including inside multi-byte characters and past the end.
    #[test]
    fn token_at_offset_matches_a_full_search(
        text in "\\PC{0,24}",
        choices in proptest::collection::vec(0u8..8, 0..200),
    ) {
        let tree = build_well_formed(&text, &choices).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let tokens = all_tokens(&tree);
        let end = u32::try_from(text.len()).unwrap_or(0);
        for raw in 0..=end + 1 {
            let offset = size(raw);
            let left = tokens.iter().find(|t| {
                let r = t.text_range();
                r.start() < offset && offset <= r.end()
            }).cloned();
            let right = tokens.iter().find(|t| t.text_range().contains(offset)).cloned();
            let expected = match (left, right) {
                (Some(l), Some(r)) if l == r => TokenAtOffset::Single(l),
                (Some(l), Some(r)) => TokenAtOffset::Between(l, r),
                (Some(t), None) | (None, Some(t)) => TokenAtOffset::Single(t),
                (None, None) => TokenAtOffset::None,
            };
            prop_assert_eq!(tree.root().token_at_offset(offset), expected, "offset {}", raw);
        }
    }

    /// `covering_element` returns something that covers the range, with no
    /// child that also covers it, and `None` for ranges outside the tree.
    #[test]
    fn covering_element_is_the_deepest_cover(
        text in "\\PC{0,24}",
        choices in proptest::collection::vec(0u8..8, 0..200),
        a in 0u32..120,
        b in 0u32..120,
    ) {
        let tree = build_well_formed(&text, &choices).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let wanted = range(a.min(b), a.max(b));
        let covering = tree.root().covering_element(wanted);
        if !tree.root().text_range().contains_range(wanted) {
            prop_assert_eq!(covering, None);
            return Ok(());
        }
        let Some(covering) = covering else {
            return Err(TestCaseError::fail("a range inside the tree has a cover"));
        };
        prop_assert!(covering.text_range().contains_range(wanted));
        if let SyntaxElement::Node(node) = covering {
            prop_assert!(node.children().all(|c| !c.text_range().contains_range(wanted)));
        }
    }
}
