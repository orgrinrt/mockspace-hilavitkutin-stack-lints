//! Lint: no bare `Result<T, E>` anywhere in source. Use
//! `notko::Outcome<T, E>` (cold path) or `notko::Just<T>` (hot path).
//!
//! Scans every non-comment, non-string line for the `Result` token
//! as a type/path identifier. `fmt::Result`, `std::fmt::Result`,
//! `io::Result`, `std::io::Result` are recognised and allowed (fixed-
//! signature std trait-method parity).
//!
//! `#[optimize_for(...)]` rewrites `Result` to the correct fallibility
//! tier at compile time via notko-macros. Attributed items should
//! carry `lint:allow(no-bare-result)` since the textual form still
//! mentions `Result`.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};

use crate::util::err;

pub struct NoBareResult;

impl Lint for NoBareResult {
    fn name(&self) -> &'static str { "no-bare-result" }

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

        if ctx.introduces("Result") { return Vec::new(); }
        for (rel_path, source) in sources {
            for (idx, raw_line) in source.lines().enumerate() {
                let trimmed = raw_line.trim_start();
                if trimmed.starts_with("//") { continue; }
                if raw_line.contains("lint:allow(no-bare-result)") { continue; }

                let scan = strip_strings_and_chars(raw_line);
                let scan = strip_line_comment(&scan);

                if contains_bare_result(&scan) {
                    out.push(err(
                        ctx,
                        idx + 1,
                        "no-bare-result",
                        format!(
                            "bare `Result` in {} line {} — use notko::Outcome<T, E> (cold) or notko::Just<T> (hot). Result does not exist in this stack",
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

fn contains_bare_result(hay: &str) -> bool {
    let bytes = hay.as_bytes();
    let needle = b"Result";
    let mut i = 0;
    while i + needle.len() <= bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            let before_ok = i == 0 || !is_ident(bytes[i - 1]);
            let after_pos = i + needle.len();
            let after_ok = after_pos >= bytes.len() || !is_ident(bytes[after_pos]);
            if before_ok && after_ok {
                if is_path_prefixed(&bytes[..i], &["fmt", "io"]) {
                    i += needle.len();
                    continue;
                }
                return true;
            }
        }
        i += 1;
    }
    false
}

/// True when the text immediately preceding `idx` ends in one of the
/// listed module segment prefixes followed by `::`. Used to accept
/// `fmt::Result` / `io::Result` / `std::fmt::Result` / `std::io::Result`.
fn is_path_prefixed(prefix: &[u8], mods: &[&str]) -> bool {
    let text = match core::str::from_utf8(prefix) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let trimmed = text.trim_end();
    for m in mods {
        let pat = format!("{m}::");
        if trimmed.ends_with(&pat) {
            return true;
        }
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
