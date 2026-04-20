//! Lint: the headline. Public signatures should carry trait bounds at
//! positions where two plausible implementations exist. This first-pass
//! heuristic flags suspicious concrete-type uses that almost always should
//! have been trait bounds:
//!
//! - Sink-shaped returns: `fn collect() -> Vec<T>` → `fn collect<C: Collector<T>>(sink: &mut C)`
//! - Concrete collection params: `fn foo(xs: Vec<T>)` → `fn foo(xs: impl IntoIterator<Item = T>)`
//! - Concrete hashmap params: `fn foo(m: HashMap<K, V>)` → trait bound
//!
//! Richer detection will grow with signal. First pass catches the big three.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};
use tree_sitter::Node;

use crate::util::{err, for_each_fn, is_public, txt};

pub struct TraitFirstSignatures;

impl Lint for TraitFirstSignatures {
    fn name(&self) -> &'static str { "trait-first-signatures" }

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
    if src_line.contains("lint:allow(trait-first-signatures)") { return; }

    let name = node.child_by_field_name("name")
        .map(|n| txt(n, ctx.source))
        .unwrap_or("<unknown>");

    // Return-type containers.
    if let Some(ret) = node.child_by_field_name("return_type") {
        let text = txt(ret, ctx.source);
        for ty in &["Vec<", "HashMap<", "BTreeMap<", "HashSet<", "VecDeque<"] {
            if text.contains(ty) {
                out.push(err(
                    ctx,
                    line,
                    "trait-first-signatures",
                    format!("`{name}` returns a concrete `{ty}...>` — take a sink or return `impl Iterator`"),
                ));
                return;
            }
        }
    }

    // Parameter containers by value.
    if let Some(params) = node.child_by_field_name("parameters") {
        let text = txt(params, ctx.source);
        // Heuristic: `: Vec<` as a parameter type → suggest impl IntoIterator.
        for ty in &[": Vec<", ": HashMap<", ": BTreeMap<", ": HashSet<", ": VecDeque<"] {
            if text.contains(ty) {
                out.push(err(
                    ctx,
                    line,
                    "trait-first-signatures",
                    format!("`{name}` parameter uses concrete `{ty}...>`; take `impl IntoIterator<Item = _>` or a trait bound"),
                ));
                return;
            }
        }
    }
}
