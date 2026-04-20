//! Lint: no bare `String` or non-static `&str` in public API positions.
//! Use `hilavitkutin_str::Str` or `&'static str`.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};
use tree_sitter::Node;

use crate::util::{err, for_each_fn, inside_macro_def, is_public, txt};

pub struct NoBareString;

impl Lint for NoBareString {
    fn name(&self) -> &'static str { "no-bare-string" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        if ctx.is_proc_macro_crate() { return Vec::new(); }
        let mut out = Vec::new();
        for_each_fn(ctx.tree.root_node(), |node| {
            if !is_public(node, ctx.source) { return; }
            if inside_macro_def(node) { return; }
            check_fn(node, ctx, &mut out);
        });
        out
    }
}

fn check_fn(node: Node, ctx: &LintContext, out: &mut Vec<LintError>) {
    let line = node.start_position().row + 1;
    let name = node.child_by_field_name("name")
        .map(|n| txt(n, ctx.source))
        .unwrap_or("<unknown>");

    let mut sig = String::new();
    if let Some(params) = node.child_by_field_name("parameters") {
        sig.push_str(txt(params, ctx.source));
    }
    if let Some(ret) = node.child_by_field_name("return_type") {
        sig.push(' ');
        sig.push_str(txt(ret, ctx.source));
    }

    if on_line_allowed(ctx, node) { return; }

    if contains_bare_string_type(&sig) {
        out.push(err(
            ctx,
            line,
            "no-bare-string",
            format!("`{name}` signature uses bare `String` — use hilavitkutin_str::Str"),
        ));
        return;
    }
    if contains_non_static_str_ref(&sig) {
        out.push(err(
            ctx,
            line,
            "no-bare-string",
            format!("`{name}` signature uses non-static `&str` — use `&'static str` or hilavitkutin_str::Str"),
        ));
    }
}

fn contains_bare_string_type(sig: &str) -> bool {
    let bytes = sig.as_bytes();
    let needle = b"String";
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

fn contains_non_static_str_ref(sig: &str) -> bool {
    // `&str` without a `'static` lifetime qualifier.
    let mut i = 0;
    let bytes = sig.as_bytes();
    while i + 4 <= bytes.len() {
        if &bytes[i..i + 1] == b"&" {
            // Skip lifetime.
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() { j += 1; }
            let lifetime_start = j;
            if j < bytes.len() && bytes[j] == b'\'' {
                j += 1;
                while j < bytes.len() && is_ident(bytes[j]) { j += 1; }
                while j < bytes.len() && bytes[j].is_ascii_whitespace() { j += 1; }
            }
            if j + 3 <= bytes.len() && &bytes[j..j + 3] == b"str" {
                let after = j + 3;
                let after_ok = after >= bytes.len() || !is_ident(bytes[after]);
                if after_ok {
                    // Check lifetime if any.
                    let lifetime = std::str::from_utf8(&bytes[lifetime_start..j]).unwrap_or("").trim();
                    if lifetime != "'static" {
                        return true;
                    }
                }
            }
        }
        i += 1;
    }
    false
}

fn is_ident(b: u8) -> bool { b.is_ascii_alphanumeric() || b == b'_' }

fn on_line_allowed(ctx: &LintContext, node: Node) -> bool {
    let row = node.start_position().row;
    let line = ctx.source.lines().nth(row).unwrap_or("");
    line.contains("lint:allow(no-bare-string)")
}
