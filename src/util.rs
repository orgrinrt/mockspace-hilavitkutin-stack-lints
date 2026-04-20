//! Shared helpers for stack lints.

use mockspace_lint_rules::{LintContext, LintError, Severity};
use tree_sitter::Node;

/// Slice of source for a node.
pub fn txt<'a>(node: Node<'a>, src: &'a str) -> &'a str {
    &src[node.byte_range()]
}

/// Whether the line a node starts on contains `lint:allow(<name>)`.
#[allow(dead_code)]
pub fn is_lint_allowed(node: Node, ctx: &LintContext, rule_name: &str) -> bool {
    let row = node.start_position().row;
    let line = ctx.source.lines().nth(row).unwrap_or("");
    let token = format!("lint:allow({rule_name})");
    line.contains(&token)
}

/// Build a blocking error with the standard (crate, line, lint, message) shape.
pub fn err(
    ctx: &LintContext,
    line: usize,
    lint_name: &'static str,
    message: String,
) -> LintError {
    LintError::with_severity(
        ctx.crate_name.to_string(),
        line,
        lint_name,
        message,
        Severity::HARD_ERROR,
    )
}

/// Visit every `function_item` / `function_signature_item` in the tree and
/// call `visit` for each one.
pub fn for_each_fn<F: FnMut(Node)>(root: Node, mut visit: F) {
    walk(root, &mut visit);
}

fn walk<F: FnMut(Node)>(node: Node, visit: &mut F) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_item" | "function_signature_item" => visit(child),
            _ => {}
        }
        if child.named_child_count() > 0 {
            walk(child, visit);
        }
    }
}

/// Visit every `struct_item` in the tree.
pub fn for_each_struct<F: FnMut(Node)>(root: Node, mut visit: F) {
    walk_kind(root, "struct_item", &mut visit);
}

/// Visit every `trait_item` in the tree.
pub fn for_each_trait<F: FnMut(Node)>(root: Node, mut visit: F) {
    walk_kind(root, "trait_item", &mut visit);
}

fn walk_kind<F: FnMut(Node)>(node: Node, kind: &str, visit: &mut F) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == kind {
            visit(child);
        }
        if child.named_child_count() > 0 {
            walk_kind(child, kind, visit);
        }
    }
}

/// Whether a function is marked `pub` / `pub(crate)` / `pub(super)` / `pub(...)`.
pub fn is_public(node: Node, src: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            let text = txt(child, src);
            return text.starts_with("pub");
        }
    }
    false
}
