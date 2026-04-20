//! Lint: arvo-types-only / newtype-at-boundaries. Umbrella rule stating
//! that every numeric position in the crate must use arvo primitives or
//! aliases grounded on them — never bare Rust numerics.
//!
//! This is the headline. Same detection as `no-bare-numeric`, but applies
//! to EVERY position (not just pub) — locals, struct fields, trait assoc
//! types, const generics, etc. Much stricter.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};

use crate::util::err;

const BARE_NUMERICS: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128",
    "i8", "i16", "i32", "i64", "i128",
    "f32", "f64",
    "usize", "isize",
];

pub struct ArvoTypesOnly;

impl Lint for ArvoTypesOnly {
    fn name(&self) -> &'static str { "arvo-types-only" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        if ctx.is_proc_macro_crate() { return Vec::new(); }
        let mut out = Vec::new();

        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") { continue; }
            if line.contains("lint:allow(arvo-types-only)") { continue; }

            for prim in BARE_NUMERICS {
                if appears_in_type_position(line, prim) {
                    out.push(err(
                        ctx,
                        idx + 1,
                        "arvo-types-only",
                        format!("bare `{prim}` in type position. arvo is the exclusive numeric substrate; use UFixed / IFixed / FastFloat / StrictFloat / USize / Cap or a domain alias grounded on one"),
                    ));
                    break;
                }
            }
        }

        out
    }
}

fn appears_in_type_position(line: &str, prim: &str) -> bool {
    let bytes = line.as_bytes();
    let needle = prim.as_bytes();
    let mut i = 0;
    while i + needle.len() <= bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            let before_ok = i == 0 || !is_ident(bytes[i - 1]);
            let after_pos = i + needle.len();
            let after_ok = after_pos >= bytes.len() || !is_ident(bytes[after_pos]);
            if before_ok && after_ok {
                // Tight filter: must appear after `:`, `->`, `<`, `&`, `,`, `(`,
                // i.e. a type-position marker.
                let prev = prev_nonspace(bytes, i);
                if matches!(prev, Some(b':' | b'<' | b'&' | b',' | b'(' | b'>')) {
                    // Exclude numeric literal suffix usage like `0u32`, `1usize`.
                    // In that case the `u32` is preceded by a digit — covered by
                    // `before_ok` above (ASCII digits are ident bytes). So safe.
                    return true;
                }
                // Also accept `-> T` where `T` is the primitive.
                if prev == Some(b'>') && i >= 2 && bytes[i.saturating_sub(2)] == b'-' {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

fn prev_nonspace(bytes: &[u8], mut idx: usize) -> Option<u8> {
    while idx > 0 {
        idx -= 1;
        if !bytes[idx].is_ascii_whitespace() {
            return Some(bytes[idx]);
        }
    }
    None
}

fn is_ident(b: u8) -> bool { b.is_ascii_alphanumeric() || b == b'_' }
