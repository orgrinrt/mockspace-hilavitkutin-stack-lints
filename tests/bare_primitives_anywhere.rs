//! Coverage test: the tightened `no-bare-*` and `arvo-types-only`
//! lints fire on drift ANYWHERE in source — not just public fn
//! signatures.
//!
//! Every case here captures a real leak in the pre-tightening lints:
//! private fields, tuple-struct fields, let bindings, cast
//! expressions, literal suffixes, trait method signatures, const
//! declarations. All must produce at least one violation per the
//! rule "primitives don't exist in this stack".

use std::collections::BTreeSet;

use mockspace_hilavitkutin_stack_lints::lints::{
    arvo_types_only::ArvoTypesOnly, no_bare_numeric::NoBareNumeric,
    no_bare_option::NoBareOption, no_bare_result::NoBareResult,
    no_bare_string::NoBareString, no_public_raw_field::NoPublicRawField,
};
use mockspace_lint_rules::{Lint, LintContext};

fn ctx_with(source: &'static str) -> LintContext<'static> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .unwrap();
    let tree = parser.parse(source, None).unwrap();
    let tree: &'static tree_sitter::Tree = Box::leak(Box::new(tree));

    LintContext {
        crate_name: "test-crate",
        short_name: "test-crate",
        source,
        tree,
        deps: &[],
        all_crates: Box::leak(Box::new(BTreeSet::new())),
        design_doc: None,
        all_doc_content: "",
        shame_doc: None,
        workspace_root: std::path::Path::new("/tmp"),
        proc_macro_crates: &[],
        crate_prefix: "test",
    }
}

// ---- arvo-types-only ------------------------------------------------------

#[test]
fn arvo_types_only_fires_on_private_tuple_struct_field() {
    let src = "pub struct Handle(u32);\n";
    let hits = ArvoTypesOnly.check(&ctx_with(src));
    assert!(!hits.is_empty(), "private tuple-struct u32 must be flagged");
}

#[test]
fn arvo_types_only_fires_on_private_named_field() {
    let src = "pub struct Cache { count: usize }\n";
    let hits = ArvoTypesOnly.check(&ctx_with(src));
    assert!(!hits.is_empty(), "private named usize field must be flagged");
}

#[test]
fn arvo_types_only_fires_on_cast_expression() {
    let src = "fn inside() { let _ = 0_i64 as u32; }\n";
    let hits = ArvoTypesOnly.check(&ctx_with(src));
    assert!(!hits.is_empty(), "`as u32` cast must be flagged");
}

#[test]
fn arvo_types_only_fires_on_literal_suffix() {
    let src = "const X: usize = 0u32 as usize;\n";
    let hits = ArvoTypesOnly.check(&ctx_with(src));
    assert!(!hits.is_empty(), "literal suffix `0u32` must be flagged");
}

#[test]
fn arvo_types_only_fires_on_trait_method_param() {
    let src = "pub trait Foo { fn bar(&self, x: bool); }\n";
    let hits = ArvoTypesOnly.check(&ctx_with(src));
    assert!(!hits.is_empty(), "trait method bare `bool` param must be flagged");
}

#[test]
fn arvo_types_only_fires_on_array_element_type() {
    let src = "const BUF: [u8; 16] = [0; 16];\n";
    let hits = ArvoTypesOnly.check(&ctx_with(src));
    assert!(!hits.is_empty(), "array element type `u8` must be flagged");
}

#[test]
fn arvo_types_only_accepts_allow_comment() {
    let src = "pub struct Handle(u32); // lint:allow(arvo-types-only) reason: rkyv Archived; tracked: #72\n";
    let hits = ArvoTypesOnly.check(&ctx_with(src));
    assert!(hits.is_empty(), "lint:allow(arvo-types-only) must silence");
}

#[test]
fn arvo_types_only_ignores_inside_string_literal() {
    let src = "pub const MSG: &str = \"u32 is forbidden\";\n";
    let hits = ArvoTypesOnly.check(&ctx_with(src));
    // MSG: &str still violates no-bare-string, but arvo-types-only
    // must NOT false-positive on the "u32" token inside the literal.
    assert!(
        hits.iter().all(|e| !e.message.contains("u32")),
        "u32 inside string literal must not be reported",
    );
}

// ---- no-bare-numeric ------------------------------------------------------

#[test]
fn no_bare_numeric_fires_on_private_fn_param() {
    let src = "fn helper(n: usize) -> usize { n }\n";
    let hits = NoBareNumeric.check(&ctx_with(src));
    assert!(!hits.is_empty(), "private fn signature usize must be flagged");
}

#[test]
fn no_bare_numeric_fires_on_let_binding() {
    let src = "pub fn f() { let _x: u64 = 0u64; }\n";
    let hits = NoBareNumeric.check(&ctx_with(src));
    assert!(!hits.is_empty(), "local let binding u64 must be flagged");
}

// ---- no-bare-option -------------------------------------------------------

#[test]
fn no_bare_option_fires_on_private_fn_return() {
    let src = "fn lookup() -> Option<u8> { None }\n";
    let hits = NoBareOption.check(&ctx_with(src));
    assert!(!hits.is_empty(), "private fn Option return must be flagged");
}

#[test]
fn no_bare_option_fires_on_struct_field() {
    let src = "pub struct Cached { handle: Option<u32> }\n";
    let hits = NoBareOption.check(&ctx_with(src));
    assert!(!hits.is_empty(), "struct field Option must be flagged");
}

#[test]
fn no_bare_option_accepts_allow_comment() {
    let src = "fn iter() -> Option<u8> { None } // lint:allow(no-bare-option) reason: core::Iterator parity; tracked: #115\n";
    let hits = NoBareOption.check(&ctx_with(src));
    assert!(hits.is_empty(), "lint:allow must silence no-bare-option");
}

// ---- no-bare-result -------------------------------------------------------

#[test]
fn no_bare_result_fires_on_private_fn_return() {
    let src = "fn load() -> Result<u8, ()> { Ok(0) }\n";
    let hits = NoBareResult.check(&ctx_with(src));
    assert!(!hits.is_empty(), "private fn Result return must be flagged");
}

#[test]
fn no_bare_result_accepts_fmt_result() {
    let src = "pub fn fmt(f: &mut core::fmt::Formatter) -> fmt::Result { Ok(()) }\n";
    let hits = NoBareResult.check(&ctx_with(src));
    assert!(
        hits.is_empty(),
        "fmt::Result (std trait parity) must pass"
    );
}

#[test]
fn no_bare_result_accepts_io_result() {
    let src = "pub fn open() -> io::Result<()> { Ok(()) }\n";
    let hits = NoBareResult.check(&ctx_with(src));
    assert!(
        hits.is_empty(),
        "io::Result (std trait parity) must pass"
    );
}

// ---- no-bare-string -------------------------------------------------------

#[test]
fn no_bare_string_fires_on_private_field() {
    let src = "struct Msg { body: String }\n";
    let hits = NoBareString.check(&ctx_with(src));
    assert!(!hits.is_empty(), "private String field must be flagged");
}

#[test]
fn no_bare_string_fires_on_private_fn_param() {
    let src = "fn shout(s: &str) {}\n";
    let hits = NoBareString.check(&ctx_with(src));
    assert!(!hits.is_empty(), "private fn non-static &str param must be flagged");
}

#[test]
fn no_bare_string_accepts_static_str() {
    let src = "pub fn label() -> &'static str { \"ok\" }\n";
    let hits = NoBareString.check(&ctx_with(src));
    assert!(hits.is_empty(), "&'static str must pass");
}

// ---- no-public-raw-field (now universal) ----------------------------------

#[test]
fn no_public_raw_field_fires_on_private_field() {
    let src = "pub struct Foo { inner: u64 }\n";
    let hits = NoPublicRawField.check(&ctx_with(src));
    assert!(!hits.is_empty(), "private u64 field of pub struct must be flagged");
}

#[test]
fn no_public_raw_field_fires_on_private_struct() {
    let src = "struct Internal { count: usize }\n";
    let hits = NoPublicRawField.check(&ctx_with(src));
    assert!(!hits.is_empty(), "private struct with raw field must be flagged");
}

#[test]
fn no_public_raw_field_fires_on_tuple_struct() {
    let src = "pub struct Handle(u32);\n";
    let hits = NoPublicRawField.check(&ctx_with(src));
    assert!(!hits.is_empty(), "tuple-struct raw u32 field must be flagged");
}

#[test]
fn no_public_raw_field_accepts_allow_comment() {
    let src = "pub struct H { v: u32 } // lint:allow(no-public-raw-field) reason: rkyv Archived; tracked: #72\n";
    let hits = NoPublicRawField.check(&ctx_with(src));
    assert!(hits.is_empty(), "lint:allow must silence no-public-raw-field");
}
