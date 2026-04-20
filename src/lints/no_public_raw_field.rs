//! Lint: no raw-typed field on any struct (pub or private).
//!
//! Every struct field must be a domain newtype, an arvo primitive,
//! a trait-bound generic, or a const-generic position. Bare
//! `u32`, `bool`, `String`, `Vec<T>`, `Option<T>`, etc. signals
//! the "primitives don't exist in this stack" rule being broken.
//!
//! Scope is universal — both pub and private fields, both pub and
//! private structs. The historical name `no-public-raw-field` is
//! retained for config compatibility; the scope is no longer
//! restricted to public fields.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};
use tree_sitter::Node;

use crate::util::{err, for_each_struct, txt};

const FORBIDDEN_FIELD_TYPES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128",
    "i8", "i16", "i32", "i64", "i128",
    "f32", "f64",
    "usize", "isize",
    "bool",
    "String",
    "&str",
];

pub struct NoPublicRawField;

impl Lint for NoPublicRawField {
    fn name(&self) -> &'static str { "no-public-raw-field" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        if ctx.is_proc_macro_crate() { return Vec::new(); }
        let mut out = Vec::new();
        for_each_struct(ctx.tree.root_node(), |node| {
            check_struct(node, ctx, &mut out);
        });
        out
    }
}

fn check_struct(node: Node, ctx: &LintContext, out: &mut Vec<LintError>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "field_declaration_list" => scan_named_body(child, ctx, out),
            "ordered_field_declaration_list" => scan_tuple_body(child, ctx, out),
            _ => {}
        }
    }
}

fn scan_named_body(node: Node, ctx: &LintContext, out: &mut Vec<LintError>) {
    let mut cursor = node.walk();
    for field in node.children(&mut cursor) {
        if field.kind() != "field_declaration" { continue; }

        let line = field.start_position().row + 1;
        let src_line = ctx
            .source
            .lines()
            .nth(field.start_position().row)
            .unwrap_or("");
        if src_line.contains("lint:allow(no-public-raw-field)") { continue; }

        let type_text = match field.child_by_field_name("type") {
            Some(t) => txt(t, ctx.source).trim().to_string(),
            None => continue,
        };
        let field_name = field
            .child_by_field_name("name")
            .map(|n| txt(n, ctx.source).to_string())
            .unwrap_or_else(|| "<anon>".to_string());

        report_if_forbidden(ctx, out, line, &field_name, &type_text);
    }
}

fn scan_tuple_body(node: Node, ctx: &LintContext, out: &mut Vec<LintError>) {
    let mut cursor = node.walk();
    for field in node.children(&mut cursor) {
        // Inside an ordered_field_declaration_list, tree-sitter
        // typically emits `(`, `visibility_modifier` (optional), a
        // type node (e.g. `primitive_type`, `generic_type`,
        // `reference_type`), `,`, `)`. We report on type nodes and
        // skip punctuation / modifiers.
        let kind = field.kind();
        if matches!(kind, "(" | ")" | "," | "visibility_modifier" | "mutable_specifier") {
            continue;
        }

        let line = field.start_position().row + 1;
        let src_line = ctx
            .source
            .lines()
            .nth(field.start_position().row)
            .unwrap_or("");
        if src_line.contains("lint:allow(no-public-raw-field)") { continue; }

        let type_text = txt(field, ctx.source).trim().to_string();
        if type_text.is_empty() { continue; }
        report_if_forbidden(ctx, out, line, "<tuple>", &type_text);
    }
}

fn report_if_forbidden(
    ctx: &LintContext,
    out: &mut Vec<LintError>,
    line: usize,
    field_name: &str,
    type_text: &str,
) {
    for forbidden in FORBIDDEN_FIELD_TYPES {
        if type_is(type_text, forbidden) {
            out.push(err(
                ctx,
                line,
                "no-public-raw-field",
                format!(
                    "field `{field_name}: {type_text}` uses raw `{forbidden}` — wrap in a domain newtype or arvo primitive. Bare primitives do not exist in this stack (pub or private field, no exception)"
                ),
            ));
            return;
        }
    }
}

fn type_is(text: &str, ty: &str) -> bool {
    let t = text.trim();
    t == ty
        || t.starts_with(&format!("{ty}<"))
        || t == format!("&{ty}")
        || t == format!("&mut {ty}")
        || t.starts_with(&format!("&'"))
            && (t.ends_with(&format!(" {ty}")) || t.ends_with(&format!("> {ty}")))
}
