//! The kinds of nodes and tokens.
//!
//! Every node and token in a syntax tree has a kind, such as "label",
//! "instruction" or "whitespace". Each language numbers its own kinds. A few
//! kinds mean the same thing in every language, so they are shared: whitespace,
//! newlines, comments, errors and verbatim regions. Shared kinds sit in a small
//! reserved range at the bottom, and every language numbers its own kinds above
//! it, so the two can never collide.

use std::fmt;

/// The kind of a node or token in a syntax tree.
///
/// A kind is a 16-bit number. Values below [`SyntaxKind::FIRST_LANGUAGE`] are
/// reserved for the shared kinds, such as [`SyntaxKind::WHITESPACE`]. A language
/// makes its own kinds with [`SyntaxKind::language`], which refuses any value
/// inside the reserved range.
///
/// ```
/// use lang198x_syntax::SyntaxKind;
///
/// // A language's kinds start at FIRST_LANGUAGE.
/// let label = SyntaxKind::language(SyntaxKind::FIRST_LANGUAGE)
///     .expect("the first language value is outside the reserved range");
/// assert!(!label.is_shared());
///
/// // A value inside the reserved range cannot be a language kind.
/// assert_eq!(SyntaxKind::language(0), None);
/// assert!(SyntaxKind::COMMENT.is_shared());
/// ```
///
/// A language usually defines its kinds as constants. A failed check inside a
/// constant stops the build, so a kind in the wrong range never reaches a
/// running program:
///
/// ```
/// use lang198x_syntax::SyntaxKind;
///
/// const fn kind(raw: u16) -> SyntaxKind {
///     match SyntaxKind::language(raw) {
///         Some(kind) => kind,
///         None => panic!("language kinds start at SyntaxKind::FIRST_LANGUAGE"),
///     }
/// }
///
/// const LABEL: SyntaxKind = kind(SyntaxKind::FIRST_LANGUAGE);
/// const MNEMONIC: SyntaxKind = kind(SyntaxKind::FIRST_LANGUAGE + 1);
/// assert_ne!(LABEL, MNEMONIC);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SyntaxKind(u16);

impl SyntaxKind {
    /// Spaces and tabs between other tokens.
    pub const WHITESPACE: Self = Self(0);

    /// The end of a line. It is a token of its own, separate from whitespace,
    /// because line-oriented languages give each line its own node.
    pub const NEWLINE: Self = Self(1);

    /// A comment, from its opening marker to its end.
    pub const COMMENT: Self = Self(2);

    /// Source the parser could not make sense of. Parsing never fails, so
    /// broken source becomes an `ERROR` node or token, and a diagnostic beside
    /// the tree says what went wrong.
    pub const ERROR: Self = Self(3);

    /// A region the formatter must leave exactly as written, such as a macro
    /// body that substitutes inside tokens.
    pub const VERBATIM: Self = Self(4);

    /// The lowest value a language can use for its own kinds.
    ///
    /// Everything below it is reserved for shared kinds. The range leaves room
    /// for more shared kinds later. Raising this value would renumber every
    /// language, so it is a breaking change.
    pub const FIRST_LANGUAGE: u16 = 16;

    /// Makes a language's own kind from its raw number.
    ///
    /// Returns `None` if `raw` is inside the reserved range, below
    /// [`SyntaxKind::FIRST_LANGUAGE`].
    #[must_use]
    pub const fn language(raw: u16) -> Option<Self> {
        if raw < Self::FIRST_LANGUAGE {
            None
        } else {
            Some(Self(raw))
        }
    }

    /// Returns the raw number.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    /// Returns whether this is one of the shared kinds, which mean the same
    /// in every language.
    #[must_use]
    pub const fn is_shared(self) -> bool {
        self.0 < Self::FIRST_LANGUAGE
    }

    /// Returns the name of a shared kind, such as `"WHITESPACE"`, or `None`
    /// for a language's own kind. Languages name their own kinds.
    #[must_use]
    pub const fn shared_name(self) -> Option<&'static str> {
        match self {
            Self::WHITESPACE => Some("WHITESPACE"),
            Self::NEWLINE => Some("NEWLINE"),
            Self::COMMENT => Some("COMMENT"),
            Self::ERROR => Some("ERROR"),
            Self::VERBATIM => Some("VERBATIM"),
            _ => None,
        }
    }
}

impl From<SyntaxKind> for u16 {
    fn from(kind: SyntaxKind) -> Self {
        kind.0
    }
}

impl fmt::Debug for SyntaxKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.shared_name() {
            Some(name) => f.write_str(name),
            None => write!(f, "SyntaxKind({})", self.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHARED: [SyntaxKind; 5] = [
        SyntaxKind::WHITESPACE,
        SyntaxKind::NEWLINE,
        SyntaxKind::COMMENT,
        SyntaxKind::ERROR,
        SyntaxKind::VERBATIM,
    ];

    #[test]
    fn shared_kinds_are_recognised_and_named() {
        for kind in SHARED {
            assert!(kind.is_shared(), "{kind:?} should be shared");
            assert!(kind.shared_name().is_some(), "{kind:?} should have a name");
        }
    }

    #[test]
    fn shared_kinds_are_distinct_and_inside_the_reserved_range() {
        for (i, a) in SHARED.iter().enumerate() {
            assert!(a.get() < SyntaxKind::FIRST_LANGUAGE);
            for b in &SHARED[i + 1..] {
                assert_ne!(a, b);
                assert_ne!(a.shared_name(), b.shared_name());
            }
        }
    }

    #[test]
    fn no_language_kind_can_be_made_inside_the_reserved_range() {
        for raw in 0..SyntaxKind::FIRST_LANGUAGE {
            assert_eq!(SyntaxKind::language(raw), None, "raw value {raw}");
        }
    }

    #[test]
    fn language_kinds_cover_the_rest_of_the_range() {
        for raw in [SyntaxKind::FIRST_LANGUAGE, 1000, u16::MAX] {
            let Some(kind) = SyntaxKind::language(raw) else {
                panic!("raw value {raw} should make a language kind");
            };
            assert_eq!(kind.get(), raw);
            assert_eq!(u16::from(kind), raw);
            assert!(!kind.is_shared());
            assert_eq!(kind.shared_name(), None);
        }
    }

    #[test]
    fn unused_reserved_values_are_shared_but_unnamed() {
        // Reserved values with no shared kind yet still count as shared, so a
        // language can never claim them, but they have no name.
        let spare = SyntaxKind(SyntaxKind::FIRST_LANGUAGE - 1);
        assert!(spare.is_shared());
        assert_eq!(spare.shared_name(), None);
    }

    #[test]
    fn debug_prints_shared_names_and_raw_language_numbers() {
        assert_eq!(format!("{:?}", SyntaxKind::COMMENT), "COMMENT");
        let Some(kind) = SyntaxKind::language(42) else {
            panic!("42 is outside the reserved range");
        };
        assert_eq!(format!("{kind:?}"), "SyntaxKind(42)");
    }
}
