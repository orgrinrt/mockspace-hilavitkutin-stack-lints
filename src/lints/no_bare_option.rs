//! Lint: no bare `Option<T>` in public API positions. Use `notko::Maybe<T>`.
//!
//! Allowed: std trait-method impls where the signature is fixed by the trait
//! (`fn next(&mut self) -> Option<Self::Item>`, etc.).

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};
use tree_sitter::Node;

use crate::util::{err, for_each_fn, has_attribute, inside_macro_def, is_public, txt};

pub struct NoBareOption;

impl Lint for NoBareOption {
    fn name(&self) -> &'static str { "no-bare-option" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        if ctx.is_proc_macro_crate() {
            return Vec::new();
        }

        let mut out = Vec::new();
        for_each_fn(ctx.tree.root_node(), |node| {
            if !is_public(node, ctx.source) {
                return;
            }
            if inside_macro_def(node) {
                return;
            }
            if has_attribute(node, ctx.source, "optimize_for") {
                return;
            }
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

    if let Some(ret) = node.child_by_field_name("return_type") {
        let text = strip_return_arrow(txt(ret, ctx.source));
        if starts_with_generic(text, "Option") {
            if !on_line_allowed(ctx, node, "no-bare-option") {
                out.push(err(
                    ctx,
                    line,
                    "no-bare-option",
                    format!("`{name}` returns `{text}` — use notko::Maybe<T> at public boundaries"),
                ));
            }
        }
    }

    if let Some(params) = node.child_by_field_name("parameters") {
        let text = txt(params, ctx.source);
        if text.contains("Option<") && !on_line_allowed(ctx, node, "no-bare-option") {
            out.push(err(
                ctx,
                line,
                "no-bare-option",
                format!("`{name}` parameters include `Option<T>` — use notko::Maybe<T>"),
            ));
        }
    }
}

fn strip_return_arrow(s: &str) -> &str {
    s.trim().strip_prefix("->").unwrap_or(s.trim()).trim()
}

fn starts_with_generic(text: &str, ty: &str) -> bool {
    text.starts_with(&format!("{ty}<")) || text == ty
}

fn on_line_allowed(ctx: &LintContext, node: Node, rule: &str) -> bool {
    let row = node.start_position().row;
    let line = ctx.source.lines().nth(row).unwrap_or("");
    line.contains(&format!("lint:allow({rule})"))
}
