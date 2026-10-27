use proptest::prelude::*;

use super::*;

const MAIN: TextId = TextId::File(FileId::new(0));
const LIB: TextId = TextId::File(FileId::new(1));

fn range(start: u32, end: u32) -> TextRange {
    TextRange::new(TextSize::new(start), TextSize::new(end))
        .expect("the end is not before the start")
}

fn span(text: TextId, start: u32, end: u32) -> Span {
    Span::new(text, range(start, end))
}

fn built(pieces: &[(&str, Origin)]) -> Result<Expansion, ProvenanceError> {
    let mut builder = ExpansionBuilder::new();
    for &(piece, origin) in pieces {
        builder.push(piece, origin);
    }
    builder.finish()
}

#[test]
fn ids_and_spans_keep_what_they_were_given() {
    assert_eq!(FileId::new(7).get(), 7);
    assert_eq!(ExpansionId::new(9).get(), 9);
    let s = span(LIB, 2, 5);
    assert_eq!(s.text(), LIB);
    assert_eq!(s.range(), range(2, 5));
}

#[test]
fn each_origin_says_where_it_was_copied_from_and_why() {
    let from = span(MAIN, 0, 3);
    let site = span(MAIN, 10, 20);
    assert_eq!(Origin::Source { from }.copied_from(), Some(from));
    assert_eq!(Origin::Source { from }.site(), None);
    let included = Origin::Included { from, site };
    assert_eq!(
        (included.copied_from(), included.site()),
        (Some(from), Some(site))
    );
    let expanded = Origin::Expanded {
        from,
        call_site: site,
    };
    assert_eq!(
        (expanded.copied_from(), expanded.site()),
        (Some(from), Some(site))
    );
    assert_eq!(expanded.made_from(), None);
    let synthesised = Origin::Synthesised {
        inputs: from,
        call_site: site,
    };
    assert_eq!(
        (
            synthesised.copied_from(),
            synthesised.made_from(),
            synthesised.site()
        ),
        (None, Some(from), Some(site))
    );
}

#[test]
fn synthesised_text_has_no_single_writer_but_keeps_its_call() {
    // `.concat("a","b")` at 0..16 in the file makes `"ab"`, in a macro
    // expansion whose call is at 40..45.
    let call = span(MAIN, 40, 45);
    let mut builder = ExpansionBuilder::new();
    builder.push(
        "\"ab\"",
        Origin::Synthesised {
            inputs: span(MAIN, 0, 16),
            call_site: call,
        },
    );
    builder.push(
        " ;",
        Origin::Expanded {
            from: span(MAIN, 16, 18),
            call_site: call,
        },
    );
    let mut provenance = Provenance::new();
    let id = provenance
        .add(builder.finish().expect("a made-up piece may be any length"))
        .expect("its spans point at the file");
    let text = TextId::Expansion(id);
    assert_eq!(provenance.trace(span(text, 0, 4)), None);
    assert_eq!(provenance.trace(span(text, 4, 6)), Some(span(MAIN, 16, 18)));
    assert_eq!(provenance.sites(span(text, 1, 2)), [call]);
    // A synthesised piece's inputs must point backwards, too.
    let mut builder = ExpansionBuilder::new();
    builder.push(
        "1",
        Origin::Synthesised {
            inputs: span(TextId::Expansion(ExpansionId::new(5)), 0, 1),
            call_site: call,
        },
    );
    let forward = provenance.add(builder.finish().expect("builds"));
    assert!(matches!(
        forward,
        Err(ProvenanceError::ForwardReference { .. })
    ));
}

#[test]
fn pieces_join_into_the_text_and_keep_their_origins() {
    let call_site = span(MAIN, 30, 38);
    let body = Origin::Expanded {
        from: span(MAIN, 8, 13),
        call_site,
    };
    let arg = Origin::Expanded {
        from: span(MAIN, 37, 38),
        call_site,
    };
    let tail = Origin::Source {
        from: span(LIB, 0, 1),
    };
    let expansion =
        built(&[("ld a,", body), ("", tail), ("2", arg), ("\n", tail)]).expect("an expansion");
    assert_eq!(expansion.text(), "ld a,2\n");
    let origins: Vec<_> = (0..8)
        .map(|o| expansion.origin_at(TextSize::new(o)))
        .collect();
    assert_eq!(
        origins,
        [
            Some(body),
            Some(body),
            Some(body),
            Some(body),
            Some(body),
            Some(arg),
            Some(tail),
            None
        ]
    );
}

#[test]
fn an_empty_expansion_has_no_pieces() {
    let expansion = ExpansionBuilder::new().finish().expect("an expansion");
    assert_eq!(expansion.text(), "");
    assert_eq!(expansion.origin_at(TextSize::ZERO), None);
    assert_eq!(expansion.pieces(range(0, 0)).count(), 0);
}

#[test]
fn pieces_overlapping_a_range() {
    let a = Origin::Source {
        from: span(MAIN, 0, 3),
    };
    let b = Origin::Source {
        from: span(MAIN, 10, 12),
    };
    let c = Origin::Source {
        from: span(MAIN, 20, 24),
    };
    let expansion = built(&[("abc", a), ("de", b), ("fghi", c)]).expect("an expansion");
    let pieces = |start, end| expansion.pieces(range(start, end)).collect::<Vec<_>>();
    assert_eq!(
        pieces(0, 9),
        [(range(0, 3), a), (range(3, 5), b), (range(5, 9), c)]
    );
    assert_eq!(pieces(2, 4), [(range(0, 3), a), (range(3, 5), b)]);
    assert_eq!(pieces(3, 5), [(range(3, 5), b)]);
    // An empty range touches only the piece that holds its offset.
    assert_eq!(pieces(3, 3), [(range(3, 5), b)]);
    assert_eq!(pieces(4, 4), [(range(3, 5), b)]);
    assert_eq!(pieces(9, 9), []);
    assert_eq!(pieces(9, 12), []);
}

#[test]
fn a_piece_must_be_as_long_as_its_span() {
    let error = built(&[
        (
            "ok",
            Origin::Source {
                from: span(MAIN, 0, 2),
            },
        ),
        (
            "abc",
            Origin::Source {
                from: span(MAIN, 0, 2),
            },
        ),
    ]);
    let expected = ProvenanceError::LengthMismatch {
        offset: TextSize::new(2),
        piece: 3,
        span: TextSize::new(2),
    };
    assert_eq!(error, Err(expected));
    assert_eq!(
        expected.to_string(),
        "the piece at offset 2 is 3 bytes long, but the span it was copied from is 2"
    );
}

#[test]
fn the_first_error_is_kept_and_later_pushes_are_ignored() {
    let mut builder = ExpansionBuilder::new();
    builder.push(
        "abc",
        Origin::Source {
            from: span(MAIN, 0, 1),
        },
    );
    builder.push(
        "x",
        Origin::Source {
            from: span(MAIN, 0, 1),
        },
    );
    builder.push(
        "yyyy",
        Origin::Source {
            from: span(MAIN, 0, 2),
        },
    );
    let Err(ProvenanceError::LengthMismatch { offset, piece, .. }) = builder.finish() else {
        panic!("expected the first mismatch");
    };
    assert_eq!((offset, piece), (TextSize::ZERO, 3));
}

#[test]
fn errors_describe_themselves() {
    assert_eq!(
        ProvenanceError::TextTooLong.to_string(),
        "the expansion is longer than 4 GiB"
    );
    let error: &dyn std::error::Error = &ProvenanceError::TextTooLong;
    assert!(error.source().is_none());
}

fn expansion_id(text: TextId) -> ExpansionId {
    match text {
        TextId::Expansion(id) => id,
        TextId::File(_) => panic!("not an expansion"),
    }
}

/// `main` calls `outer` at 40..45, whose body calls `inner`, whose body is
/// `db 1` and includes a library line.
fn nested() -> (Provenance, TextId, TextId) {
    let mut provenance = Provenance::new();
    let call = span(MAIN, 40, 45);
    let outer = built(&[(
        " inner\n",
        Origin::Expanded {
            from: span(MAIN, 20, 27),
            call_site: call,
        },
    )])
    .expect("an expansion");
    let outer = TextId::Expansion(provenance.add(outer).expect("added"));
    let call = span(outer, 0, 6);
    let include_site = span(MAIN, 12, 30);
    let inner = built(&[
        (
            " db 1\n",
            Origin::Expanded {
                from: span(MAIN, 6, 12),
                call_site: call,
            },
        ),
        (
            "ret",
            Origin::Included {
                from: span(LIB, 4, 7),
                site: include_site,
            },
        ),
    ])
    .expect("an expansion");
    let inner = TextId::Expansion(provenance.add(inner).expect("added"));
    (provenance, outer, inner)
}

#[test]
fn tracing_a_nested_expansion_reaches_the_file() {
    let (provenance, outer, inner) = nested();
    assert_eq!(expansion_id(outer).get(), 0);
    assert_eq!(expansion_id(inner).get(), 1);
    assert_eq!(
        provenance.get(expansion_id(inner)).map(Expansion::text),
        Some(" db 1\nret")
    );
    assert!(provenance.get(ExpansionId::new(2)).is_none());

    assert_eq!(provenance.trace(span(inner, 1, 3)), Some(span(MAIN, 7, 9)));
    assert_eq!(provenance.trace(span(inner, 6, 9)), Some(span(LIB, 4, 7)));
    assert_eq!(
        provenance.trace(span(inner, 9, 9)),
        None,
        "the end of the text holds no piece"
    );
    assert_eq!(
        provenance.trace(span(outer, 1, 6)),
        Some(span(MAIN, 21, 26))
    );
    assert_eq!(provenance.trace(span(MAIN, 3, 4)), Some(span(MAIN, 3, 4)));
}

#[test]
fn a_span_across_two_pieces_has_no_single_origin() {
    let (provenance, _, inner) = nested();
    assert_eq!(provenance.trace(span(inner, 4, 8)), None);
    let parts: Vec<_> = provenance
        .get(expansion_id(inner))
        .map(|e| e.pieces(range(4, 8)).map(|(r, _)| r).collect())
        .unwrap_or_default();
    assert_eq!(parts, [range(0, 6), range(6, 9)]);
}

#[test]
fn sites_run_from_the_innermost_call_outwards() {
    let (provenance, outer, inner) = nested();
    assert_eq!(
        provenance.sites(span(inner, 1, 3)),
        [span(outer, 0, 6), span(MAIN, 40, 45)]
    );
    assert_eq!(provenance.sites(span(inner, 6, 9)), [span(MAIN, 12, 30)]);
    assert_eq!(provenance.sites(span(outer, 0, 1)), [span(MAIN, 40, 45)]);
    assert_eq!(provenance.sites(span(MAIN, 0, 1)), []);
    assert_eq!(
        provenance.sites(span(inner, 50, 51)),
        [],
        "outside the text"
    );
}

#[test]
fn a_plain_copy_of_an_expansion_passes_its_sites_through() {
    let (mut provenance, _, inner) = nested();
    let copy = built(&[(
        "db",
        Origin::Source {
            from: span(inner, 1, 3),
        },
    )])
    .expect("an expansion");
    let copy = TextId::Expansion(provenance.add(copy).expect("added"));
    assert_eq!(provenance.trace(span(copy, 0, 2)), Some(span(MAIN, 7, 9)));
    assert_eq!(provenance.sites(span(copy, 0, 2)).len(), 2);
}

#[test]
fn an_expansion_may_point_only_backwards_and_inside() {
    let mut provenance = Provenance::new();
    let itself = span(TextId::Expansion(ExpansionId::new(0)), 0, 1);
    let refused = built(&[("a", Origin::Source { from: itself })]).expect("an expansion");
    assert_eq!(
        provenance.add(refused),
        Err(ProvenanceError::ForwardReference { span: itself })
    );

    let first = built(&[(
        "ab",
        Origin::Source {
            from: span(MAIN, 0, 2),
        },
    )])
    .expect("an expansion");
    let first = TextId::Expansion(provenance.add(first).expect("added"));
    let past_end = span(first, 1, 3);
    let refused = built(&[("xy", Origin::Source { from: past_end })]).expect("an expansion");
    assert_eq!(
        provenance.add(refused),
        Err(ProvenanceError::OutsideText { span: past_end })
    );

    let forward_site = span(TextId::Expansion(ExpansionId::new(5)), 0, 0);
    let refused = built(&[(
        "c",
        Origin::Expanded {
            from: span(first, 0, 1),
            call_site: forward_site,
        },
    )])
    .expect("an expansion");
    assert_eq!(
        provenance.add(refused),
        Err(ProvenanceError::ForwardReference { span: forward_site })
    );
    assert!(
        provenance.get(ExpansionId::new(1)).is_none(),
        "refused expansions are not added"
    );
}

#[test]
fn registry_errors_describe_themselves() {
    let forward = ProvenanceError::ForwardReference {
        span: span(TextId::Expansion(ExpansionId::new(3)), 0, 1),
    };
    assert_eq!(
        forward.to_string(),
        "a span points at Expansion(ExpansionId(3)), which is not an earlier expansion"
    );
    let outside = ProvenanceError::OutsideText {
        span: span(TextId::Expansion(ExpansionId::new(0)), 1, 3),
    };
    assert_eq!(
        outside.to_string(),
        "a span at 1..3 runs past the end of Expansion(ExpansionId(0))"
    );
    assert_eq!(
        ProvenanceError::TooManyExpansions.to_string(),
        "there are more than four billion expansions"
    );
}

/// A random origin whose span is `len` bytes long.
fn origin(kind: u8, text: u8, start: u32, len: u32) -> Origin {
    let from = Span::new(
        TextId::File(FileId::new(u32::from(text))),
        range(start, start + len),
    );
    let site = span(MAIN, 100, 110);
    match kind % 4 {
        0 => Origin::Source { from },
        1 => Origin::Included { from, site },
        2 => Origin::Expanded {
            from,
            call_site: site,
        },
        _ => Origin::Synthesised {
            inputs: from,
            call_site: site,
        },
    }
}

proptest! {
    /// Any pieces build an expansion whose text is the pieces joined, and
    /// whose lookups agree with a linear search through the pieces.
    #[test]
    fn lookups_agree_with_a_linear_search(
        parts in prop::collection::vec(("[a-zé€ ]{0,6}", 0u8..4, 0u8..3, 0u32..50), 0..12),
        probe in 0u32..80,
        probe_len in 0u32..20,
    ) {
        let mut builder = ExpansionBuilder::new();
        let mut expected = Vec::new();
        let mut offset = 0u32;
        for (piece, kind, text, start) in &parts {
            let len = u32::try_from(piece.len()).expect("short");
            let origin = origin(*kind, *text, *start, len);
            builder.push(piece, origin);
            if len > 0 {
                expected.push((range(offset, offset + len), origin));
            }
            offset += len;
        }
        let expansion = builder.finish().expect("matching lengths always build");
        let joined: String = parts.iter().map(|(p, ..)| p.as_str()).collect();
        prop_assert_eq!(expansion.text(), joined.as_str());

        for at in 0..=offset + 1 {
            let linear = expected.iter().find(|(r, _)| r.contains(TextSize::new(at))).map(|&(_, o)| o);
            prop_assert_eq!(expansion.origin_at(TextSize::new(at)), linear);
        }

        let query = range(probe, probe + probe_len);
        let linear: Vec<_> = expected
            .iter()
            .copied()
            .filter(|(r, _)| {
                if query.is_empty() {
                    r.contains(query.start())
                } else {
                    r.start() < query.end() && query.start() < r.end()
                }
            })
            .collect();
        prop_assert_eq!(expansion.pieces(query).collect::<Vec<_>>(), linear);
    }
}

/// One step of building a random expansion: copy `len` bytes starting at
/// `start` from text number `source`, scaled to fit what exists.
#[derive(Debug, Clone)]
struct Copy {
    source: usize,
    start: usize,
    len: usize,
    kind: u8,
}

proptest! {
    /// Expansions that really copy text from files and earlier expansions
    /// trace back to the very characters they copied, and every chain ends
    /// in a file.
    #[test]
    fn tracing_lands_on_the_characters_that_were_copied(
        files in prop::collection::vec("[a-z ]{1,30}", 1..4),
        plans in prop::collection::vec(
            prop::collection::vec(
                (0usize..100, 0usize..100, 1usize..8, 0u8..3).prop_map(|(source, start, len, kind)| Copy { source, start, len, kind }),
                1..6,
            ),
            1..6,
        ),
    ) {
        let mut provenance = Provenance::new();
        // Texts so far: files first, then each expansion as it is added.
        let mut texts: Vec<(TextId, String)> = files
            .iter()
            .enumerate()
            .map(|(i, text)| (TextId::File(FileId::new(u32::try_from(i).expect("few"))), text.clone()))
            .collect();
        for plan in &plans {
            let mut builder = ExpansionBuilder::new();
            for copy in plan {
                let (source, text) = &texts[copy.source % texts.len()];
                let start = copy.start % text.len();
                let len = copy.len.min(text.len() - start);
                let piece = &text[start..start + len];
                let from = span(*source, u32::try_from(start).expect("short"), u32::try_from(start + len).expect("short"));
                let site = span(texts[copy.start % texts.len()].0, 0, 0);
                let origin = match copy.kind {
                    0 => Origin::Source { from },
                    1 => Origin::Included { from, site },
                    _ => Origin::Expanded { from, call_site: site },
                };
                builder.push(piece, origin);
            }
            let expansion = builder.finish().expect("copied pieces match their spans");
            let text = expansion.text().to_owned();
            let id = provenance.add(expansion).expect("only backward references");
            texts.push((TextId::Expansion(id), text));
        }

        let file_text = |id: TextId| texts.iter().find(|(t, _)| *t == id).map(|(_, s)| s.as_str());
        for (id, text) in texts.iter().filter(|(t, _)| matches!(t, TextId::Expansion(_))) {
            for at in 0..text.len() {
                let at32 = u32::try_from(at).expect("short");
                let traced = provenance.trace(span(*id, at32, at32 + 1));
                let traced = traced.expect("one byte always sits inside one piece");
                prop_assert!(matches!(traced.text(), TextId::File(_)));
                let written = file_text(traced.text()).and_then(|t| traced.range().slice(t));
                prop_assert_eq!(written, Some(&text[at..=at]));
                prop_assert!(provenance.sites(span(*id, at32, at32 + 1)).len() <= plans.len() * 6);
            }
        }
    }
}
