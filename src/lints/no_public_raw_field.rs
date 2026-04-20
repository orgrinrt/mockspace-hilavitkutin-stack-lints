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
use tree_sitter::{Node, Parser, Tree};

use crate::util::{categories, crate_introduces_category, err, for_each_struct, txt};

/// Forbidden field types paired with the substrate category each
/// falls under. When a crate is tagged as introducing a category,
/// the check skips forbidden types in that category but continues
/// scanning types in other categories — a `["numeric"]`-tagged
/// crate still gets its `String` fields flagged.
const FORBIDDEN_FIELD_TYPES: &[(&str, &str)] = &[
    ("u8",    categories::NUMERIC),
    ("u16",   categories::NUMERIC),
    ("u32",   categories::NUMERIC),
    ("u64",   categories::NUMERIC),
    ("u128",  categories::NUMERIC),
    ("i8",    categories::NUMERIC),
    ("i16",   categories::NUMERIC),
    ("i32",   categories::NUMERIC),
    ("i64",   categories::NUMERIC),
    ("i128",  categories::NUMERIC),
    ("f32",   categories::NUMERIC),
    ("f64",   categories::NUMERIC),
    ("usize", categories::NUMERIC),
    ("isize", categories::NUMERIC),
    ("bool",  categories::NUMERIC),
    ("String", categories::STRING),
    ("&str",   categories::STRING),
];

pub struct NoPublicRawField;

impl Lint for NoPublicRawField {
    fn name(&self) -> &'static str { "no-public-raw-field" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        if ctx.is_proc_macro_crate() { return Vec::new(); }
        // Per-category skip happens inside `report_if_forbidden`: a
        // `["numeric"]` crate skips numeric field types but still gets
        // `String` / `&str` field drift flagged.
        let mut out = Vec::new();

        // Scan every src/*.rs file, not just lib.rs. Parse each file
        // with its own tree-sitter tree so node positions and text
        // line up with that file's contents.
        if ctx.all_sources.is_empty() {
            // Back-compat: older mockspace versions carry only
            // ctx.source/ctx.tree for lib.rs.
            for_each_struct(ctx.tree.root_node(), |node| {
                check_struct(node, ctx.source, "src/lib.rs", &mut out, ctx);
            });
            return out;
        }

        let mut parser = Parser::new();
        if parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .is_err()
        {
            return out;
        }

        for file in ctx.all_sources {
            let tree: Tree = match parser.parse(&file.text, None) {
                Some(t) => t,
                None => continue,
            };
            let rel_path = file.rel_path.display().to_string();
            for_each_struct(tree.root_node(), |node| {
                check_struct(node, &file.text, &rel_path, &mut out, ctx);
            });
        }

        out
    }
}

fn check_struct(
    node: Node,
    source: &str,
    rel_path: &str,
    out: &mut Vec<LintError>,
    ctx: &LintContext,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "field_declaration_list" => scan_named_body(child, source, rel_path, out, ctx),
            "ordered_field_declaration_list" => scan_tuple_body(child, source, rel_path, out, ctx),
            _ => {}
        }
    }
}

fn scan_named_body(
    node: Node,
    source: &str,
    rel_path: &str,
    out: &mut Vec<LintError>,
    ctx: &LintContext,
) {
    let mut cursor = node.walk();
    for field in node.children(&mut cursor) {
        if field.kind() != "field_declaration" { continue; }

        let line = field.start_position().row + 1;
        let src_line = source
            .lines()
            .nth(field.start_position().row)
            .unwrap_or("");
        if src_line.contains("lint:allow(no-public-raw-field)") { continue; }

        let type_text = match field.child_by_field_name("type") {
            Some(t) => txt(t, source).trim().to_string(),
            None => continue,
        };
        let field_name = field
            .child_by_field_name("name")
            .map(|n| txt(n, source).to_string())
            .unwrap_or_else(|| "<anon>".to_string());

        report_if_forbidden(ctx, out, rel_path, line, &field_name, &type_text);
    }
}

fn scan_tuple_body(
    node: Node,
    source: &str,
    rel_path: &str,
    out: &mut Vec<LintError>,
    ctx: &LintContext,
) {
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
        let src_line = source
            .lines()
            .nth(field.start_position().row)
            .unwrap_or("");
        if src_line.contains("lint:allow(no-public-raw-field)") { continue; }

        let type_text = txt(field, source).trim().to_string();
        if type_text.is_empty() { continue; }
        report_if_forbidden(ctx, out, rel_path, line, "<tuple>", &type_text);
    }
}

fn report_if_forbidden(
    ctx: &LintContext,
    out: &mut Vec<LintError>,
    rel_path: &str,
    line: usize,
    field_name: &str,
    type_text: &str,
) {
    for (forbidden, category) in FORBIDDEN_FIELD_TYPES {
        if type_is(type_text, forbidden) {
            // Skip this specific field type when the crate introduces
            // its category; keep scanning so an unrelated-category
            // type later in the list still fires.
            if crate_introduces_category(ctx, category) { return; }
            out.push(err(
                ctx,
                line,
                "no-public-raw-field",
                format!(
                    "field `{field_name}: {type_text}` in {rel_path} uses raw `{forbidden}` — wrap in a domain newtype or arvo primitive. Bare primitives do not exist in this stack (pub or private field, no exception)"
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
