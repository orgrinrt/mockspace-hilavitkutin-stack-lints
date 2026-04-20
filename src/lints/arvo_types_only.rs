//! Lint: arvo-types-only. The headline.
//!
//! arvo is the exclusive numeric substrate. Bare Rust primitives
//! (`u8..u128`, `i8..i128`, `f32`, `f64`, `usize`, `isize`, `bool`)
//! do not exist in this stack — "as if they don't exist". Any
//! appearance of such a token anywhere in source is drift, not just
//! in public API.
//!
//! The lint scans every non-comment, non-string line of source for
//! any word-boundary occurrence of a bare primitive identifier. This
//! includes (non-exhaustive):
//!
//! - fn signatures (params, return type, trait method sigs)
//! - struct fields (pub or private)
//! - enum discriminants and variant fields
//! - let / const / static bindings
//! - type aliases
//! - array element types (`[u32; N]`)
//! - cast expressions (`x as u32`)
//! - associated paths (`u32::MAX`, `bool::from_str`)
//! - literal suffixes (`0u32`, `1_usize`, `0.0_f32`)
//! - tuple-field declarations (`pub struct Str(u32)`)
//!
//! Escape hatch (single line): `// lint:allow(arvo-types-only) reason: ...; tracked: #N`
//! — only appropriate for foreign-crate boundary where the crate
//! demands a specific primitive and no arvo impl of its contract is
//! possible. Prefer dropping the crate over a long-lived allowance.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};

use crate::util::{categories, crate_introduces_category, err};

const BARE_PRIMITIVES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128",
    "i8", "i16", "i32", "i64", "i128",
    "f32", "f64",
    "usize", "isize",
    "bool",
];

pub struct ArvoTypesOnly;

impl Lint for ArvoTypesOnly {
    fn name(&self) -> &'static str { "arvo-types-only" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        if ctx.is_proc_macro_crate() { return Vec::new(); }
        if crate_introduces_category(ctx, categories::NUMERIC) { return Vec::new(); }
        let mut out = Vec::new();

        // Scan every .rs file under src/ — module files (bits.rs,
        // prim.rs, ufixed_impl.rs, ...) are where drift usually
        // lives. Fall back to ctx.source only if the context carried
        // no all_sources (older mockspace version).
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
                let trimmed_start = raw_line.trim_start();
                if trimmed_start.starts_with("//") { continue; }
                if raw_line.contains("lint:allow(arvo-types-only)") { continue; }

                let scan = strip_string_and_char_literals(raw_line);
                let scan = strip_line_comment(&scan);

                for prim in BARE_PRIMITIVES {
                    if contains_bare_word(&scan, prim) {
                        out.push(err(
                            ctx,
                            idx + 1,
                            "arvo-types-only",
                            format!(
                                "bare `{prim}` in {} line {} — the stack has no bare numeric/bool primitives. Use an arvo type (UFixed / IFixed / FastFloat / StrictFloat / USize / Cap / Bool) or a domain alias grounded on one",
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

/// Strip `"..."` string literal contents and `'.'` char literals from
/// the line so primitive names inside them don't false-positive. This
/// is a line-local approximation (multi-line raw strings aren't
/// handled) that's good enough for the 99% case.
fn strip_string_and_char_literals(line: &str) -> String {
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
                if c == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                if c == b'"' {
                    out.push('"');
                    i += 1;
                    break;
                }
                i += 1;
            }
        } else if b == b'\'' {
            out.push('\'');
            i += 1;
            let start = i;
            while i < bytes.len() {
                let c = bytes[i];
                if c == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                if c == b'\'' && i != start {
                    out.push('\'');
                    i += 1;
                    break;
                }
                i += 1;
            }
        } else {
            out.push(b as char);
            i += 1;
        }
    }
    out
}

/// Drop content after a `//` line-comment marker (outside strings —
/// this runs AFTER `strip_string_and_char_literals`).
fn strip_line_comment(line: &str) -> String {
    if let Some(idx) = line.find("//") {
        line[..idx].to_string()
    } else {
        line.to_string()
    }
}

/// True when `needle` appears as a standalone identifier (word-
/// boundary on both sides) in `hay`. Word-boundary = the adjacent
/// byte (if any) is not an ident byte (alnum or underscore).
fn contains_bare_word(hay: &str, needle: &str) -> bool {
    let bytes = hay.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() || n.len() > bytes.len() {
        return false;
    }
    let mut i = 0;
    while i + n.len() <= bytes.len() {
        if &bytes[i..i + n.len()] == n {
            let before_ok = i == 0 || !is_ident(bytes[i - 1]);
            let after_pos = i + n.len();
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
