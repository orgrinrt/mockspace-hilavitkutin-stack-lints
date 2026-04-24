//! Lint: no bare `String` or non-static `&str` anywhere in source.
//!
//! Use `hilavitkutin_str::Str` (interned handle) or a `&'static str`
//! (compile-time literal). Bare `String` is heap-allocated and
//! forbidden by the no_alloc rule; non-static `&str` leaks unowned
//! borrowed state across API boundaries.
//!
//! Scans every non-comment, non-string line; any appearance in any
//! position is drift. Line-local `lint:allow(no-bare-string)` is the
//! only escape hatch, for foreign-crate boundaries.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};

use crate::util::{categories, crate_introduces_category, err};

pub struct NoBareString;

impl Lint for NoBareString {
    fn name(&self) -> &'static str { "no-bare-string" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        if ctx.should_skip_proc_macro_source_lint() { return Vec::new(); }
        let mut out = Vec::new();

        let sources: Vec<(String, &str)> = if ctx.all_sources.is_empty() {
            vec![("src/lib.rs".to_string(), ctx.source)]
        } else {
            ctx.all_sources
                .iter()
                .map(|f| (f.rel_path.display().to_string(), f.text.as_str()))
                .collect()
        };

        if crate_introduces_category(ctx, categories::STRING) { return Vec::new(); }
        let check_string = true;
        let check_str = true;
        for (rel_path, source) in sources {
            for (idx, raw_line) in source.lines().enumerate() {
                let trimmed = raw_line.trim_start();
                if trimmed.starts_with("//") { continue; }
                if raw_line.contains("lint:allow(no-bare-string)") { continue; }

                let scan = strip_strings_and_chars(raw_line);
                let scan = strip_line_comment(&scan);

                if check_string && contains_bare_string_type(&scan) {
                    out.push(err(
                        ctx,
                        idx + 1,
                        "no-bare-string",
                        format!(
                            "bare `String` in {} line {} — use hilavitkutin_str::Str. String is heap-allocated and does not exist in this stack",
                            rel_path,
                            idx + 1,
                        ),
                    ));
                    continue;
                }
                if check_str && contains_non_static_str_ref(&scan) {
                    out.push(err(
                        ctx,
                        idx + 1,
                        "no-bare-string",
                        format!(
                            "non-static `&str` in {} line {} — use `&'static str` or hilavitkutin_str::Str. Unowned borrowed strings do not cross API boundaries",
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

fn contains_bare_string_type(hay: &str) -> bool {
    let bytes = hay.as_bytes();
    let needle = b"String";
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

fn contains_non_static_str_ref(hay: &str) -> bool {
    let bytes = hay.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'&' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() { j += 1; }
            // Optionally `mut`.
            if j + 3 <= bytes.len() && &bytes[j..j + 3] == b"mut" {
                let k = j + 3;
                if k < bytes.len() && !is_ident(bytes[k]) {
                    j = k;
                    while j < bytes.len() && bytes[j].is_ascii_whitespace() { j += 1; }
                }
            }
            let lifetime_start = j;
            if j < bytes.len() && bytes[j] == b'\'' {
                j += 1;
                while j < bytes.len() && is_ident(bytes[j]) { j += 1; }
                while j < bytes.len() && bytes[j].is_ascii_whitespace() { j += 1; }
            }
            if j + 3 <= bytes.len() && &bytes[j..j + 3] == b"str" {
                let after = j + 3;
                let after_ok = after >= bytes.len() || !is_ident(bytes[after]);
                if after_ok {
                    let lifetime = core::str::from_utf8(&bytes[lifetime_start..j]).unwrap_or("").trim();
                    if lifetime != "'static" {
                        return true;
                    }
                }
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
            // Don't consume the tick here — char literals are recognised
            // above, lifetimes start with a tick that's NOT a char-literal
            // opener (no closing tick on the line). Treat identical to
            // the numeric lint.
            out.push('\'');
            i += 1;
            let start = i;
            while i < bytes.len() {
                let c = bytes[i];
                if c == b'\\' && i + 1 < bytes.len() { i += 2; continue; }
                if c == b'\'' && i != start { out.push('\''); i += 1; break; }
                if !is_ident(c) && i == start + 1 {
                    // Short single-tick followed by non-ident: lifetime.
                    // Restore cursor; emit nothing more.
                    break;
                }
                out.push(c as char);
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
