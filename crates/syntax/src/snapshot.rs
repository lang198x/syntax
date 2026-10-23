//! Comparing a tree dump with an expected file.
//!
//! A snapshot test builds a tree, dumps it with [`SyntaxNode::dump`], and
//! checks the dump against a `.tree` file kept beside the test. When a change
//! to a parser is meant to change the trees, run the tests with
//! `LANG198X_BLESS=1` and the expected files are rewritten instead. Review the
//! changed files before committing them.
//!
//! [`SyntaxNode::dump`]: crate::SyntaxNode::dump

use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// The environment variable that rewrites expected files instead of checking
/// them, when set to `1`.
pub const BLESS_VARIABLE: &str = "LANG198X_BLESS";

/// Why a snapshot check failed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SnapshotError {
    /// The expected file does not exist yet.
    Missing {
        /// The expected file.
        path: PathBuf,
    },
    /// The dump differs from the expected file.
    Mismatch {
        /// The expected file.
        path: PathBuf,
        /// The first line that differs, counting from 1.
        line: usize,
        /// That line in the expected file, or `None` past its end.
        expected: Option<String>,
        /// That line in the dump, or `None` past its end.
        actual: Option<String>,
    },
    /// The expected file could not be read or written.
    Io {
        /// The file.
        path: PathBuf,
        /// What went wrong.
        message: String,
    },
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { path } => write!(
                f,
                "{} does not exist; run with {BLESS_VARIABLE}=1 to create it",
                path.display()
            ),
            Self::Mismatch {
                path,
                line,
                expected,
                actual,
            } => {
                let show = |line: &Option<String>| match line {
                    Some(text) => format!("{text:?}"),
                    None => "(end of text)".to_owned(),
                };
                write!(
                    f,
                    "{} differs at line {line}\n  expected: {}\n  actual:   {}\n\
                     run with {BLESS_VARIABLE}=1 to accept the new dump",
                    path.display(),
                    show(expected),
                    show(actual),
                )
            }
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
        }
    }
}

impl std::error::Error for SnapshotError {}

/// Checks `actual` against the expected file at `path`, or rewrites the file
/// when `LANG198X_BLESS=1` is set.
///
/// Line endings are compared as `\n`, so a checkout that turns them into
/// `\r\n` does not make every snapshot fail.
///
/// # Errors
///
/// Returns an error if the file is missing, differs from `actual`, or cannot
/// be read or written.
pub fn check_snapshot(actual: &str, path: impl AsRef<Path>) -> Result<(), SnapshotError> {
    let bless = std::env::var(BLESS_VARIABLE).is_ok_and(|value| value == "1");
    compare_or_bless(actual, path.as_ref(), bless)
}

pub(crate) fn compare_or_bless(
    actual: &str,
    path: &Path,
    bless: bool,
) -> Result<(), SnapshotError> {
    let io = |error: std::io::Error| SnapshotError::Io {
        path: path.to_owned(),
        message: error.to_string(),
    };
    if bless {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io)?;
        }
        return fs::write(path, actual).map_err(io);
    }
    let expected = match fs::read_to_string(path) {
        Ok(text) => text.replace("\r\n", "\n"),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(SnapshotError::Missing {
                path: path.to_owned(),
            });
        }
        Err(error) => return Err(io(error)),
    };
    let actual = actual.replace("\r\n", "\n");
    if expected == actual {
        return Ok(());
    }
    let mut expected_lines = expected.split('\n');
    let mut actual_lines = actual.split('\n');
    let mut line = 1;
    loop {
        let (e, a) = (expected_lines.next(), actual_lines.next());
        if e != a {
            return Err(SnapshotError::Mismatch {
                path: path.to_owned(),
                line,
                expected: e.map(str::to_owned),
                actual: a.map(str::to_owned),
            });
        }
        line += 1;
    }
}

#[cfg(test)]
mod tests;
