//! Lint: no runtime registration patterns. Forbid `lazy_static!`,
//! `once_cell::sync::OnceCell`, `std::sync::OnceLock`, `LinkedList`-of-plugins,
//! `inventory` crate, dashmap, and other "register this at program startup"
//! patterns. The stack is static; plugin sets are known at compile time.

use mockspace_lint_rules::{CrateLint, Lint, LintContext, LintError, Severity};

use crate::util::{err, line_lint_allowed};

const PATTERNS: &[&str] = &[
    "lazy_static!",
    "lazy_static::",
    "once_cell::",
    "std::sync::OnceLock",
    "OnceLock::",
    "OnceCell::",
    "inventory::",
    "inventory!",
    "dashmap::",
    "ctor::",
    "linkme::",
];

/// Whether the line names this path, rather than merely containing its letters.
///
/// A plain substring search reads `ctor::` out of `FeatureVector::new` and
/// `OnceCell::` out of anything ending in those letters, which is a hard error
/// on a line that registers nothing. The failure is worse than a false positive
/// usually is, because the message names a crate the file does not use and the
/// only way out looks like renaming your own type.
///
/// A path starts where an identifier starts, so the character before it must not
/// be one an identifier can contain. That is the whole rule: the patterns are
/// already anchored at their own end by the `::` or the `!` they carry.
fn names_the_path(line: &str, pattern: &str) -> bool {
    let mut from = 0usize;
    while let Some(at) = line[from ..].find(pattern) {
        let at = from + at;
        let before_is_identifier = line[.. at]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_');
        if !before_is_identifier {
            return true;
        }
        from = at + pattern.len();
    }
    false
}

pub struct NoRuntimeRegistration;

impl Lint for NoRuntimeRegistration {
    fn name(&self) -> &'static str {
        "no-runtime-registration"
    }

    fn default_severity(&self) -> Severity {
        Severity::HARD_ERROR
    }
}

impl CrateLint for NoRuntimeRegistration {
    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        if ctx.should_skip_proc_macro_source_lint() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            if line_lint_allowed(line, "no-runtime-registration") {
                continue;
            }
            for p in PATTERNS {
                if names_the_path(line, p) {
                    out.push(err(
                        ctx,
                        idx + 1,
                        "no-runtime-registration",
                        format!("`{p}` forbidden; compile-time registration only (const/static + generic)"),
                    ));
                    break;
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_at_the_start_of_an_identifier_is_named() {
        assert!(names_the_path("    ctor::ctor! { fn init() {} }", "ctor::"));
        assert!(names_the_path("use once_cell::sync::Lazy;", "once_cell::"));
        assert!(names_the_path("let x = OnceCell::new();", "OnceCell::"));
    }

    #[test]
    fn the_same_letters_inside_an_identifier_are_not() {
        // The false positive this exists for, and it is worse than a false
        // positive usually is: the message names a crate the file does not use,
        // and the only way out looks like renaming your own type.
        assert!(!names_the_path("FeatureVector::new(set, values)", "ctor::"));
        assert!(!names_the_path("let v = MyVector::default();", "ctor::"));
        assert!(!names_the_path("SomeOnceCell::get()", "OnceCell::"));
    }

    #[test]
    fn a_real_use_on_a_line_that_also_has_a_lookalike_is_still_caught() {
        // The control for scanning past the first hit rather than stopping at
        // it. Finding the lookalike first must not hide the real one.
        assert!(names_the_path(
            "let v = FeatureVector::new(x); ctor::ctor! {}",
            "ctor::"
        ));
    }

    #[test]
    fn a_path_after_a_bracket_or_a_colon_is_named() {
        // The characters that can precede a real path, which the rule has to
        // allow or it refuses everything but a line-leading use.
        assert!(names_the_path("Some(ctor::thing())", "ctor::"));
        assert!(names_the_path("crate::x = <ctor::T>::new()", "ctor::"));
        assert!(names_the_path("ctor::at_the_very_start()", "ctor::"));
    }

    #[test]
    fn a_line_with_none_of_it_is_not() {
        assert!(!names_the_path("let x = 1;", "ctor::"));
        assert!(!names_the_path("", "ctor::"));
    }
}
