//! Lint: no dynamic dispatch. Forbid `dyn Trait`, `Box<dyn Trait>`,
//! `Arc<dyn Trait>`, `&dyn Trait`, `&mut dyn Trait` in source. The stack
//! uses monomorphisation; dispatch is a zero-cost abstraction only when
//! static.

use mockspace_lint_rules::{Lint, LintContext, LintError, Severity};

use crate::util::err;

pub struct NoDynDispatch;

impl Lint for NoDynDispatch {
    fn name(&self) -> &'static str { "no-dyn-dispatch" }

    fn default_severity(&self) -> Severity { Severity::HARD_ERROR }

    fn check(&self, ctx: &LintContext) -> Vec<LintError> {
        let mut out = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") { continue; }
            if line.contains("lint:allow(no-dyn-dispatch)") { continue; }
            if contains_dyn_type(line) {
                out.push(err(
                    ctx,
                    idx + 1,
                    "no-dyn-dispatch",
                    "`dyn Trait` forbidden; use generics or concrete impls (monomorphisation is free)".to_string(),
                ));
            }
        }
        out
    }
}

fn contains_dyn_type(line: &str) -> bool {
    // Tokens where `dyn` sits in a type position: `&dyn`, `&mut dyn`,
    // `Box<dyn`, `<dyn`, `(dyn`, `, dyn`, `: dyn`.
    for marker in &[
        "&dyn ", "&mut dyn ", "Box<dyn ", "Arc<dyn ", "Rc<dyn ", "<dyn ",
        "(dyn ", ", dyn ", ": dyn ", "impl dyn ",
    ] {
        if line.contains(marker) { return true; }
    }
    // Catch leading-position `dyn ` that wasn't preceded by whitespace
    // marker above.
    line.trim_start().starts_with("dyn ")
}
