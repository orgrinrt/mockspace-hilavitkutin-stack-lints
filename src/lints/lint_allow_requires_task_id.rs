//! Lint: every `// lint:allow(...)` escape hatch must carry a tracked task
//! id. Format:
//!
//! ```text
//! // lint:allow(<rule>) reason: <why>; tracked: #<task-id>
//! ```
//!
//! Loose forms — no reason, no tracked id — get rejected. This ensures
//! every escape becomes an auditable piece of debt.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};

use crate::util::err;

pub struct LintAllowRequiresTaskId;

impl Lint for LintAllowRequiresTaskId {
    fn name(&self) -> &'static str { "lint-allow-requires-task-id" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut out = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let Some(pos) = line.find("lint:allow(") else { continue; };
            let line_num = idx + 1;
            // Must include "tracked: #<digits>"
            let tail = &line[pos..];
            if !has_tracked_task(tail) {
                out.push(err(
                    ctx,
                    line_num,
                    "lint-allow-requires-task-id",
                    "lint:allow(...) missing `tracked: #<task-id>`; every escape is tracked debt".to_string(),
                ));
            }
            if !tail.contains("reason:") {
                out.push(err(
                    ctx,
                    line_num,
                    "lint-allow-requires-task-id",
                    "lint:allow(...) missing `reason: <why>`".to_string(),
                ));
            }
        }
        out
    }
}

fn has_tracked_task(s: &str) -> bool {
    let Some(pos) = s.find("tracked:") else { return false; };
    let after = &s[pos + "tracked:".len()..];
    let after = after.trim_start();
    let after = match after.strip_prefix('#') {
        Some(rest) => rest,
        None => return false,
    };
    after.chars().next().is_some_and(|c| c.is_ascii_digit())
}
