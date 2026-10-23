//! Runs the shared invariants over the toy lexer's trees, so the code the
//! fuzz targets use is also built and run by `cargo test`.

mod support {
    pub mod invariants;
    pub mod toy_lexer;
}

use std::path::PathBuf;

use lang198x_syntax::check_snapshot;
use support::invariants::check_tree;
use support::toy_lexer::{self, GROUP, LINE, PUNCT, ROOT, SUM, WORD};

fn parsed(text: &str) -> lang198x_syntax::SyntaxTree {
    match toy_lexer::parse(text) {
        Ok(tree) => tree,
        Err(error) => panic!("the toy lexer always builds a tree: {error}"),
    }
}

#[test]
fn the_fuzz_seeds_parse_and_keep_every_rule() {
    let seeds = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/seeds/toy_lexer");
    let mut count = 0;
    for entry in std::fs::read_dir(&seeds).unwrap_or_else(|e| panic!("{}: {e}", seeds.display())) {
        let path = entry.unwrap_or_else(|e| panic!("{e}")).path();
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let text = String::from_utf8_lossy(&bytes);
        check_tree(&parsed(&text), &text);
        count += 1;
    }
    assert!(count > 0, "no seeds found in {}", seeds.display());
}

#[test]
fn awkward_text_keeps_every_rule() {
    for text in [
        "",
        "\n",
        "\r\n",
        "(((",
        ")))",
        "a+",
        "+a",
        "a+b+c",
        "a+(b)",
        "x ;é\r\n(",
        "a+b)",
    ] {
        check_tree(&parsed(text), text);
    }
}

#[test]
fn the_toy_lexer_builds_lines_groups_and_sums() {
    let names = |kind| {
        Some(match kind {
            k if k == ROOT => "ROOT",
            k if k == LINE => "LINE",
            k if k == GROUP => "GROUP",
            k if k == SUM => "SUM",
            k if k == WORD => "WORD",
            k if k == PUNCT => "PUNCT",
            _ => return None,
        })
    };
    let text = "ld a,(b+1+c) ; go\nret\n";
    let tree = parsed(text);
    check_tree(&tree, text);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/toy-lexer.tree");
    if let Err(error) = check_snapshot(&tree.root().dump(names), path) {
        panic!("{error}");
    }
}
