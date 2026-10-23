//! Any text builds a valid tree through the toy lexer, and the tree keeps
//! every rule, including joining back into the text exactly.
#![no_main]

#[path = "../../crates/syntax/tests/support/invariants.rs"]
mod invariants;
#[path = "../../crates/syntax/tests/support/toy_lexer.rs"]
mod toy_lexer;

libfuzzer_sys::fuzz_target!(|text: &str| {
    match toy_lexer::parse(text) {
        Ok(tree) => invariants::check_tree(&tree, text),
        Err(error) => panic!("the toy lexer always builds a tree: {error}"),
    }
});
