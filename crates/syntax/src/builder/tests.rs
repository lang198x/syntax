use proptest::prelude::*;

use super::*;

fn kind(offset: u16) -> SyntaxKind {
    let Some(kind) = SyntaxKind::language(SyntaxKind::FIRST_LANGUAGE + offset) else {
        panic!("offset {offset} is outside the reserved range");
    };
    kind
}

fn size(raw: u32) -> TextSize {
    TextSize::new(raw)
}

fn build(text: &str, calls: impl FnOnce(&mut TreeBuilder<'_>)) -> Result<SyntaxTree, BuildError> {
    let mut builder = TreeBuilder::new(text);
    calls(&mut builder);
    builder.finish()
}

fn ok(result: Result<SyntaxTree, BuildError>) -> SyntaxTree {
    let tree = match result {
        Ok(tree) => tree,
        Err(error) => panic!("expected a tree, got: {error}"),
    };
    if let Err(problem) = tree.check() {
        panic!("the builder made a broken tree: {problem}");
    }
    tree
}

/// The text of every token, in order.
fn joined(tree: &SyntaxTree) -> String {
    tree.entries()
        .iter()
        .filter(|entry| entry.is_token)
        .map(|entry| {
            let end = entry.start.to_usize() + entry.len.to_usize();
            &tree.text()[entry.start.to_usize()..end]
        })
        .collect()
}

/// Each entry as (kind offset, start, length, parent), for comparing shapes.
/// Shared kinds wrap below zero, which is fine for comparing.
fn shape(tree: &SyntaxTree) -> Vec<(u16, u32, u32, Option<u32>)> {
    tree.entries()
        .iter()
        .map(|e| {
            (
                e.kind.get().wrapping_sub(SyntaxKind::FIRST_LANGUAGE),
                e.start.get(),
                e.len.get(),
                e.parent,
            )
        })
        .collect()
}

#[test]
fn builds_nested_nodes_with_their_ranges_and_parents() {
    // ROOT(LINE(ld a), NEWLINE-in-root)
    let tree = ok(build("ld a\n", |b| {
        b.start_node(kind(0));
        b.start_node(kind(1));
        b.token(kind(2), size(2));
        b.token(kind(3), size(1));
        b.token(kind(4), size(1));
        b.finish_node();
        b.token(kind(5), size(1));
        b.finish_node();
    }));
    assert_eq!(joined(&tree), "ld a\n");
    assert_eq!(
        shape(&tree),
        [
            (0, 0, 5, None),
            (1, 0, 4, Some(0)),
            (2, 0, 2, Some(1)),
            (3, 2, 1, Some(1)),
            (4, 3, 1, Some(1)),
            (5, 4, 1, Some(0)),
        ]
    );
}

#[test]
fn empty_text_and_empty_tokens_are_allowed() {
    let tree = ok(build("", |b| {
        b.start_node(kind(0));
        b.token(SyntaxKind::ERROR, size(0));
        b.finish_node();
    }));
    assert_eq!(
        shape(&tree),
        [
            (0, 0, 0, None),
            (
                SyntaxKind::ERROR
                    .get()
                    .wrapping_sub(SyntaxKind::FIRST_LANGUAGE),
                0,
                0,
                Some(0)
            )
        ]
    );
}

#[test]
fn a_checkpoint_wraps_what_was_parsed_after_it() {
    // ROOT(SUM(1 + 2)): the parser only knows it has a sum on seeing `+`.
    let tree = ok(build("1+2", |b| {
        b.start_node(kind(0));
        let before_lhs = b.checkpoint();
        b.token(kind(2), size(1));
        b.start_node_at(before_lhs, kind(1));
        b.token(kind(3), size(1));
        b.token(kind(2), size(1));
        b.finish_node();
        b.finish_node();
    }));
    assert_eq!(
        shape(&tree),
        [
            (0, 0, 3, None),
            (1, 0, 3, Some(0)),
            (2, 0, 1, Some(1)),
            (3, 1, 1, Some(1)),
            (2, 2, 1, Some(1)),
        ]
    );
}

#[test]
fn a_checkpoint_can_wrap_finished_nodes_and_keeps_their_insides() {
    // ROOT(token, OUTER(INNER(a b), c)): OUTER is opened at a checkpoint taken
    // after the first token, before INNER, so INNER's tokens shift down.
    let tree = ok(build("xabc", |b| {
        b.start_node(kind(0));
        b.token(kind(5), size(1));
        let checkpoint = b.checkpoint();
        b.start_node(kind(2));
        b.token(kind(3), size(1));
        b.token(kind(3), size(1));
        b.finish_node();
        b.start_node_at(checkpoint, kind(1));
        b.token(kind(4), size(1));
        b.finish_node();
        b.finish_node();
    }));
    assert_eq!(
        shape(&tree),
        [
            (0, 0, 4, None),
            (5, 0, 1, Some(0)),
            (1, 1, 3, Some(0)),
            (2, 1, 2, Some(2)),
            (3, 1, 1, Some(3)),
            (3, 2, 1, Some(3)),
            (4, 3, 1, Some(2)),
        ]
    );
}

#[test]
fn a_checkpoint_at_the_start_can_wrap_the_finished_root() {
    let tree = ok(build("ab", |b| {
        let start = b.checkpoint();
        b.start_node(kind(1));
        b.token(kind(2), size(2));
        b.finish_node();
        b.start_node_at(start, kind(0));
        b.finish_node();
    }));
    assert_eq!(
        shape(&tree),
        [(0, 0, 2, None), (1, 0, 2, Some(0)), (2, 0, 2, Some(1))]
    );
}

#[test]
fn an_unfinished_node_is_an_error() {
    let result = build("a", |b| {
        b.start_node(kind(0));
        b.start_node(kind(1));
        b.token(kind(2), size(1));
        b.finish_node();
    });
    assert_eq!(result, Err(BuildError::UnclosedNodes { count: 1 }));
}

#[test]
fn an_extra_finish_node_is_an_error() {
    let result = build("a", |b| {
        b.start_node(kind(0));
        b.token(kind(2), size(1));
        b.finish_node();
        b.finish_node();
    });
    assert_eq!(result, Err(BuildError::ExtraFinishNode { offset: size(1) }));
}

#[test]
fn a_token_past_the_end_is_an_error() {
    let result = build("ab", |b| {
        b.start_node(kind(0));
        b.token(kind(2), size(1));
        b.token(kind(2), size(2));
        b.finish_node();
    });
    assert_eq!(result, Err(BuildError::TokenPastEnd { offset: size(1) }));
    // An offset that overflows 32 bits is past the end too.
    let result = build("ab", |b| {
        b.start_node(kind(0));
        b.token(kind(2), size(1));
        b.token(kind(2), size(u32::MAX));
    });
    assert_eq!(result, Err(BuildError::TokenPastEnd { offset: size(1) }));
}

#[test]
fn a_token_that_splits_a_character_is_an_error() {
    // `é` is two bytes in UTF-8.
    let result = build("é", |b| {
        b.start_node(kind(0));
        b.token(kind(2), size(1));
    });
    assert_eq!(
        result,
        Err(BuildError::TokenSplitsCharacter { offset: size(1) })
    );
}

#[test]
fn text_left_uncovered_is_an_error() {
    let result = build("abc", |b| {
        b.start_node(kind(0));
        b.token(kind(2), size(2));
        b.finish_node();
    });
    assert_eq!(result, Err(BuildError::TextNotCovered { offset: size(2) }));
}

#[test]
fn a_token_outside_any_node_is_an_error() {
    let result = build("a", |b| b.token(kind(2), size(1)));
    assert_eq!(
        result,
        Err(BuildError::TokenOutsideNode { offset: size(0) })
    );
}

#[test]
fn no_root_is_an_error() {
    assert_eq!(build("", |_| {}), Err(BuildError::NoRoot));
}

#[test]
fn a_second_root_is_an_error() {
    let result = build("ab", |b| {
        b.start_node(kind(0));
        b.token(kind(2), size(1));
        b.finish_node();
        b.start_node(kind(0));
    });
    assert_eq!(result, Err(BuildError::MultipleRoots { offset: size(1) }));
}

#[test]
fn a_top_level_checkpoint_after_the_root_would_make_a_second_root() {
    let result = build("a", |b| {
        b.start_node(kind(0));
        b.token(kind(2), size(1));
        b.finish_node();
        let after = b.checkpoint();
        b.start_node_at(after, kind(1));
    });
    assert_eq!(result, Err(BuildError::MultipleRoots { offset: size(1) }));
}

#[test]
fn a_checkpoint_used_after_its_node_closed_is_stale() {
    let result = build("ab", |b| {
        b.start_node(kind(0));
        b.start_node(kind(1));
        let inside = b.checkpoint();
        b.token(kind(2), size(1));
        b.finish_node();
        b.start_node_at(inside, kind(3));
    });
    assert_eq!(result, Err(BuildError::StaleCheckpoint { offset: size(1) }));
}

#[test]
fn a_checkpoint_from_another_builder_cannot_break_the_tree() {
    // The other builder's checkpoint matches this builder's depth and open
    // node, but points before that node.
    let other = TreeBuilder::new("");
    let foreign = other.checkpoint();
    let result = build("a", |b| {
        b.start_node(kind(0));
        b.start_node(kind(1));
        b.start_node_at(
            Checkpoint {
                parent: Some(1),
                depth: 2,
                ..foreign
            },
            kind(2),
        );
    });
    assert_eq!(result, Err(BuildError::StaleCheckpoint { offset: size(0) }));
    // One pointing past the end of this builder's entries is refused too.
    let result = build("a", |b| {
        b.start_node(kind(0));
        b.start_node_at(
            Checkpoint {
                index: 9,
                parent: Some(0),
                depth: 1,
            },
            kind(2),
        );
    });
    assert_eq!(result, Err(BuildError::StaleCheckpoint { offset: size(0) }));
}

#[test]
fn the_first_mistake_is_the_one_reported() {
    let result = build("a", |b| {
        b.finish_node();
        b.token(kind(2), size(5));
        b.start_node(kind(0));
        let checkpoint = b.checkpoint();
        b.start_node_at(checkpoint, kind(1));
        b.finish_node();
    });
    assert_eq!(result, Err(BuildError::ExtraFinishNode { offset: size(0) }));
}

#[test]
fn every_error_explains_itself() {
    let errors = [
        BuildError::TextTooLong,
        BuildError::TooManyElements,
        BuildError::TokenOutsideNode { offset: size(1) },
        BuildError::TokenPastEnd { offset: size(1) },
        BuildError::TokenSplitsCharacter { offset: size(1) },
        BuildError::ExtraFinishNode { offset: size(1) },
        BuildError::MultipleRoots { offset: size(1) },
        BuildError::StaleCheckpoint { offset: size(1) },
        BuildError::UnclosedNodes { count: 2 },
        BuildError::NoRoot,
        BuildError::TextNotCovered { offset: size(1) },
    ];
    let messages: Vec<String> = errors.iter().map(ToString::to_string).collect();
    for (i, message) in messages.iter().enumerate() {
        assert!(!message.is_empty());
        assert!(
            !messages[i + 1..].contains(message),
            "duplicate message: {message}"
        );
    }
}

#[test]
fn a_tree_shows_its_size_but_not_its_storage() {
    let tree = ok(build("ab", |b| {
        b.start_node(kind(0));
        b.token(kind(2), size(2));
        b.finish_node();
    }));
    assert_eq!(
        format!("{tree:?}"),
        "SyntaxTree { text_len: 2, elements: 2 }"
    );
}

#[test]
fn a_builder_shows_its_progress_but_not_its_storage() {
    let mut builder = TreeBuilder::new("ab");
    builder.start_node(kind(0));
    builder.token(kind(2), size(1));
    assert_eq!(
        format!("{builder:?}"),
        "TreeBuilder { offset: TextSize(1), open_nodes: 1, error: None }"
    );
}

#[test]
fn the_checker_rejects_a_broken_tree() {
    // The properties below rely on `check`, so it must be able to fail.
    let tree = ok(build("ab", |b| {
        b.start_node(kind(0));
        b.token(kind(2), size(1));
        b.token(kind(2), size(1));
        b.finish_node();
    }));
    let mut entries = tree.entries().to_vec();
    entries[2].start = size(0);
    assert!(
        SyntaxTree::from_parts("ab", entries.clone())
            .check()
            .is_err()
    );
    entries[2].start = size(1);
    entries[0].len = size(1);
    assert!(
        SyntaxTree::from_parts("ab", entries.clone())
            .check()
            .is_err()
    );
    entries[0].len = size(2);
    entries[2].parent = Some(2);
    assert!(
        SyntaxTree::from_parts("ab", entries.clone())
            .check()
            .is_err()
    );
    assert!(SyntaxTree::from_parts("ab", Vec::new()).check().is_err());
}

/// One call a parser might make, for generating random call sequences.
#[derive(Debug, Clone)]
enum Call {
    Start(u16),
    Token(u16, u32),
    Finish,
    Checkpoint,
    StartAt(usize, u16),
}

fn call() -> impl Strategy<Value = Call> {
    prop_oneof![
        (0u16..4).prop_map(Call::Start),
        (0u16..4, 0u32..6).prop_map(|(k, len)| Call::Token(k, len)),
        Just(Call::Finish),
        Just(Call::Checkpoint),
        (0usize..4, 0u16..4).prop_map(|(i, k)| Call::StartAt(i, k)),
    ]
}

proptest! {
    // About 6% of random sequences end in a tree, so run enough to check
    // over a hundred trees each time.
    #![proptest_config(ProptestConfig::with_cases(2048))]

    /// Any sequence of calls gives a tree that passes its own check and
    /// round-trips, or a clean error. Nothing panics. Half the sequences are
    /// completed, so that random checkpoints and wraps produce real trees for
    /// the check to inspect, not only errors.
    #[test]
    fn any_calls_give_a_valid_tree_or_an_error(
        text in "[a-c é\n]{0,12}",
        calls in proptest::collection::vec(call(), 0..40),
        complete in any::<bool>(),
    ) {
        if let Ok(tree) = run_calls(&text, &calls, complete) {
            prop_assert_eq!(tree.check(), Ok(()));
            prop_assert_eq!(joined(&tree), text);
        }
    }

    /// Well-formed calls always succeed, and joining the tokens gives back
    /// the input exactly.
    #[test]
    fn well_formed_calls_round_trip_the_text(
        text in "\\PC{0,24}",
        choices in proptest::collection::vec(0u8..8, 0..200),
    ) {
        let tree = build_well_formed(&text, &choices);
        let tree = match tree {
            Ok(tree) => tree,
            Err(error) => return Err(TestCaseError::fail(format!("{error}"))),
        };
        prop_assert_eq!(tree.check(), Ok(()));
        prop_assert_eq!(joined(&tree), text);
    }
}

/// Makes the calls in order. With `complete`, it first opens a root, and
/// afterwards adds a token for any text left and closes every open node, so
/// many random sequences end in a tree rather than an error.
fn run_calls(text: &str, calls: &[Call], complete: bool) -> Result<SyntaxTree, BuildError> {
    let mut builder = TreeBuilder::new(text);
    if complete {
        builder.start_node(kind(0));
    }
    let mut checkpoints = Vec::new();
    for call in calls {
        match *call {
            Call::Start(k) => builder.start_node(kind(k)),
            Call::Token(k, len) => builder.token(kind(k), size(len)),
            Call::Finish => builder.finish_node(),
            Call::Checkpoint => checkpoints.push(builder.checkpoint()),
            Call::StartAt(i, k) => {
                if let Some(&checkpoint) = checkpoints.get(i) {
                    builder.start_node_at(checkpoint, kind(k));
                }
            }
        }
    }
    if complete {
        let rest = builder
            .text_len
            .checked_sub(builder.pos)
            .unwrap_or(TextSize::ZERO);
        builder.token(kind(3), rest);
        for _ in 0..builder.open.len() {
            builder.finish_node();
        }
    }
    builder.finish()
}

/// Drives the builder with calls that are always valid, steered by `choices`:
/// one token per character, with nodes opened, closed and wrapped around them
/// at random.
fn build_well_formed(text: &str, choices: &[u8]) -> Result<SyntaxTree, BuildError> {
    let mut choices = choices.iter().copied().cycle().chain(std::iter::repeat(0));
    let mut next = move || choices.next().unwrap_or(0);
    let mut builder = TreeBuilder::new(text);
    let root = builder.checkpoint();
    builder.start_node(kind(0));
    // Checkpoints taken at each open depth; closing a node drops its own.
    let mut levels: Vec<Vec<Checkpoint>> = vec![Vec::new()];
    for ch in text.chars() {
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
            _ => {}
        }
        let len = u32::try_from(ch.len_utf8()).unwrap_or(0);
        builder.token(kind(3), size(len));
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
