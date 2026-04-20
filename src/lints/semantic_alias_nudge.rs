//! Lint: advisory warn on raw arvo primitives at public API boundaries.
//! Encourages flipping `QWord` → `RecordIndex`, `UFixed<I32, F0, Hot>` →
//! `SlotId`, etc. — a semantic alias reads better than the primitive.
//!
//! Default severity: ADVISORY (warn everywhere, blocks nothing).

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};
use tree_sitter::Node;

use crate::util::{for_each_fn, is_public, txt};

const RAW_ARVO_PRIMITIVES: &[&str] = &[
    "UFixed", "IFixed", "FastFloat", "StrictFloat",
    "Byte", "Word", "DWord", "QWord", "Nibble", "Bit",
    "USize", "ISize", "Cap", "Bool",
];

pub struct SemanticAliasNudge;

impl Lint for SemanticAliasNudge {
    fn name(&self) -> &'static str { "semantic-alias-nudge" }

    fn default_severity(&self) -> Severity { Severity::ADVISORY }

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
    if src_line.contains("lint:allow(semantic-alias-nudge)") { return; }

    let mut sig = String::new();
    if let Some(params) = node.child_by_field_name("parameters") {
        sig.push_str(txt(params, ctx.source));
    }
    if let Some(ret) = node.child_by_field_name("return_type") {
        sig.push(' ');
        sig.push_str(txt(ret, ctx.source));
    }

    for prim in RAW_ARVO_PRIMITIVES {
        if contains_token(&sig, prim) {
            let name = node.child_by_field_name("name")
                .map(|n| txt(n, ctx.source))
                .unwrap_or("<unknown>");
            out.push(LintError::warning(
                ctx.crate_name.to_string(),
                line,
                "semantic-alias-nudge",
                format!("`{name}` exposes raw `{prim}` in its signature — consider a domain alias (NodeId / SlotId / RecordIndex / etc.) for readability"),
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

fn is_ident(b: u8) -> bool { b.is_ascii_alphanumeric() || b == b'_' }
