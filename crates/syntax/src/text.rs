//! Positions and ranges in source text.
//!
//! Every token and node in a syntax tree records where it sits in the source,
//! as a byte offset and a length. These two types carry those numbers.
//!
//! Offsets are 32-bit, so a single source file can be up to 4 GiB. That is far
//! more than any vintage program needs, and it halves the size of every range
//! compared with `usize` on a 64-bit host.

use std::fmt;

/// A byte offset into source text, or a length measured in bytes.
///
/// Offsets count bytes of UTF-8, not characters. A range that starts or ends
/// inside a multi-byte character cannot be used to slice text; see
/// [`TextRange::slice`].
///
/// ```
/// use lang198x_syntax::TextSize;
///
/// let len = TextSize::of("LD A,(HL)").expect("a short string fits");
/// assert_eq!(len.get(), 9);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TextSize(u32);

impl TextSize {
    /// The offset of the first byte, and the length of empty text.
    pub const ZERO: Self = Self(0);

    /// Makes a size from a raw byte count.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw byte count.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Returns the length of `text` in bytes.
    ///
    /// Returns `None` if the text is longer than 4 GiB, which is the most a
    /// 32-bit offset can address.
    #[must_use]
    pub fn of(text: &str) -> Option<Self> {
        u32::try_from(text.len()).ok().map(Self)
    }

    /// Adds two sizes, or returns `None` if the result would not fit.
    #[must_use]
    pub const fn checked_add(self, other: Self) -> Option<Self> {
        match self.0.checked_add(other.0) {
            Some(raw) => Some(Self(raw)),
            None => None,
        }
    }

    /// Subtracts `other` from this size, or returns `None` if `other` is
    /// larger.
    #[must_use]
    pub const fn checked_sub(self, other: Self) -> Option<Self> {
        match self.0.checked_sub(other.0) {
            Some(raw) => Some(Self(raw)),
            None => None,
        }
    }

    /// Converts the size to a `usize`, for indexing into a `str`.
    #[must_use]
    pub fn to_usize(self) -> usize {
        // Every host this crate supports has a pointer of at least 32 bits, so
        // a u32 always fits. On a smaller target this fails to compile, which
        // is better than truncating silently.
        const _: () = assert!(usize::BITS >= u32::BITS);
        self.0 as usize
    }
}

impl From<u32> for TextSize {
    fn from(raw: u32) -> Self {
        Self(raw)
    }
}

impl From<TextSize> for u32 {
    fn from(size: TextSize) -> Self {
        size.0
    }
}

impl TryFrom<usize> for TextSize {
    type Error = std::num::TryFromIntError;

    fn try_from(raw: usize) -> Result<Self, Self::Error> {
        u32::try_from(raw).map(Self)
    }
}

impl fmt::Display for TextSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A span of source text, from `start` up to but not including `end`.
///
/// A range can be empty, with `start` equal to `end`. An empty range still has
/// a position, which is how a diagnostic points at something missing, such as
/// an operand that should have followed a comma.
///
/// ```
/// use lang198x_syntax::{TextRange, TextSize};
///
/// let source = "start: nop";
/// let label = TextRange::new(TextSize::new(0), TextSize::new(5))
///     .expect("the end is not before the start");
/// assert_eq!(label.slice(source), Some("start"));
/// assert_eq!(label.len().get(), 5);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextRange {
    start: TextSize,
    end: TextSize,
}

impl TextRange {
    /// Makes a range from `start` to `end`.
    ///
    /// Returns `None` if `end` is before `start`.
    #[must_use]
    pub const fn new(start: TextSize, end: TextSize) -> Option<Self> {
        if end.0 < start.0 {
            None
        } else {
            Some(Self { start, end })
        }
    }

    /// Makes a range of `len` bytes beginning at `start`.
    ///
    /// Returns `None` if the end would lie beyond 4 GiB.
    #[must_use]
    pub const fn at(start: TextSize, len: TextSize) -> Option<Self> {
        match start.checked_add(len) {
            Some(end) => Some(Self { start, end }),
            None => None,
        }
    }

    /// Makes an empty range at `offset`.
    #[must_use]
    pub const fn empty(offset: TextSize) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }

    /// Returns the offset of the first byte in the range.
    #[must_use]
    pub const fn start(self) -> TextSize {
        self.start
    }

    /// Returns the offset just past the last byte in the range.
    #[must_use]
    pub const fn end(self) -> TextSize {
        self.end
    }

    /// Returns the number of bytes in the range.
    #[must_use]
    pub const fn len(self) -> TextSize {
        // `new` and `at` guarantee that `end` is not before `start`.
        TextSize(self.end.0 - self.start.0)
    }

    /// Returns `true` if the range covers no bytes.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start.0 == self.end.0
    }

    /// Returns `true` if `offset` falls inside the range.
    ///
    /// The end is excluded, so an empty range contains no offsets.
    #[must_use]
    pub const fn contains(self, offset: TextSize) -> bool {
        self.start.0 <= offset.0 && offset.0 < self.end.0
    }

    /// Returns `true` if `other` lies entirely inside this range.
    ///
    /// An empty range is inside this one if its position is between `start`
    /// and `end`, including at `end`.
    #[must_use]
    pub const fn contains_range(self, other: Self) -> bool {
        self.start.0 <= other.start.0 && other.end.0 <= self.end.0
    }

    /// Returns the smallest range that covers both this range and `other`.
    ///
    /// A parent node uses this to span all of its children.
    #[must_use]
    pub fn cover(self, other: Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Returns the part of `text` this range covers.
    ///
    /// Returns `None` if the range runs past the end of `text`, or if either
    /// end falls inside a multi-byte character. Slicing never panics.
    #[must_use]
    pub fn slice(self, text: &str) -> Option<&str> {
        text.get(self.start.to_usize()..self.end.to_usize())
    }
}

impl fmt::Display for TextRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(raw: u32) -> TextSize {
        TextSize::new(raw)
    }

    fn range(start: u32, end: u32) -> TextRange {
        TextRange::new(size(start), size(end)).expect("test ranges are ordered")
    }

    #[test]
    fn a_size_measures_bytes_not_characters() {
        assert_eq!(TextSize::of(""), Some(TextSize::ZERO));
        // "£" is two bytes of UTF-8.
        assert_eq!(TextSize::of("£1"), Some(size(3)));
    }

    #[test]
    fn size_arithmetic_refuses_to_overflow() {
        assert_eq!(size(2).checked_add(size(3)), Some(size(5)));
        assert_eq!(size(u32::MAX).checked_add(size(1)), None);
        assert_eq!(size(5).checked_sub(size(3)), Some(size(2)));
        assert_eq!(size(3).checked_sub(size(5)), None);
    }

    #[test]
    fn sizes_convert_to_and_from_integers() {
        assert_eq!(TextSize::from(7), size(7));
        assert_eq!(u32::from(size(7)), 7);
        assert_eq!(TextSize::try_from(7usize), Ok(size(7)));
        assert_eq!(size(7).to_usize(), 7);
        assert_eq!(size(7).to_string(), "7");
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn a_usize_beyond_four_gib_is_refused() {
        let too_big = usize::try_from(u64::from(u32::MAX) + 1).expect("64-bit host");
        assert!(TextSize::try_from(too_big).is_err());
    }

    #[test]
    fn a_range_cannot_end_before_it_starts() {
        assert_eq!(TextRange::new(size(4), size(3)), None);
        assert!(TextRange::new(size(3), size(3)).is_some());
    }

    #[test]
    fn a_range_built_from_a_length_refuses_to_overflow() {
        assert_eq!(TextRange::at(size(2), size(3)), Some(range(2, 5)));
        assert_eq!(TextRange::at(size(u32::MAX), size(1)), None);
    }

    #[test]
    fn an_empty_range_has_a_position_but_no_bytes() {
        let missing = TextRange::empty(size(6));
        assert!(missing.is_empty());
        assert_eq!(missing.len(), TextSize::ZERO);
        assert_eq!(missing.start(), size(6));
        assert_eq!(missing.end(), size(6));
        assert!(!missing.contains(size(6)));
    }

    #[test]
    fn contains_excludes_the_end() {
        let label = range(0, 5);
        assert!(label.contains(size(0)));
        assert!(label.contains(size(4)));
        assert!(!label.contains(size(5)));
        assert!(!range(3, 5).contains(size(2)));
    }

    #[test]
    fn contains_range_accepts_an_empty_range_at_the_end() {
        let line = range(2, 10);
        assert!(line.contains_range(range(2, 10)));
        assert!(line.contains_range(range(4, 6)));
        assert!(line.contains_range(TextRange::empty(size(10))));
        assert!(!line.contains_range(range(1, 4)));
        assert!(!line.contains_range(range(8, 11)));
    }

    #[test]
    fn cover_spans_both_ranges_in_either_order() {
        assert_eq!(range(2, 4).cover(range(7, 9)), range(2, 9));
        assert_eq!(range(7, 9).cover(range(2, 4)), range(2, 9));
        assert_eq!(range(2, 9).cover(range(4, 5)), range(2, 9));
    }

    #[test]
    fn slicing_never_panics() {
        let text = "£1 nop";
        assert_eq!(range(0, 3).slice(text), Some("£1"));
        assert_eq!(range(4, 7).slice(text), Some("nop"));
        // Ends inside the two-byte "£".
        assert_eq!(range(0, 1).slice(text), None);
        // Runs past the end of the text.
        assert_eq!(range(4, 20).slice(text), None);
    }

    #[test]
    fn a_range_displays_as_start_and_end() {
        assert_eq!(range(3, 8).to_string(), "3..8");
    }
}
