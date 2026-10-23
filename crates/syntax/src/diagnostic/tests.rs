use proptest::prelude::*;

use super::*;
use crate::test_support::{build_well_formed, kind, size};
use crate::{SyntaxElement, SyntaxKind, TextSize, TreeBuilder};

fn range(start: u32, end: u32) -> TextRange {
    let Some(range) = TextRange::new(size(start), size(end)) else {
        panic!("{start}..{end} is not a range");
    };
    range
}

/// `ld é` as one root holding three tokens. `é` is two bytes, at 3..5.
fn tree() -> SyntaxTree {
    let mut b = TreeBuilder::new("ld é");
    b.start_node(kind(0));
    b.token(kind(1), size(2));
    b.token(SyntaxKind::WHITESPACE, size(1));
    b.token(SyntaxKind::ERROR, size(2));
    b.finish_node();
    match b.finish() {
        Ok(tree) => tree,
        Err(error) => panic!("the sample tree is valid: {error}"),
    }
}

#[test]
fn a_parse_keeps_its_diagnostics_in_order_with_their_ranges() {
    let diagnostics = vec![
        Diagnostic::new(
            DiagnosticKind::Invalid,
            range(3, 5),
            "`é` is not a register",
        ),
        Diagnostic::new(
            DiagnosticKind::Unsupported,
            range(0, 2),
            "`ld` with this operand",
        ),
        Diagnostic::new(DiagnosticKind::Unknown, range(5, 5), "nothing follows"),
    ];
    let parse = match Parse::new(tree(), diagnostics.clone()) {
        Ok(parse) => parse,
        Err(error) => panic!("every range fits: {error}"),
    };
    assert_eq!(parse.diagnostics(), diagnostics);
    let first = &parse.diagnostics()[0];
    assert_eq!(first.kind(), DiagnosticKind::Invalid);
    assert_eq!(first.range(), range(3, 5));
    assert_eq!(first.message(), "`é` is not a register");
    assert_eq!(parse.tree().text(), "ld é");
    let (tree, kept) = parse.into_parts();
    assert_eq!(kept, diagnostics);
    assert_eq!(tree.root().text(), "ld é");
}

#[test]
fn a_diagnostic_past_the_end_is_refused() {
    let diagnostics = vec![
        Diagnostic::new(DiagnosticKind::Invalid, range(0, 2), "fine"),
        Diagnostic::new(DiagnosticKind::Invalid, range(4, 9), "too far"),
    ];
    assert_eq!(
        Parse::new(tree(), diagnostics),
        Err(DiagnosticOutsideText {
            index: 1,
            range: range(4, 9)
        })
    );
}

#[test]
fn a_diagnostic_inside_a_character_is_refused() {
    let diagnostics = vec![Diagnostic::new(
        DiagnosticKind::Invalid,
        range(4, 5),
        "half of é",
    )];
    let error = Parse::new(tree(), diagnostics).err();
    assert_eq!(
        error,
        Some(DiagnosticOutsideText {
            index: 0,
            range: range(4, 5)
        })
    );
    let message = error.map(|e| e.to_string()).unwrap_or_default();
    assert_eq!(
        message,
        "diagnostic 0 covers 4..5, which is not a span of the source text"
    );
}

#[test]
fn no_diagnostics_is_a_clean_parse() {
    let parse = Parse::new(tree(), Vec::new());
    assert_eq!(parse.map(|p| p.diagnostics().len()), Ok(0));
}

proptest! {
    /// Diagnostics on any element of any tree keep their kind, range and
    /// message when attached, and ranges past the end are always refused.
    #[test]
    fn diagnostics_keep_their_ranges_when_attached(
        text in "\\PC{0,24}",
        choices in proptest::collection::vec(0u8..8, 0..200),
        picks in proptest::collection::vec((0usize..64, 0u8..3), 0..8),
        overshoot in 1u32..5,
    ) {
        let tree = build_well_formed(&text, &choices).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let elements: Vec<SyntaxElement> = tree.root().descendants().collect();
        let kinds = [DiagnosticKind::Invalid, DiagnosticKind::Unsupported, DiagnosticKind::Unknown];
        let diagnostics: Vec<_> = picks
            .iter()
            .map(|&(i, k)| {
                let element = &elements[i % elements.len()];
                Diagnostic::new(kinds[usize::from(k)], element.text_range(), format!("{:?}", element.kind()))
            })
            .collect();
        let past_end = TextSize::of(&text).and_then(|len| len.checked_add(size(overshoot))).unwrap_or(size(1));
        let mut too_far = diagnostics.clone();
        too_far.push(Diagnostic::new(DiagnosticKind::Unknown, TextRange::empty(past_end), "past the end"));
        let parse = Parse::new(tree.clone(), diagnostics.clone()).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(parse.diagnostics(), diagnostics.as_slice());
        for diagnostic in parse.diagnostics() {
            prop_assert!(diagnostic.range().slice(parse.tree().text()).is_some());
        }
        prop_assert_eq!(
            Parse::new(tree, too_far).err().map(|e| e.index),
            Some(diagnostics.len())
        );
    }
}
