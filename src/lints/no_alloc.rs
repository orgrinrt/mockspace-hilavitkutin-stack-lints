//! Lint: no heap allocation. Forbid `alloc::*`, `Vec`, `String`, `Box`,
//! `HashMap`, `BTreeMap`, etc. anywhere in source. The stack is `no_alloc`;
//! storage comes from arvo newtypes and hilavitkutin storage primitives.
//!
//! Default severity: HARD_ERROR.
//!
//! Escape hatch for a single line: `// lint:allow(no-alloc) reason: ...; tracked: #N`.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};

use crate::util::err;

const ALLOC_PATHS: &[&str] = &[
    "alloc::",
    "::alloc::",
    "std::vec::Vec",
    "std::string::String",
    "std::boxed::Box",
    "std::collections::HashMap",
    "std::collections::BTreeMap",
    "std::collections::HashSet",
    "std::collections::BTreeSet",
    "std::collections::VecDeque",
    "std::collections::LinkedList",
    "std::collections::BinaryHeap",
    "std::rc::Rc",
    "std::sync::Arc",
];

const ALLOC_IDENTS: &[&str] = &[
    "Vec", "String", "Box", "HashMap", "BTreeMap", "HashSet", "BTreeSet",
    "VecDeque", "LinkedList", "BinaryHeap", "Rc", "Arc",
];

pub struct NoAlloc;

impl Lint for NoAlloc {
    fn name(&self) -> &'static str { "no-alloc" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut out = Vec::new();

        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            if line.contains("lint:allow(no-alloc)") {
                continue;
            }
            let lineno = idx + 1;
            let mut reported = false;

            for path in ALLOC_PATHS {
                if line.contains(path) {
                    out.push(err(
                        ctx,
                        lineno,
                        "no-alloc",
                        format!("`{path}` is a heap allocation; the stack is no_alloc. Use arvo/hilavitkutin storage primitives"),
                    ));
                    reported = true;
                    break;
                }
            }
            if reported {
                continue;
            }
            for ident in ALLOC_IDENTS {
                if bare_ident_in_type_position(line, ident) {
                    out.push(err(
                        ctx,
                        lineno,
                        "no-alloc",
                        format!("`{ident}` is a heap container; use Seq<T, N: Cap> / Map<K, V, N: Cap> / &[T] / impl IntoIterator"),
                    ));
                    break;
                }
            }
        }

        out
    }
}

/// Heuristic: identifier appears where a type would, not as a field name or local.
fn bare_ident_in_type_position(line: &str, ident: &str) -> bool {
    let bytes = line.as_bytes();
    let needle = ident.as_bytes();
    let mut i = 0;
    while i + needle.len() <= bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            let after_pos = i + needle.len();
            let after_ok = after_pos >= bytes.len() || !is_ident_byte(bytes[after_pos]);
            if before_ok && after_ok {
                let prev = prev_non_space(bytes, i);
                let next = next_non_space(bytes, after_pos);
                let type_before = matches!(prev, Some(b':' | b'<' | b'&' | b',' | b'(' | b'>'));
                let type_after = matches!(next, Some(b'<' | b',' | b'>' | b')' | b';' | b'{'))
                    || next.is_none();
                if type_before && type_after {
                    return true;
                }
            }
            i += needle.len();
        } else {
            i += 1;
        }
    }
    false
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn prev_non_space(bytes: &[u8], mut idx: usize) -> Option<u8> {
    while idx > 0 {
        idx -= 1;
        if !bytes[idx].is_ascii_whitespace() {
            return Some(bytes[idx]);
        }
    }
    None
}

fn next_non_space(bytes: &[u8], mut idx: usize) -> Option<u8> {
    while idx < bytes.len() {
        if !bytes[idx].is_ascii_whitespace() {
            return Some(bytes[idx]);
        }
        idx += 1;
    }
    None
}
