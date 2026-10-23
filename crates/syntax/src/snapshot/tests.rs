use std::fs;
use std::path::PathBuf;

use super::*;

/// A directory for one test, unique to this test process, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("lang198x-syntax-{}-{name}", std::process::id()));
        // Left over only if an earlier run with the same process id crashed.
        let _ = fs::remove_dir_all(&dir);
        if let Err(error) = fs::create_dir_all(&dir) {
            panic!("cannot make {}: {error}", dir.display());
        }
        Self(dir)
    }

    fn path(&self, file: &str) -> PathBuf {
        self.0.join(file)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write(path: &Path, text: &str) {
    if let Err(error) = fs::write(path, text) {
        panic!("cannot write {}: {error}", path.display());
    }
}

#[test]
fn a_matching_dump_passes() {
    let scratch = Scratch::new("match");
    let path = scratch.path("a.tree");
    write(&path, "ROOT@0..1\n  X@0..1 \"x\"\n");
    assert_eq!(
        compare_or_bless("ROOT@0..1\n  X@0..1 \"x\"\n", &path, false),
        Ok(())
    );
}

#[test]
fn a_differing_dump_fails_at_the_first_different_line() {
    // The checker must be able to fail, or every snapshot test passes while
    // checking nothing.
    let scratch = Scratch::new("mismatch");
    let path = scratch.path("a.tree");
    write(&path, "ROOT@0..2\n  X@0..1 \"x\"\n  Y@1..2 \"y\"\n");
    let result = compare_or_bless("ROOT@0..2\n  X@0..1 \"x\"\n  Z@1..2 \"y\"\n", &path, false);
    assert_eq!(
        result,
        Err(SnapshotError::Mismatch {
            path: path.clone(),
            line: 3,
            expected: Some("  Y@1..2 \"y\"".to_owned()),
            actual: Some("  Z@1..2 \"y\"".to_owned()),
        })
    );
    let message = result.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(message.contains("differs at line 3"), "{message}");
    assert!(message.contains("LANG198X_BLESS=1"), "{message}");
}

#[test]
fn a_shorter_or_longer_dump_fails_at_the_end() {
    let scratch = Scratch::new("length");
    let path = scratch.path("a.tree");
    write(&path, "A\nB\n");
    let shorter = compare_or_bless("A\n", &path, false);
    assert!(matches!(
        shorter,
        Err(SnapshotError::Mismatch {
            line: 2,
            expected: Some(_),
            actual: Some(_),
            ..
        })
    ));
    let longer = compare_or_bless("A\nB\nC", &path, false);
    let Err(SnapshotError::Mismatch {
        line,
        expected,
        actual,
        ..
    }) = &longer
    else {
        panic!("expected a mismatch, got {longer:?}");
    };
    assert_eq!(
        (*line, expected.as_deref(), actual.as_deref()),
        (3, Some(""), Some("C"))
    );
    write(&path, "A");
    let past_end = compare_or_bless("A\nB", &path, false);
    let message = past_end.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(message.contains("expected: (end of text)"), "{message}");
}

#[test]
fn crlf_in_the_expected_file_still_matches() {
    let scratch = Scratch::new("crlf");
    let path = scratch.path("a.tree");
    write(&path, "ROOT@0..1\r\n  X@0..1 \"x\"\r\n");
    assert_eq!(
        compare_or_bless("ROOT@0..1\n  X@0..1 \"x\"\n", &path, false),
        Ok(())
    );
}

#[test]
fn a_missing_file_says_how_to_create_it() {
    let scratch = Scratch::new("missing");
    let path = scratch.path("none.tree");
    let result = compare_or_bless("A\n", &path, false);
    assert_eq!(result, Err(SnapshotError::Missing { path }));
    let message = result.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(
        message.contains("LANG198X_BLESS=1 to create it"),
        "{message}"
    );
}

#[test]
fn blessing_writes_the_file_and_its_folders() {
    let scratch = Scratch::new("bless");
    let path = scratch.path("deep/er/a.tree");
    assert_eq!(compare_or_bless("A\n", &path, true), Ok(()));
    assert_eq!(fs::read_to_string(&path).ok().as_deref(), Some("A\n"));
    assert_eq!(compare_or_bless("A\n", &path, false), Ok(()));
}

#[test]
fn unreadable_and_unwritable_paths_are_errors() {
    let scratch = Scratch::new("io");
    // A directory cannot be read as a file.
    let dir = scratch.path("dir");
    assert!(fs::create_dir(&dir).is_ok());
    assert!(matches!(
        compare_or_bless("A\n", &dir, false),
        Err(SnapshotError::Io { .. })
    ));
    // A file cannot hold a folder.
    let file = scratch.path("file");
    write(&file, "");
    let blocked = compare_or_bless("A\n", &file.join("a.tree"), true);
    let Err(error @ SnapshotError::Io { .. }) = &blocked else {
        panic!("expected an I/O error, got {blocked:?}");
    };
    assert!(error.to_string().starts_with(&file.display().to_string()));
}

#[test]
fn check_snapshot_reads_the_bless_variable() {
    // Tests cannot set the variable without `unsafe`, so this only runs the
    // checking path, unless someone is blessing on purpose.
    let scratch = Scratch::new("public");
    let path = scratch.path("a.tree");
    write(&path, "A\n");
    if std::env::var(BLESS_VARIABLE).as_deref() != Ok("1") {
        assert_eq!(check_snapshot("A\n", &path), Ok(()));
        assert!(check_snapshot("B\n", &path).is_err());
    }
}
