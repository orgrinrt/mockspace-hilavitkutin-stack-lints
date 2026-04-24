//! Lint: no runtime registration patterns. Forbid `lazy_static!`,
//! `once_cell::sync::OnceCell`, `std::sync::OnceLock`, `LinkedList`-of-plugins,
//! `inventory` crate, dashmap, and other "register this at program startup"
//! patterns. The stack is static; plugin sets are known at compile time.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};

use crate::util::err;

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

pub struct NoRuntimeRegistration;

impl Lint for NoRuntimeRegistration {
    fn name(&self) -> &'static str { "no-runtime-registration" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        if ctx.should_skip_proc_macro_source_lint() { return Vec::new(); }
        let mut out = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") { continue; }
            if line.contains("lint:allow(no-runtime-registration)") { continue; }
            for p in PATTERNS {
                if line.contains(p) {
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
