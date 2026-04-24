//! Lint: no bare `const` / `static` `&str` outside `#[cfg(debug_assertions)]` gates.
//!
//! Static / const `&str` literals occupy space in the release binary and
//! leak identity-bearing strings into runtime contexts where the arvo /
//! hilavitkutin stack does not want them. The rule:
//!
//! - Pre-`hilavitkutin-str` crates (arvo, notko, mockspace-*) must gate
//!   every `const NAME: &str` / `static NAME: &str` behind a
//!   `#[cfg(debug_assertions)]` attribute (directly on the item or on an
//!   enclosing item / module / file). Debug-only diagnostics and Debug
//!   impl bodies live there; release builds drop them.
//! - Post-`hilavitkutin-str` crates (hilavitkutin engine + consumers,
//!   clause) use `hilavitkutin_str::Str` interning via `str_const!()` at
//!   the call site instead of a bare literal.
//!
//! `hilavitkutin-str` itself introduces the `static-string` substrate
//! category and is exempt from the lint — it is the source-of-truth for
//! compile-time string handles.
//!
//! Implementation: tree-sitter walk of `const_item` and `static_item`
//! nodes. For each with a `&str` or `&'static str` type, the item and
//! every enclosing scope is scanned for a `#[cfg(debug_assertions)]`
//! attribute (accepting `cfg(any(debug_assertions, ...))` and
//! `cfg(all(debug_assertions, ...))` variants). Fires when no gate is
//! present.
//!
//! Escape hatch: inline `// lint:allow(no-bare-static-str) reason: ...;
//! tracked: #N` — only when a foreign contract or macro-expansion path
//! genuinely requires a bare literal at runtime.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};
use tree_sitter::{Node, Parser, Tree};

use crate::util::{categories, crate_introduces_category, err, txt};

pub struct NoBareStaticStr;

impl Lint for NoBareStaticStr {
    fn name(&self) -> &'static str { "no-bare-static-str" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        if ctx.should_skip_proc_macro_source_lint() { return Vec::new(); }
        // hilavitkutin-str introduces this category — it is the interning
        // home and its own internals are the legitimate site for static
        // string tables.
        if crate_introduces_category(ctx, categories::STATIC_STRING) {
            return Vec::new();
        }

        let mut out = Vec::new();

        if ctx.all_sources.is_empty() {
            scan_source(ctx, "src/lib.rs", ctx.source, &mut out);
            return out;
        }

        let mut parser = Parser::new();
        if parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .is_err()
        {
            return out;
        }

        for file in ctx.all_sources {
            let tree: Tree = match parser.parse(&file.text, None) {
                Some(t) => t,
                None => continue,
            };
            let rel_path = file.rel_path.display().to_string();
            walk(tree.root_node(), &file.text, &rel_path, &mut out, ctx);
        }

        out
    }
}

fn scan_source(ctx: &LintContext, rel_path: &str, source: &str, out: &mut Vec<LintError>) {
    let mut parser = Parser::new();
    if parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .is_err()
    {
        return;
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return,
    };
    walk(tree.root_node(), source, rel_path, out, ctx);
}

fn walk(
    node: Node,
    source: &str,
    rel_path: &str,
    out: &mut Vec<LintError>,
    ctx: &LintContext,
) {
    match node.kind() {
        "const_item" | "static_item" => {
            if let Some(ty) = node.child_by_field_name("type") {
                if type_is_str_ref(ty, source) {
                    // Check if this item or any enclosing scope has a
                    // `cfg(debug_assertions)` gate.
                    if !is_debug_gated(node, source) {
                        let line = node.start_position().row + 1;
                        let raw_line = source.lines().nth(node.start_position().row).unwrap_or("");
                        if !raw_line.contains("lint:allow(no-bare-static-str)") {
                            let name = node
                                .child_by_field_name("name")
                                .map(|n| txt(n, source).to_string())
                                .unwrap_or_else(|| "<anon>".to_string());
                            let keyword = if node.kind() == "const_item" { "const" } else { "static" };
                            out.push(err(
                                ctx,
                                line,
                                "no-bare-static-str",
                                format!(
                                    "bare `{keyword} {name}: &str` in {rel_path} line {line} — gate behind `#[cfg(debug_assertions)]` (pre-hilavitkutin-str crates) or use `hilavitkutin_str::Str::const!()` interning (post-hilavitkutin-str crates). Static &str does not exist in this stack outside debug builds"
                                ),
                            ));
                        }
                    }
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(child, source, rel_path, out, ctx);
    }
}

/// Whether `ty` is `&str`, `&'static str`, or a reference to `str`.
fn type_is_str_ref(ty: Node, source: &str) -> bool {
    if ty.kind() != "reference_type" {
        return false;
    }
    // `reference_type` children include optional `'lifetime`, optional
    // `mut`, and the inner `type`. Look for a `primitive_type` or `type_identifier`
    // child named `str`.
    let mut cursor = ty.walk();
    for child in ty.children(&mut cursor) {
        let text = txt(child, source).trim();
        if text == "str" && (child.kind() == "primitive_type" || child.kind() == "type_identifier") {
            return true;
        }
    }
    false
}

/// Whether `node` or any ancestor carries a `cfg(debug_assertions)`
/// attribute (direct or inside `any` / `all`).
fn is_debug_gated(node: Node, source: &str) -> bool {
    // Check the item itself first.
    if item_has_debug_gate(node, source) {
        return true;
    }
    // Walk ancestors; any item-like ancestor with the gate covers this item.
    let mut current = node.parent();
    while let Some(n) = current {
        if item_has_debug_gate(n, source) {
            return true;
        }
        current = n.parent();
    }
    false
}

/// Whether the given node has an `attribute_item` child (or precedes one
/// via sibling relation at module scope) naming `cfg(debug_assertions)`.
fn item_has_debug_gate(node: Node, source: &str) -> bool {
    // Inner attributes (source_file / mod_item use `#![cfg(...)]`).
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if (child.kind() == "attribute_item" || child.kind() == "inner_attribute_item")
            && attribute_is_debug_gate(child, source)
        {
            return true;
        }
    }
    // Outer attributes are siblings preceding the item in the parent's
    // child list. Scan preceding siblings.
    if let Some(parent) = node.parent() {
        let mut cursor = parent.walk();
        let mut prev: Option<Node> = None;
        for sibling in parent.children(&mut cursor) {
            if sibling.id() == node.id() {
                // Walk back over contiguous attribute_item siblings.
                let mut p = prev;
                while let Some(s) = p {
                    if s.kind() != "attribute_item" {
                        break;
                    }
                    if attribute_is_debug_gate(s, source) {
                        return true;
                    }
                    // Climb to the previous sibling by walking children again.
                    let mut c2 = parent.walk();
                    let mut found_before: Option<Node> = None;
                    for s2 in parent.children(&mut c2) {
                        if s2.id() == s.id() { break; }
                        found_before = Some(s2);
                    }
                    p = found_before;
                }
                break;
            }
            prev = Some(sibling);
        }
    }
    false
}

/// Whether the attribute's body references `debug_assertions` in a cfg
/// position (`cfg(debug_assertions)` / `cfg(any(debug_assertions, ...))` /
/// `cfg(all(debug_assertions, ...))` / `cfg(not(not(debug_assertions)))`).
fn attribute_is_debug_gate(attr: Node, source: &str) -> bool {
    let text = txt(attr, source);
    if !text.contains("cfg") {
        return false;
    }
    if !text.contains("debug_assertions") {
        return false;
    }
    // Reject `#[cfg(not(debug_assertions))]` — opposite meaning.
    if text.contains("not(debug_assertions)") {
        return false;
    }
    true
}
