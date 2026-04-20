//! Lint: no std. Hard-block `use std::*`, `std::*` path references, and
//! absence of `#![no_std]` at the crate root.
//!
//! Escape via `// lint:allow(no-std)`. Test crates may disable via
//! `[lints.no-std] severity = "off"`.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};

use crate::util::err;

pub struct NoStd;

impl Lint for NoStd {
    fn name(&self) -> &'static str { "no-std" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut out = Vec::new();

        // Root-level #![no_std] must be present (allowing a lint:allow on the
        // first 20 lines to opt out for test / proc-macro crates).
        let head: String = ctx.source.lines().take(30).collect::<Vec<_>>().join("\n");
        let has_no_std = head.contains("#![no_std]");
        let allowed_at_root = head.contains("lint:allow(no-std)");
        if !has_no_std && !allowed_at_root && !ctx.is_proc_macro_crate() {
            out.push(err(
                ctx,
                1,
                "no-std",
                "crate root is missing `#![no_std]`. Every stack crate must be no_std unless explicitly allowed".to_string(),
            ));
        }

        // Flag per-line `std::*` and `use std::`.
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            if line.contains("lint:allow(no-std)") {
                continue;
            }
            let has_use_std = line.contains("use std::");
            let has_std_path = line.contains(" std::") || line.starts_with("std::") || line.contains("(std::");
            let has_extern_std = line.contains("extern crate std");
            if has_use_std || has_std_path || has_extern_std {
                out.push(err(
                    ctx,
                    idx + 1,
                    "no-std",
                    "`std::*` reference found; use `core::*` equivalents or the stack's own primitives".to_string(),
                ));
            }
        }

        out
    }
}
