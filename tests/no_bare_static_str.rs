//! Coverage test: the `no-bare-static-str` lint.
//!
//! Pre-hilavitkutin-str crates (arvo, notko, mockspace-*) must gate
//! every `const NAME: &str` / `static NAME: &str` behind a
//! `#[cfg(debug_assertions)]` attribute. Post-hilavitkutin-str crates
//! use `hilavitkutin_str::Str` interning. The `static-string`
//! substrate introducer (hilavitkutin-str itself) is exempt.

use std::collections::{BTreeMap, BTreeSet};

use mockspace_hilavitkutin_stack_lints::lints::no_bare_static_str::NoBareStaticStr;
use mockspace_lint_rules::{CrateSourceFile, Lint, LintContext};

fn ctx(crate_name: &'static str, introductions: Vec<(&'static str, Vec<&'static str>)>, source: &'static str) -> LintContext<'static> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .unwrap();
    let tree = parser.parse(source, None).unwrap();
    let tree: &'static tree_sitter::Tree = Box::leak(Box::new(tree));

    let all_sources: &'static [CrateSourceFile] = Box::leak(Box::new(vec![
        CrateSourceFile {
            rel_path: std::path::PathBuf::from("src/lib.rs"),
            text: source.to_string(),
        },
    ]));

    let introductions_map: BTreeMap<String, Vec<String>> = introductions
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.into_iter().map(|s| s.to_string()).collect()))
        .collect();
    let introductions_leaked: &'static BTreeMap<String, Vec<String>> =
        Box::leak(Box::new(introductions_map));

    LintContext {
        crate_name,
        short_name: crate_name,
        source,
        tree,
        all_sources,
        deps: &[],
        all_crates: Box::leak(Box::new(BTreeSet::new())),
        design_doc: None,
        all_doc_content: "",
        shame_doc: None,
        workspace_root: std::path::Path::new("/tmp"),
        proc_macro_crates: &[],
        crate_prefix: "test",
        primitive_introductions: introductions_leaked,
    }
}

fn plain_ctx(source: &'static str) -> LintContext<'static> {
    ctx("test-crate", Vec::new(), source)
}

// ---- (a) bare const / static fires ----------------------------------------

#[test]
fn fires_on_bare_const_str() {
    let src = "pub const NAME: &str = \"hi\";\n";
    let hits = NoBareStaticStr.check(&plain_ctx(src));
    assert!(!hits.is_empty(), "bare const &str must fire");
}

#[test]
fn fires_on_bare_static_str() {
    let src = "pub static NAME: &str = \"hi\";\n";
    let hits = NoBareStaticStr.check(&plain_ctx(src));
    assert!(!hits.is_empty(), "bare static &str must fire");
}

#[test]
fn fires_on_static_lifetime_str() {
    let src = "pub const NAME: &'static str = \"hi\";\n";
    let hits = NoBareStaticStr.check(&plain_ctx(src));
    assert!(!hits.is_empty(), "const with &'static str must fire");
}

#[test]
fn fires_on_private_const_str() {
    let src = "const NAME: &str = \"hi\";\n";
    let hits = NoBareStaticStr.check(&plain_ctx(src));
    assert!(!hits.is_empty(), "private const &str must still fire");
}

// ---- (b) debug-gated item allowed -----------------------------------------

#[test]
fn accepts_cfg_debug_assertions_on_item() {
    let src = "#[cfg(debug_assertions)]\npub const NAME: &str = \"hi\";\n";
    let hits = NoBareStaticStr.check(&plain_ctx(src));
    assert!(hits.is_empty(), "#[cfg(debug_assertions)] on item must silence");
}

// ---- (c) debug-gated enclosing module allowed -----------------------------

#[test]
fn accepts_cfg_debug_assertions_on_enclosing_module() {
    let src = "#[cfg(debug_assertions)]\nmod debug_only {\n    pub const NAME: &str = \"hi\";\n}\n";
    let hits = NoBareStaticStr.check(&plain_ctx(src));
    assert!(
        hits.is_empty(),
        "enclosing #[cfg(debug_assertions)] module must cover inner const"
    );
}

// ---- (d) lint:allow inline allowed ---------------------------------------

#[test]
fn accepts_lint_allow_inline() {
    let src = "pub const NAME: &str = \"hi\"; // lint:allow(no-bare-static-str) reason: foreign ABI; tracked: #125\n";
    let hits = NoBareStaticStr.check(&plain_ctx(src));
    assert!(hits.is_empty(), "lint:allow must silence");
}

// ---- (e) static-string-category crate exempt ------------------------------

#[test]
fn static_string_introducer_crate_exempt() {
    let src = "pub const NAME: &str = \"hi\";\n";
    let ctx = ctx(
        "hilavitkutin-str",
        vec![("hilavitkutin-str", vec!["static-string"])],
        src,
    );
    let hits = NoBareStaticStr.check(&ctx);
    assert!(
        hits.is_empty(),
        "hilavitkutin-str introduces static-string; lint must skip entirely"
    );
}

#[test]
fn non_static_string_crate_still_fires() {
    let src = "pub const NAME: &str = \"hi\";\n";
    let ctx = ctx(
        "arvo",
        vec![("hilavitkutin-str", vec!["static-string"])],
        src,
    );
    let hits = NoBareStaticStr.check(&ctx);
    assert!(
        !hits.is_empty(),
        "arvo doesn't introduce static-string; lint must fire"
    );
}

// ---- (f) inner attribute at file root allowed -----------------------------

#[test]
fn accepts_inner_attribute_at_file_root() {
    let src = "#![cfg(debug_assertions)]\n\npub const NAME: &str = \"hi\";\n";
    let hits = NoBareStaticStr.check(&plain_ctx(src));
    assert!(
        hits.is_empty(),
        "file-level #![cfg(debug_assertions)] must cover inner const"
    );
}

// ---- (g) cfg(not(debug_assertions)) does NOT exempt -----------------------

#[test]
fn rejects_cfg_not_debug_assertions() {
    let src = "#[cfg(not(debug_assertions))]\npub const NAME: &str = \"hi\";\n";
    let hits = NoBareStaticStr.check(&plain_ctx(src));
    assert!(
        !hits.is_empty(),
        "cfg(not(debug_assertions)) is release-only; lint must fire"
    );
}

// ---- (h) cfg(any(debug_assertions, ...)) allowed --------------------------

#[test]
fn accepts_cfg_any_with_debug_assertions() {
    let src = "#[cfg(any(debug_assertions, test))]\npub const NAME: &str = \"hi\";\n";
    let hits = NoBareStaticStr.check(&plain_ctx(src));
    assert!(
        hits.is_empty(),
        "cfg(any(debug_assertions, test)) includes debug; must silence"
    );
}

#[test]
fn accepts_cfg_all_with_debug_assertions() {
    let src = "#[cfg(all(debug_assertions, feature = \"trace\"))]\npub const NAME: &str = \"hi\";\n";
    let hits = NoBareStaticStr.check(&plain_ctx(src));
    assert!(
        hits.is_empty(),
        "cfg(all(debug_assertions, ...)) requires debug; must silence"
    );
}

// ---- type coverage --------------------------------------------------------

#[test]
fn ignores_non_str_ref_const() {
    let src = "pub const NUMBER: u32 = 5;\n";
    let hits = NoBareStaticStr.check(&plain_ctx(src));
    assert!(hits.is_empty(), "non-&str const must not trigger this lint");
}

#[test]
fn ignores_non_reference_const() {
    let src = "pub const NAME: [u8; 2] = [0, 0];\n";
    let hits = NoBareStaticStr.check(&plain_ctx(src));
    assert!(hits.is_empty(), "array const must not trigger this lint");
}
