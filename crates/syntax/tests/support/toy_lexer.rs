//! A small lexer and parser that builds a tree from any text, for tests and
//! fuzzing. It is not a real language: it exists to drive the builder through
//! lines, nesting and checkpoints with text nobody chose.
//!
//! - Each line, with its line ending, is a `LINE` node.
//! - `;` starts a comment that runs to the end of the line.
//! - Runs of letters, digits and `_` are `WORD` tokens, runs of spaces and
//!   tabs are whitespace, and any other character is a `PUNCT` token.
//! - `(` opens a `GROUP` node and `)` closes it, if one is open on the line.
//! - A word followed by `+` and another word becomes a `SUM` node, opened at a
//!   checkpoint taken before the first word. Sums chain to the left, so
//!   `a+b+c` wraps the first `SUM` in a second one, from the same checkpoint.

use lang198x_syntax::{BuildError, SyntaxKind, SyntaxTree, TextSize, TreeBuilder};

const fn kind(offset: u16) -> SyntaxKind {
    match SyntaxKind::language(SyntaxKind::FIRST_LANGUAGE + offset) {
        Some(kind) => kind,
        None => panic!("language kinds start at SyntaxKind::FIRST_LANGUAGE"),
    }
}

pub const ROOT: SyntaxKind = kind(0);
pub const LINE: SyntaxKind = kind(1);
pub const GROUP: SyntaxKind = kind(2);
pub const SUM: SyntaxKind = kind(3);
pub const WORD: SyntaxKind = kind(4);
pub const PUNCT: SyntaxKind = kind(5);

/// Builds a tree from `text`. It always succeeds for text under 4 GiB.
pub fn parse(text: &str) -> Result<SyntaxTree, BuildError> {
    let mut b = TreeBuilder::new(text);
    b.start_node(ROOT);
    for line in text.split_inclusive('\n') {
        b.start_node(LINE);
        parse_line(&mut b, line);
        b.finish_node();
    }
    b.finish_node();
    b.finish()
}

fn size(text: &str) -> TextSize {
    TextSize::of(text).unwrap_or(TextSize::ZERO)
}

fn parse_line(b: &mut TreeBuilder<'_>, line: &str) {
    let mut groups = 0;
    // Set after a word or a finished sum: where a `SUM` would start.
    let mut before_word = None;
    // The checkpoint the open `SUM` started at.
    let mut sum_start = None;
    let mut in_sum = false;
    let mut rest = line;
    while let Some(ch) = rest.chars().next() {
        let taken = if ch == ';' {
            let end = rest.find(['\r', '\n']).unwrap_or(rest.len());
            b.token(SyntaxKind::COMMENT, size(&rest[..end]));
            end
        } else if ch == '\r' || ch == '\n' {
            if in_sum {
                b.finish_node();
                in_sum = false;
            }
            let end = if rest.starts_with("\r\n") { 2 } else { 1 };
            b.token(SyntaxKind::NEWLINE, size(&rest[..end]));
            end
        } else if ch == ' ' || ch == '\t' {
            let end = rest.find(|c| c != ' ' && c != '\t').unwrap_or(rest.len());
            b.token(SyntaxKind::WHITESPACE, size(&rest[..end]));
            end
        } else if ch.is_alphanumeric() || ch == '_' {
            let checkpoint = b.checkpoint();
            let end = rest
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(rest.len());
            b.token(WORD, size(&rest[..end]));
            if in_sum {
                // The sum is complete; a further `+` wraps the whole sum.
                b.finish_node();
                in_sum = false;
                before_word = sum_start.take();
            } else {
                before_word = Some(checkpoint);
            }
            end
        } else {
            let end = ch.len_utf8();
            // A sum ends at the word after its `+`; any other character ends
            // it first, so brackets never close the wrong node.
            if in_sum {
                b.finish_node();
                in_sum = false;
            }
            match ch {
                '(' => {
                    b.start_node(GROUP);
                    groups += 1;
                    b.token(PUNCT, size(&rest[..end]));
                }
                ')' if groups > 0 => {
                    b.token(PUNCT, size(&rest[..end]));
                    b.finish_node();
                    groups -= 1;
                }
                '+' if !in_sum && before_word.is_some() => {
                    if let Some(checkpoint) = before_word.take() {
                        b.start_node_at(checkpoint, SUM);
                        sum_start = Some(checkpoint);
                        in_sum = true;
                    }
                    b.token(PUNCT, size(&rest[..end]));
                }
                _ => b.token(PUNCT, size(&rest[..end])),
            }
            if ch != '+' {
                before_word = None;
            }
            end
        };
        rest = &rest[taken..];
        if ch.is_whitespace() || ch == ';' {
            // A sum only joins words that sit right next to the `+`.
            if in_sum {
                b.finish_node();
                in_sum = false;
            }
            before_word = None;
        }
    }
    if in_sum {
        b.finish_node();
    }
    for _ in 0..groups {
        b.finish_node();
    }
}
