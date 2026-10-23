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

/// A random origin whose span is `len` bytes long.
fn origin(kind: u8, text: u8, start: u32, len: u32) -> Origin {
    let from = Span::new(
        TextId::File(FileId::new(u32::from(text))),
        range(start, start + len),
    );
    let site = span(MAIN, 100, 110);
    match kind % 3 {
        0 => Origin::Source { from },
        1 => Origin::Included { from, site },
        _ => Origin::Expanded {
            from,
            call_site: site,
        },
    }
}

proptest! {
    /// Any pieces build an expansion whose text is the pieces joined, and
    /// whose lookups agree with a linear search through the pieces.
    #[test]
    fn lookups_agree_with_a_linear_search(
        parts in prop::collection::vec(("[a-zé€ ]{0,6}", 0u8..3, 0u8..3, 0u32..50), 0..12),
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
