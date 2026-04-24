//! Lint: no bare numeric primitive anywhere in source.
//!
//! Historically scoped to public fn signatures; now scans the entire
//! source to match the "arvo is the exclusive numeric substrate" rule
//! verbatim — bare `u*`/`i*`/`f*`/`usize`/`isize`/`bool` do not exist
//! in this stack, not in pub API, not in private fields, not in
//! expressions, not in casts, not in literal suffixes.
//!
//! This lint is retained alongside `arvo-types-only` so configs that
//! named it still apply; the two are semantically equivalent today.
//! Prefer `arvo-types-only` in new configs.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};

use crate::util::{categories, crate_introduces_category, err};

const BARE_NUMERICS: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128",
    "i8", "i16", "i32", "i64", "i128",
    "f32", "f64",
    "usize", "isize",
    "bool",
];

pub struct NoBareNumeric;

impl Lint for NoBareNumeric {
    fn name(&self) -> &'static str { "no-bare-numeric" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        if ctx.should_skip_proc_macro_source_lint() { return Vec::new(); }
        if crate_introduces_category(ctx, categories::NUMERIC) { return Vec::new(); }
        let mut out = Vec::new();

        let sources: Vec<(String, &str)> = if ctx.all_sources.is_empty() {
            vec![("src/lib.rs".to_string(), ctx.source)]
        } else {
            ctx.all_sources
                .iter()
                .map(|f| (f.rel_path.display().to_string(), f.text.as_str()))
                .collect()
        };

        for (rel_path, source) in sources {
            for (idx, raw_line) in source.lines().enumerate() {
                let trimmed = raw_line.trim_start();
                if trimmed.starts_with("//") { continue; }
                if raw_line.contains("lint:allow(no-bare-numeric)") { continue; }

                let scan = strip_strings_and_chars(raw_line);
                let scan = strip_line_comment(&scan);

                for prim in BARE_NUMERICS {
                    if contains_bare_word(&scan, prim) {
                        out.push(err(
                            ctx,
                            idx + 1,
                            "no-bare-numeric",
                            format!(
                                "bare `{prim}` in {} line {} — arvo is the exclusive numeric substrate. Wrap in UFixed / IFixed / FastFloat / StrictFloat / USize / Cap / Bool or a domain alias; bare primitives do not exist in this stack",
                                rel_path,
                                idx + 1,
                            ),
                        ));
                        break;
                    }
                }
            }
        }

        out
    }
}

fn strip_strings_and_chars(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' {
            out.push('"');
            i += 1;
            while i < bytes.len() {
                let c = bytes[i];
                if c == b'\\' && i + 1 < bytes.len() { i += 2; continue; }
                if c == b'"' { out.push('"'); i += 1; break; }
                i += 1;
            }
        } else if b == b'\'' {
            out.push('\'');
            i += 1;
            let start = i;
            while i < bytes.len() {
                let c = bytes[i];
                if c == b'\\' && i + 1 < bytes.len() { i += 2; continue; }
                if c == b'\'' && i != start { out.push('\''); i += 1; break; }
                i += 1;
            }
        } else {
            out.push(b as char);
            i += 1;
        }
    }
    out
}

fn strip_line_comment(line: &str) -> String {
    if let Some(idx) = line.find("//") { line[..idx].to_string() } else { line.to_string() }
}

fn contains_bare_word(hay: &str, needle: &str) -> bool {
    let bytes = hay.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() || n.len() > bytes.len() { return false; }
    let mut i = 0;
    while i + n.len() <= bytes.len() {
        if &bytes[i..i + n.len()] == n {
            let before_ok = i == 0 || !is_ident(bytes[i - 1]);
            let after_pos = i + n.len();
            let after_ok = after_pos >= bytes.len() || !is_ident(bytes[after_pos]);
            if before_ok && after_ok { return true; }
        }
        i += 1;
    }
    false
}

fn is_ident(b: u8) -> bool { b.is_ascii_alphanumeric() || b == b'_' }
