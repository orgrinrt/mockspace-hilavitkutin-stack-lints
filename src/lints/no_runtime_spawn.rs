//! Lint: no runtime spawn. Forbid `std::thread::spawn`, `tokio::spawn`,
//! `rayon::spawn`, and similar thread/task launchers. Concurrency in the
//! stack is scheduler-managed, not ad-hoc.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};

use crate::util::err;

const PATTERNS: &[&str] = &[
    "std::thread::spawn",
    "thread::spawn",
    "tokio::spawn",
    "tokio::task::spawn",
    "rayon::spawn",
    "async_std::task::spawn",
    "smol::spawn",
    "futures::executor::spawn",
];

pub struct NoRuntimeSpawn;

impl Lint for NoRuntimeSpawn {
    fn name(&self) -> &'static str { "no-runtime-spawn" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        if ctx.is_proc_macro_crate() { return Vec::new(); }
        let mut out = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") { continue; }
            if line.contains("lint:allow(no-runtime-spawn)") { continue; }
            for p in PATTERNS {
                if line.contains(p) {
                    out.push(err(
                        ctx,
                        idx + 1,
                        "no-runtime-spawn",
                        format!("`{p}` forbidden; concurrency is scheduler-managed in this stack"),
                    ));
                    break;
                }
            }
        }
        out
    }
}
