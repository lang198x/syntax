//! Turning source bytes into text, and back.
//!
//! Many vintage tools read source as bytes, not as UTF-8 text. ca65 and
//! SjASMPlus, for example, copy a byte from `$80` to `$FF` in a string into
//! the program as it is. Trees hold UTF-8 text, so a front end decodes the
//! bytes first, with the character set its tool reads.
//!
//! Decoding never loses anything. [`Decoded`] keeps a map between offsets in
//! the text and offsets in the bytes, so a diagnostic can point at the byte
//! a person wrote, and [`Decoded::encode`] gives back the exact bytes.
//!
//! The rules are in Lang198x's record on
//! [settings that change how source is read](https://github.com/lang198x/lang198x/blob/main/decisions/reading-settings.md).

use std::fmt;

use crate::TextSize;

/// A character set that source can arrive in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CharSet {
    /// ISO 8859-1. Each byte is the character with the same number, so
    /// every byte sequence decodes, and decodes back to itself.
    Latin1,
}

/// Source bytes decoded into text, with a map back to the bytes.
///
/// ```
/// use lang198x_syntax::{CharSet, TextSize, decode};
///
/// // `é` in Latin-1 is one byte, $E9.
/// let decoded = decode(b"db \"\xE9\"", CharSet::Latin1)?;
/// assert_eq!(decoded.text(), "db \"é\"");
/// // In the text, `é` takes two bytes, so the closing quote moves.
/// assert_eq!(decoded.byte_offset(TextSize::from(6_u32)), Some(5));
/// assert_eq!(decoded.encode(), b"db \"\xE9\"");
/// # Ok::<(), lang198x_syntax::DecodeError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    text: String,
    set: CharSet,
    /// The byte offset of each character whose text is longer than its
    /// bytes, in order. The offsets between them are worked out from these.
    wide: Vec<usize>,
}

/// Decodes `bytes` as `set`.
///
/// # Errors
///
/// [`DecodeError::TooLong`] if the text would be longer than 4 GiB, the
/// most a tree can hold.
pub fn decode(bytes: &[u8], set: CharSet) -> Result<Decoded, DecodeError> {
    match set {
        CharSet::Latin1 => {
            let wide: Vec<usize> = bytes
                .iter()
                .enumerate()
                .filter(|(_, byte)| !byte.is_ascii())
                .map(|(at, _)| at)
                .collect();
            let length = bytes.len().saturating_add(wide.len());
            if u32::try_from(length).is_err() {
                return Err(DecodeError::TooLong);
            }
            let text = bytes.iter().map(|&byte| char::from(byte)).collect();
            Ok(Decoded { text, set, wide })
        }
    }
}

/// Encodes `text` as `set`, as a tool that reads `set` would see it.
///
/// # Errors
///
/// [`EncodeError::Unencodable`] for the first character `set` cannot hold.
///
/// ```
/// use lang198x_syntax::{CharSet, encode};
///
/// assert_eq!(encode("é", CharSet::Latin1), Ok(vec![0xE9]));
/// assert!(encode("€", CharSet::Latin1).is_err());
/// ```
pub fn encode(text: &str, set: CharSet) -> Result<Vec<u8>, EncodeError> {
    match set {
        CharSet::Latin1 => text
            .char_indices()
            .map(|(at, character)| {
                u8::try_from(u32::from(character)).map_err(|_| EncodeError::Unencodable {
                    character,
                    offset: TextSize::try_from(at).unwrap_or(TextSize::ZERO),
                })
            })
            .collect(),
    }
}

impl Decoded {
    /// Returns the text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the text, giving up the map.
    #[must_use]
    pub fn into_text(self) -> String {
        self.text
    }

    /// Returns the character set the bytes were decoded from.
    #[must_use]
    pub fn char_set(&self) -> CharSet {
        self.set
    }

    /// Returns the bytes the text was decoded from, exactly.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        match self.set {
            CharSet::Latin1 => self
                .text
                .chars()
                .map(|character| u8::try_from(u32::from(character)).unwrap_or(b'?'))
                .collect(),
        }
    }

    /// Returns the offset in the bytes of the character that starts at `at`
    /// in the text, or the end of the bytes for the end of the text. `None`
    /// if `at` is inside a character or past the end.
    ///
    /// ```
    /// use lang198x_syntax::{CharSet, TextSize, decode};
    ///
    /// let decoded = decode(b"\xE9a", CharSet::Latin1)?;
    /// assert_eq!(decoded.byte_offset(TextSize::from(2_u32)), Some(1));
    /// assert_eq!(decoded.byte_offset(TextSize::from(1_u32)), None);
    /// # Ok::<(), lang198x_syntax::DecodeError>(())
    /// ```
    #[must_use]
    pub fn byte_offset(&self, at: TextSize) -> Option<usize> {
        let at = usize::try_from(u32::from(at)).ok()?;
        if at > self.text.len() || !self.text.is_char_boundary(at) {
            return None;
        }
        // Each wide character before `at` adds one byte to the text.
        // `before` counts them: the wide characters whose text starts
        // before `at`.
        let before = self
            .wide
            .partition_point(|&byte| byte + self.extra_before(byte) < at);
        Some(at - before)
    }

    /// Returns the offset in the text of the character that starts at byte
    /// `at`, or the end of the text for the end of the bytes. `None` past
    /// the end.
    ///
    /// ```
    /// use lang198x_syntax::{CharSet, TextSize, decode};
    ///
    /// let decoded = decode(b"\xE9a", CharSet::Latin1)?;
    /// assert_eq!(decoded.text_offset(1), Some(TextSize::from(2_u32)));
    /// # Ok::<(), lang198x_syntax::DecodeError>(())
    /// ```
    #[must_use]
    pub fn text_offset(&self, at: usize) -> Option<TextSize> {
        let bytes = self.text.len().checked_sub(self.wide.len())?;
        if at > bytes {
            return None;
        }
        let offset = at + self.extra_before(at);
        TextSize::try_from(offset).ok()
    }

    /// How many bytes the text gains before byte `at`: one for each wide
    /// character before it.
    fn extra_before(&self, at: usize) -> usize {
        self.wide.partition_point(|&byte| byte < at)
    }
}

/// Why bytes could not be decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DecodeError {
    /// The text would be longer than 4 GiB.
    TooLong,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong => f.write_str("the decoded text would be longer than 4 GiB"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Why text could not be encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EncodeError {
    /// A character the character set cannot hold.
    Unencodable {
        /// The character.
        character: char,
        /// Where it starts in the text.
        offset: TextSize,
    },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unencodable { character, offset } => write!(
                f,
                "the character {character:?} at {offset} has no byte in this character set"
            ),
        }
    }
}

impl std::error::Error for EncodeError {}

#[cfg(test)]
mod tests;
