//! Lint: no `Vec<T>` (or equivalent heap container) in trait method
//! signatures. Traits are contracts; callers provide a sink/iterator,
//! implementers don't return an owned heap container.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};
use tree_sitter::Node;

use crate::util::{err, for_each_trait, txt};

const FORBIDDEN_IN_TRAIT: &[&str] = &["Vec<", "HashMap<", "BTreeMap<", "HashSet<", "BTreeSet<", "VecDeque<", "String"];

pub struct NoVecInTraitSig;

impl Lint for NoVecInTraitSig {
    fn name(&self) -> &'static str { "no-vec-in-trait-sig" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        if ctx.is_proc_macro_crate() { return Vec::new(); }
        let mut out = Vec::new();
        for_each_trait(ctx.tree.root_node(), |node| {
            check_trait(node, ctx, &mut out);
        });
        out
    }
}

fn check_trait(node: Node, ctx: &LintContext, out: &mut Vec<LintError>) {
    let body = match node.child_by_field_name("body") {
        Some(b) => b,
        None => return,
    };

    let mut cursor = body.walk();
    for item in body.children(&mut cursor) {
        if item.kind() != "function_item" && item.kind() != "function_signature_item" {
            continue;
        }
        let line = item.start_position().row + 1;
        let src_line = ctx.source.lines().nth(item.start_position().row).unwrap_or("");
        if src_line.contains("lint:allow(no-vec-in-trait-sig)") { continue; }

        let name = item.child_by_field_name("name")
            .map(|n| txt(n, ctx.source))
            .unwrap_or("<unknown>");

        let mut sig = String::new();
        if let Some(params) = item.child_by_field_name("parameters") {
            sig.push_str(txt(params, ctx.source));
        }
        if let Some(ret) = item.child_by_field_name("return_type") {
            sig.push(' ');
            sig.push_str(txt(ret, ctx.source));
        }

        for forbidden in FORBIDDEN_IN_TRAIT {
            if sig.contains(forbidden) {
                out.push(err(
                    ctx,
                    line,
                    "no-vec-in-trait-sig",
                    format!("trait method `{name}` signature contains `{forbidden}`; use &[T] / impl Iterator / &mut impl Collector<T> / &mut impl Sink"),
                ));
                break;
            }
        }
    }
}
