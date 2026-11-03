use proptest::prelude::*;

use super::*;

#[test]
fn ascii_decodes_as_itself() {
    let decoded = decode(b"lda #1 ; hi", CharSet::Latin1).expect("decodes");
    assert_eq!(decoded.text(), "lda #1 ; hi");
    assert_eq!(decoded.char_set(), CharSet::Latin1);
    for at in 0..=11_u32 {
        assert_eq!(decoded.byte_offset(TextSize::from(at)), Some(at as usize));
        assert_eq!(decoded.text_offset(at as usize), Some(TextSize::from(at)));
    }
}

#[test]
fn every_byte_is_its_own_character() {
    let bytes: Vec<u8> = (0..=255).collect();
    let decoded = decode(&bytes, CharSet::Latin1).expect("decodes");
    for (character, byte) in decoded.text().chars().zip(&bytes) {
        assert_eq!(u32::from(character), u32::from(*byte));
    }
    // 128 one-byte characters and 128 two-byte ones.
    assert_eq!(decoded.text().len(), 128 + 2 * 128);
    assert_eq!(decoded.encode(), bytes);
    assert_eq!(encode(decoded.text(), CharSet::Latin1), Ok(bytes));
}

#[test]
fn offsets_map_both_ways_around_wide_characters() {
    // "a" $E9 "b" $FF $FF "c"
    let decoded = decode(b"a\xE9b\xFF\xFFc", CharSet::Latin1).expect("decodes");
    assert_eq!(decoded.text(), "aébÿÿc");
    let pairs = [(0, 0), (1, 1), (3, 2), (4, 3), (6, 4), (8, 5), (9, 6)];
    for (text, byte) in pairs {
        assert_eq!(
            decoded.byte_offset(TextSize::from(text)),
            Some(byte),
            "text {text}"
        );
        assert_eq!(
            decoded.text_offset(byte),
            Some(TextSize::from(text)),
            "byte {byte}"
        );
    }
    // Inside a character, and past either end.
    for inside in [2_u32, 5, 7, 10] {
        assert_eq!(
            decoded.byte_offset(TextSize::from(inside)),
            None,
            "{inside}"
        );
    }
    assert_eq!(decoded.text_offset(7), None);
}

#[test]
fn empty_bytes_are_empty_text() {
    let decoded = decode(b"", CharSet::Latin1).expect("decodes");
    assert_eq!(decoded.clone().into_text(), "");
    assert_eq!(decoded.byte_offset(TextSize::ZERO), Some(0));
    assert_eq!(decoded.text_offset(0), Some(TextSize::ZERO));
    assert!(decoded.encode().is_empty());
}

#[test]
fn a_character_outside_the_set_is_refused_where_it_is() {
    let refused = encode("ab€c", CharSet::Latin1);
    assert_eq!(
        refused,
        Err(EncodeError::Unencodable {
            character: '€',
            offset: TextSize::from(2_u32),
        })
    );
    let message = refused.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(
        message.contains("'€'") && message.contains('2'),
        "{message}"
    );
    assert!(DecodeError::TooLong.to_string().contains("4 GiB"));
}

proptest! {
    /// Decoding never loses a byte, and every byte offset maps to a text
    /// offset that maps back.
    #[test]
    fn decoding_round_trips(bytes in proptest::collection::vec(any::<u8>(), 0..300)) {
        let decoded = decode(&bytes, CharSet::Latin1).expect("decodes");
        prop_assert_eq!(decoded.encode(), bytes.clone());
        prop_assert_eq!(encode(decoded.text(), CharSet::Latin1), Ok(bytes.clone()));
        for at in 0..=bytes.len() {
            let text = decoded.text_offset(at).expect("in range");
            prop_assert_eq!(decoded.byte_offset(text), Some(at));
        }
    }
}
