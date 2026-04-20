//! Lint: every public `UFixed<I, F>` / `IFixed<I, F>` numeric type
//! position must carry an explicit `S: Strategy` marker — `Hot`, `Warm`,
//! `Cold`, `Precise`, or a bound. A bare `UFixed<I32, F0>` without `S`
//! signals a sloppy signature where the caller can't choose container
//! speed.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};
use tree_sitter::Node;

use crate::util::{err, for_each_fn, is_public, txt};

pub struct StrategyMarkerRequired;

impl Lint for StrategyMarkerRequired {
    fn name(&self) -> &'static str { "strategy-marker-required" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        if ctx.is_proc_macro_crate() { return Vec::new(); }
        let mut out = Vec::new();
        for_each_fn(ctx.tree.root_node(), |node| {
            if !is_public(node, ctx.source) { return; }
            check_fn(node, ctx, &mut out);
        });
        out
    }
}

fn check_fn(node: Node, ctx: &LintContext, out: &mut Vec<LintError>) {
    let line = node.start_position().row + 1;
    let src_line = ctx.source.lines().nth(node.start_position().row).unwrap_or("");
    if src_line.contains("lint:allow(strategy-marker-required)") { return; }

    let mut sig = String::new();
    if let Some(params) = node.child_by_field_name("parameters") {
        sig.push_str(txt(params, ctx.source));
    }
    if let Some(ret) = node.child_by_field_name("return_type") {
        sig.push(' ');
        sig.push_str(txt(ret, ctx.source));
    }

    for ty in &["UFixed", "IFixed"] {
        for hit in find_generic_invocations(&sig, ty) {
            // Count type params. UFixed<I, F, S> is 3. UFixed<I, F> is 2.
            let params = count_top_level_args(hit);
            if params < 3 {
                let name = node.child_by_field_name("name")
                    .map(|n| txt(n, ctx.source))
                    .unwrap_or("<unknown>");
                out.push(err(
                    ctx,
                    line,
                    "strategy-marker-required",
                    format!("`{name}` uses `{ty}<...>` without a Strategy marker. Require `S: Strategy` or use Hot/Warm/Cold/Precise explicitly"),
                ));
                return;
            }
        }
    }
}

/// Find all substrings of the form `TY< ... >` (balanced angle brackets)
/// and return the inner substring (without the wrapping brackets).
fn find_generic_invocations<'a>(hay: &'a str, ty: &str) -> Vec<&'a str> {
    let mut hits = Vec::new();
    let pattern = format!("{ty}<");
    let mut start = 0;
    while let Some(pos) = hay[start..].find(&pattern) {
        let abs = start + pos;
        let open = abs + pattern.len() - 1;
        let mut depth = 0;
        let bytes = hay.as_bytes();
        let mut close = None;
        for i in open..bytes.len() {
            match bytes[i] {
                b'<' => depth += 1,
                b'>' => {
                    depth -= 1;
                    if depth == 0 { close = Some(i); break; }
                }
                _ => {}
            }
        }
        if let Some(c) = close {
            hits.push(&hay[open + 1..c]);
            start = c + 1;
        } else {
            break;
        }
    }
    hits
}

fn count_top_level_args(inner: &str) -> usize {
    let mut depth = 0i32;
    let mut count = 1;
    for c in inner.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => count += 1,
            _ => {}
        }
    }
    if inner.trim().is_empty() { 0 } else { count }
}
