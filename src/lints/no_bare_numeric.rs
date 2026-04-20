//! Lint: no bare `u8..u128`, `i8..i128`, `f32`, `f64`, `bool`, `usize`,
//! `isize` in public API positions. Use the arvo fixed-point primitive or
//! a domain alias grounded on one.
//!
//! arvo is the exclusive numeric substrate. Bare primitives betray that.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};
use tree_sitter::Node;

use crate::util::{err, for_each_fn, has_attribute, inside_macro_def, is_public, txt};

const BARE_NUMERICS: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128",
    "i8", "i16", "i32", "i64", "i128",
    "f32", "f64",
    "usize", "isize",
    "bool",
];

pub struct NoBareNumeric;

impl Lint for NoBareNumeric {
    fn name(&self) -> &'static str { "no-bare-numeric" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        if ctx.is_proc_macro_crate() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for_each_fn(ctx.tree.root_node(), |node| {
            if !is_public(node, ctx.source) { return; }
            if inside_macro_def(node) { return; }
            if has_attribute(node, ctx.source, "optimize_for") { return; }
            check_fn(node, ctx, &mut out);
        });
        out
    }
}

fn check_fn(node: Node, ctx: &LintContext, out: &mut Vec<LintError>) {
    let line = node.start_position().row + 1;
    let name = node
        .child_by_field_name("name")
        .map(|n| txt(n, ctx.source))
        .unwrap_or("<unknown>");

    // Collect parameters + return type text; scan for bare numeric tokens.
    let mut text = String::new();
    if let Some(params) = node.child_by_field_name("parameters") {
        text.push_str(txt(params, ctx.source));
    }
    if let Some(ret) = node.child_by_field_name("return_type") {
        text.push(' ');
        text.push_str(txt(ret, ctx.source));
    }

    if text.contains("lint:allow(no-bare-numeric)") {
        return;
    }
    if on_line_allowed(ctx, node) {
        return;
    }

    for prim in BARE_NUMERICS {
        if contains_token(&text, prim) {
            out.push(err(
                ctx,
                line,
                "no-bare-numeric",
                format!(
                    "`{name}` signature contains bare `{prim}`. Use an arvo fixed-point primitive (UFixed / IFixed / FastFloat / StrictFloat / USize / Cap / Bool) or a domain alias grounded on one"
                ),
            ));
            return;
        }
    }
}

fn contains_token(hay: &str, tok: &str) -> bool {
    let bytes = hay.as_bytes();
    let needle = tok.as_bytes();
    let mut i = 0;
    while i + needle.len() <= bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            let before_ok = i == 0 || !is_ident(bytes[i - 1]);
            let after_pos = i + needle.len();
            let after_ok = after_pos >= bytes.len() || !is_ident(bytes[after_pos]);
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn on_line_allowed(ctx: &LintContext, node: Node) -> bool {
    let row = node.start_position().row;
    let line = ctx.source.lines().nth(row).unwrap_or("");
    line.contains("lint:allow(no-bare-numeric)")
}
