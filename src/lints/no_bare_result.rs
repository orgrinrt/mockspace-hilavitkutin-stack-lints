//! Lint: no bare `Result<T, E>` in public API positions. Use `notko::Outcome<T, E>`
//! (cold path) or `notko::Just<T>` (hot path). `#[optimize_for(...)]` rewrites
//! `Result` to the right fallibility tier at compile time, so attributed fns
//! are exempt.
//!
//! Allowed: `fmt::Result`, `io::Result` (std trait method impls).

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};
use tree_sitter::Node;

use crate::util::{err, for_each_fn, has_attribute, inside_macro_def, is_public, txt};

pub struct NoBareResult;

impl Lint for NoBareResult {
    fn name(&self) -> &'static str { "no-bare-result" }

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

    if let Some(ret) = node.child_by_field_name("return_type") {
        let text = ret_text(ret, ctx.source);
        if is_ok_result(&text) { return; }
        if text.starts_with("Result<") {
            if on_line_allowed(ctx, node) { return; }
            out.push(err(
                ctx,
                line,
                "no-bare-result",
                format!("`{name}` returns `{text}` — use notko::Outcome<T, E> or #[optimize_for(hot|cold)]"),
            ));
        }
    }
}

fn ret_text(node: Node, src: &str) -> String {
    txt(node, src).trim().strip_prefix("->").unwrap_or(txt(node, src).trim()).trim().to_string()
}

fn is_ok_result(text: &str) -> bool {
    matches!(text, "fmt::Result" | "std::fmt::Result")
        || text.starts_with("io::Result")
        || text.starts_with("std::io::Result")
}

fn on_line_allowed(ctx: &LintContext, node: Node) -> bool {
    let row = node.start_position().row;
    let line = ctx.source.lines().nth(row).unwrap_or("");
    line.contains("lint:allow(no-bare-result)")
}
