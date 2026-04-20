//! Lint: no public raw-typed fields on `pub struct`. Every pub field must
//! be either a domain newtype, an arvo primitive, a trait-bound generic,
//! or a const-generic position. Raw `u32`, `bool`, `String`, `Vec<T>`,
//! `Option<T>`, etc. in a pub field signals leaking implementation detail.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};
use tree_sitter::Node;

use crate::util::{err, for_each_struct, is_public, txt};

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
            if !is_public(node, ctx.source) { return; }
            check_struct(node, ctx, &mut out);
        });
        out
    }
}

fn check_struct(node: Node, ctx: &LintContext, out: &mut Vec<LintError>) {
    let body = match node.child_by_field_name("body") {
        Some(b) => b,
        None => return,
    };
    let mut cursor = body.walk();
    for field in body.children(&mut cursor) {
        if field.kind() != "field_declaration" { continue; }
        if !is_public(field, ctx.source) { continue; }

        let line = field.start_position().row + 1;
        let src_line = ctx.source.lines().nth(field.start_position().row).unwrap_or("");
        if src_line.contains("lint:allow(no-public-raw-field)") { continue; }

        let type_text = match field.child_by_field_name("type") {
            Some(t) => txt(t, ctx.source).trim().to_string(),
            None => continue,
        };
        let field_name = field.child_by_field_name("name")
            .map(|n| txt(n, ctx.source))
            .unwrap_or("<unknown>");

        for forbidden in FORBIDDEN_FIELD_TYPES {
            if type_is(&type_text, forbidden) {
                out.push(err(
                    ctx,
                    line,
                    "no-public-raw-field",
                    format!("pub field `{field_name}: {type_text}` uses raw `{forbidden}`; wrap in a domain newtype or arvo primitive"),
                ));
                break;
            }
        }
    }
}

fn type_is(text: &str, ty: &str) -> bool {
    text == ty || text.starts_with(&format!("{ty}<")) || text == format!("&{ty}") || text == format!("&mut {ty}")
}
