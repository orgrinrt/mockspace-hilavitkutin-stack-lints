//! Lint: no bare `Option<T>` anywhere in source. Use `notko::Maybe<T>`.
//!
//! Scans every non-comment, non-string line. Any appearance of the
//! `Option` type name (word boundary, followed by `<` or used as the
//! standalone type) is drift, regardless of context: pub API, private
//! impl, let bindings, struct fields, match arms with
//! `Option::Some(...)`, etc. The only form that passes silently is
//! an explicit `lint:allow(no-bare-option)` on the offending line.
//!
//! Allowed via line-local exemption when implementing a std trait
//! method whose signature is fixed externally
//! (`fn next(&mut self) -> Option<Self::Item>`).

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};

use crate::util::err;

pub struct NoBareOption;

impl Lint for NoBareOption {
    fn name(&self) -> &'static str { "no-bare-option" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        if ctx.is_proc_macro_crate() { return Vec::new(); }
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
                if raw_line.contains("lint:allow(no-bare-option)") { continue; }

                let scan = strip_strings_and_chars(raw_line);
                let scan = strip_line_comment(&scan);

                if contains_option_token(&scan) {
                    out.push(err(
                        ctx,
                        idx + 1,
                        "no-bare-option",
                        format!(
                            "bare `Option` in {} line {} — use notko::Maybe<T>. Option does not exist in this stack",
                            rel_path,
                            idx + 1,
                        ),
                    ));
                }
            }
        }

        out
    }
}

/// True when `Option` appears as a type/path identifier. Excludes
/// `std::option` path segments and `core::option` since those are
/// module references rather than the generic type.
fn contains_option_token(hay: &str) -> bool {
    let bytes = hay.as_bytes();
    let needle = b"Option";
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
