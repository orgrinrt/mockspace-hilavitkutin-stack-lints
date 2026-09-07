//! Lint: a pull-request or merge-request body, and arbitrary forbidden content.
//!
//! A forge body never passes through git, so no git hook can inspect one. This
//! and [`super::message_attribution`] are the only layers that can, and they are
//! reached from the agent hook before `gh` is invoked.
//!
//! Two jobs, both entirely configured:
//!
//! The first is shape: required sections, a minimum length, and whether process
//! narrative is permitted. A body that reads "final state after review iterations"
//! tells a reader nothing about what changed, and the reader six months from now
//! is the one the body exists for.
//!
//! The second is forbidden patterns: an arbitrary list a project supplies, each with an
//! optional reason shown when it matches, and each scoped to the surfaces it
//! applies to. This is the general facility: internal hostnames, ticket URLs
//! that mean nothing publicly, vocabulary a project has retired. Nothing is
//! forbidden by default, because what counts as leakage is entirely
//! project-specific.

use std::collections::HashMap;

use mockspace_lint_rules::{Lint, LintError, MessageContext, MessageDomain, MessageLint, Severity};

const LINT_NAME: &str = "forge-body";

#[derive(Default)]
pub struct ForgeBody {
    /// Headings the body must contain, matched case-insensitively as substrings.
    required_sections: Vec<String>,
    /// Minimum authored length in characters. Zero disables the check.
    min_length:        usize,
    /// Patterns forbidden anywhere in the body, as `pattern` or
    /// `pattern=reason`, matched case-insensitively as substrings.
    forbidden:         Vec<(String, Option<String>)>,
}

impl Lint for ForgeBody {
    fn name(&self) -> &'static str {
        LINT_NAME
    }

    /// The body is in the command when an agent hook is the caller, so this
    /// lint cannot do its job without the invocation. `body_source` says what it
    /// does with it.
    fn invocation_wanted(&self) -> bool {
        true
    }

    fn description(&self) -> &'static str {
        "a forge body carries the sections a project requires and none of its forbidden content"
    }

    fn source_only(&self) -> bool {
        false
    }

    fn default_severity(&self) -> Severity {
        Severity::HARD_ERROR
    }

    fn finding_kinds(&self) -> &[&str] {
        &["missing-section", "too-short", "forbidden-pattern"]
    }

    fn config_keys(&self) -> &[&str] {
        &["required_sections", "min_length", "forbidden"]
    }

    fn configure(&mut self, params: &HashMap<String, String>) {
        if let Some(v) = params.get("required_sections") {
            self.required_sections = split_list(v);
        }
        if let Some(v) = params.get("min_length") {
            if let Ok(n) = v.trim().parse::<usize>() {
                self.min_length = n;
            }
        }
        if let Some(v) = params.get("forbidden") {
            self.forbidden = split_list(v)
                .into_iter()
                .map(|entry| {
                    match entry.split_once('=') {
                        Some((p, reason)) => {
                            (p.trim().to_string(), Some(reason.trim().to_string()))
                        },
                        None => (entry, None),
                    }
                })
                .collect();
        }
    }
}

impl MessageLint for ForgeBody {
    fn domains(&self) -> &[MessageDomain] {
        // Shape applies to a PR or MR body. A comment is not expected to carry
        // sections, so holding one to the same requirements would be nonsense.
        &[MessageDomain::PullRequestBody]
    }

    fn check_message(&self, ctx: &MessageContext) -> Vec<LintError> {
        let mut out = Vec::new();

        // The forbidden scan reads everything, and the two shape checks read
        // only what was authored. That asymmetry is deliberate and it is not
        // the same defect wearing two faces.
        //
        // A shape check over the whole text answers about the command: a
        // configured minimum is satisfied by a long command carrying an empty
        // body, and a required section is found among the command's own words.
        // Both fail permissively, so narrowing them is the fix.
        //
        // A forbidden pattern is the other way round. Narrowing that scan to
        // the extracted body loses every shape the extractor does not reach and
        // loses the title, which publishes exactly as the body does, so a
        // pattern moved from one to the other stops being seen.
        //
        // The price is stated rather than minimised: on the hook path this text
        // is the whole serialised tool input, so the pattern is also caught in
        // a file path and in the agent's own description of the call, neither
        // of which publishes anything. That is a refusal somebody can read and
        // answer in one edit, and the other direction publishes the thing.
        let everything = ctx.message.to_ascii_lowercase();
        for (pattern, reason) in &self.forbidden {
            if everything.contains(&pattern.to_ascii_lowercase()) {
                out.push(finding(ctx, "forbidden-pattern", &match reason {
                    Some(r) => format!("`{pattern}` is not permitted here: {r}"),
                    None => format!("`{pattern}` is not permitted here"),
                }));
            }
        }

        let Some(body) = body_source(ctx) else {
            return out;
        };
        let lower = body.to_ascii_lowercase();

        for section in &self.required_sections {
            if !lower.contains(&section.to_ascii_lowercase()) {
                out.push(finding(
                    ctx,
                    "missing-section",
                    &format!("the body should contain a `{section}` section"),
                ));
            }
        }

        if self.min_length > 0 {
            let len = body.trim().chars().count();
            if len < self.min_length {
                out.push(finding(
                    ctx,
                    "too-short",
                    &format!(
                        "the body is {len} characters, under the configured minimum of {}",
                        self.min_length
                    ),
                ));
            }
        }

        out
    }
}

fn split_list(v: &str) -> Vec<String> {
    v.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// The text that is the body, which is not always `ctx.message`.
///
/// The same two callers as the commit lint has, and the same trap. Without an
/// invocation the text is the body, which is how a forge tool's own hook would
/// call this. With one, the text is the serialised tool input and the body is on
/// the command line, so measuring the given text measures the command: a
/// configured minimum would then be satisfied by a long command carrying an
/// empty body, and a required section would be found in the command's own words.
///
/// `--body-file` names a path this cannot open, so it declines rather than
/// measuring the path's length, which is the shape that reads as a real check
/// and is not one.
fn body_source(ctx: &MessageContext) -> Option<String> {
    let Some(invocation) = ctx.invocation.as_ref() else {
        return Some(ctx.message.to_string());
    };
    super::authored_message::body_on_the_command_line(invocation.command?)
}

fn finding(ctx: &MessageContext, kind: &'static str, message: &str) -> LintError {
    LintError::with_finding_kind(
        ctx.origin.to_string(),
        1,
        LINT_NAME,
        message.to_string(),
        Severity::HARD_ERROR,
        kind,
    )
}

#[cfg(test)]
mod tests {
    use mockspace_lint_rules::AgentMode;

    use super::*;

    fn check(l: &ForgeBody, domain: MessageDomain, msg: &str) -> Vec<String> {
        let ctx = MessageContext {
            domain,
            mode: AgentMode::Assistant,
            message: msg,
            origin: "pr-body",
            repo_root: std::path::Path::new("/tmp"),
            invocation: None,
        };
        l.check_message(&ctx)
            .into_iter()
            .map(|e| e.finding_kind.unwrap_or("none").to_string())
            .collect()
    }

    /// What an agent's PreToolUse hook hands over: the serialised tool input as
    /// the text, and the command beside it.
    fn check_from_a_hook(l: &ForgeBody, command: &str) -> Vec<String> {
        let serialised = format!(
            "{{\"command\":\"{}\",\"description\":\"open the pull request\"}}",
            command.replace('"', "\\\"")
        );
        let ctx = MessageContext {
            domain:     MessageDomain::PullRequestBody,
            mode:       AgentMode::Assistant,
            message:    &serialised,
            origin:     "<stdin>",
            repo_root:  std::path::Path::new("/tmp"),
            invocation: Some(mockspace_lint_rules::Invocation {
                command:   Some(command),
                tool_name: Some("Bash"),
            }),
        };
        l.check_message(&ctx)
            .into_iter()
            .map(|e| e.finding_kind.unwrap_or("none").to_string())
            .collect()
    }

    #[test]
    fn a_length_is_the_bodys_and_not_the_commands() {
        // The defect this fixes. On the hook path the text is the serialised
        // tool input, so a minimum was satisfied by a long command carrying an
        // empty body, and a short body inside a long command passed.
        let l = with(&[("min_length", "40")]);
        let short = check_from_a_hook(
            &l,
            "gh pr create --title 'a perfectly ordinary title here' --body 'too short'",
        );
        assert!(
            short.contains(&"too-short".to_string()),
            "a nine-character body inside a long command passed: {short:?}"
        );

        let long = check_from_a_hook(
            &l,
            "gh pr create --body 'this body is comfortably longer than the configured minimum of forty'",
        );
        assert!(long.is_empty(), "a long body was refused: {long:?}");
    }

    #[test]
    fn a_required_section_is_looked_for_in_the_body_and_not_in_the_command() {
        // The other half, and the one that fails in the permissive direction:
        // a section name appearing anywhere in the command line satisfied the
        // check, and a command mentioning it while the body does not is
        // exactly what a template-filling agent produces.
        let l = with(&[("required_sections", "Summary")]);
        let found = check_from_a_hook(
            &l,
            "echo Summary && gh pr create --body 'nothing of the kind in here'",
        );
        assert!(
            found.contains(&"missing-section".to_string()),
            "the section was found in the command rather than the body: {found:?}"
        );
    }

    #[test]
    fn a_body_the_command_does_not_carry_is_judged_by_nothing() {
        // `--body-file` names a path this cannot open, and measuring the path
        // would be a check that reads as real and is not.
        let l = with(&[("min_length", "40")]);
        assert!(check_from_a_hook(&l, "gh pr create --body-file /tmp/b.md").is_empty());
        assert!(check_from_a_hook(&l, "gh pr create --title 'x'").is_empty());
    }

    #[test]
    fn a_forbidden_pattern_is_caught_on_every_shape_the_extractor_cannot_read() {
        // The direction the other two checks go the opposite way, and the one
        // that has to be got right, because narrowing this scan loses the
        // pattern rather than reporting it. Every row here is a command the
        // extractor returns `None` or a clean body for, and every one of them
        // publishes the pattern.
        let l = with(&[("forbidden", "internal.corp")]);
        for command in [
            "gh pr create -b 'see internal.corp for the rest'",
            "gh pr create --title 'fix: move off internal.corp' --body 'a clean body'",
            "glab mr create --description 'see internal.corp'",
            "gh release create v1 --notes 'see internal.corp'",
            "GH_TOKEN=x gh pr create --body 'see internal.corp'",
            "gh pr create --body-file /tmp/b.md # internal.corp",
        ] {
            let found = check_from_a_hook(&l, command);
            assert!(
                found.contains(&"forbidden-pattern".to_string()),
                "the pattern was published unseen by `{command}`: {found:?}"
            );
        }
    }

    #[test]
    fn the_shape_checks_reach_every_invocation_shape_the_table_covers() {
        // The arm above passes whatever the extractor does, because the
        // forbidden scan never consults it, so on its own it reads as coverage
        // of these shapes and is coverage of nothing about them. This is the
        // half that actually names them: each of these carries a body of nine
        // characters against a configured minimum of forty, and each has to be
        // refused for that.
        let l = with(&[("min_length", "40")]);
        for command in [
            "gh pr create -b 'too short'",
            "gh pr create -b'too short'",
            "GH_TOKEN=x gh pr create --body 'too short'",
            "(gh pr create --body 'too short')",
            "env gh pr create --body 'too short'",
            "/opt/homebrew/bin/gh pr create --body 'too short'",
            "gh issue create -b 'too short'",
            "gh release create v1 --notes 'too short'",
            "gh release create v1 -n 'too short'",
            "glab mr create --description 'too short'",
            "glab issue create -d 'too short'",
        ] {
            let found = check_from_a_hook(&l, command);
            assert!(
                found.contains(&"too-short".to_string()),
                "`{command}` was not measured at all: {found:?}"
            );
        }
    }

    #[test]
    fn a_body_with_nothing_forbidden_in_it_is_not_refused() {
        // The control for the arm above. A scan that reads everything is only
        // useful if it still says no when there is nothing there, and one that
        // always fires would satisfy every row above while catching nothing.
        let l = with(&[("forbidden", "internal.corp")]);
        assert!(
            check_from_a_hook(&l, "gh pr create --body 'an ordinary body'").is_empty(),
            "a clean command was refused"
        );
    }

    #[test]
    fn the_lint_asks_the_host_for_the_invocation() {
        // Without this the host hands `invocation: None`, `body_source` falls
        // back to the whole serialised tool input, and the defect every arm
        // above pins comes back with all of them still green, because each one
        // builds its own context and none of them can see the request.
        assert!(
            ForgeBody::default().invocation_wanted(),
            "the lint reads a command it never asked for"
        );
        assert!(
            super::super::commit_style::CommitStyle::default().invocation_wanted(),
            "the subject lint reads a command it never asked for"
        );
    }

    #[test]
    fn an_invocation_carrying_no_command_judges_no_shape_and_still_reads_everything() {
        // A host can hand over an invocation with nothing in it. There is then
        // no body to measure, so the shape checks say nothing rather than
        // measuring the serialised input, and the forbidden scan runs anyway
        // because it never depended on the extraction.
        let l = with(&[("min_length", "40"), ("forbidden", "internal.corp")]);
        let judged = |message: &str| -> Vec<String> {
            let ctx = MessageContext {
                domain: MessageDomain::PullRequestBody,
                mode: AgentMode::Assistant,
                message,
                origin: "<stdin>",
                repo_root: std::path::Path::new("/tmp"),
                invocation: Some(mockspace_lint_rules::Invocation {
                    command:   Some("   "),
                    tool_name: Some("Bash"),
                }),
            };
            l.check_message(&ctx)
                .into_iter()
                .map(|e| e.finding_kind.unwrap_or("none").to_string())
                .collect()
        };
        assert!(
            judged("short").is_empty(),
            "the serialised input was measured as a body"
        );
        assert_eq!(judged("short internal.corp"), vec!["forbidden-pattern"]);
    }

    #[test]
    fn the_forge_hooks_own_path_is_untouched() {
        // The control: with no invocation the text is the body, which is how a
        // forge tool's own hook calls this, and every other arm in this file
        // goes through it.
        let l = with(&[("min_length", "40")]);
        assert!(
            check(&l, MessageDomain::PullRequestBody, "short").contains(&"too-short".to_string())
        );
    }

    fn with(pairs: &[(&str, &str)]) -> ForgeBody {
        let mut l = ForgeBody::default();
        let mut p = HashMap::new();
        for (k, v) in pairs {
            p.insert((*k).to_string(), (*v).to_string());
        }
        l.configure(&p);
        l
    }

    #[test]
    fn an_unconfigured_lint_imposes_nothing() {
        let l = ForgeBody::default();
        assert!(check(&l, MessageDomain::PullRequestBody, "").is_empty());
        assert!(check(&l, MessageDomain::PullRequestBody, "anything at all").is_empty());
    }

    #[test]
    fn required_sections_are_reported_individually() {
        let l = with(&[("required_sections", "## Summary,## Test plan")]);
        assert_eq!(
            check(&l, MessageDomain::PullRequestBody, "## Summary\nx"),
            vec!["missing-section"]
        );
        assert!(
            check(
                &l,
                MessageDomain::PullRequestBody,
                "## Summary\nx\n\n## Test plan\ny"
            )
            .is_empty()
        );
    }

    #[test]
    fn section_matching_ignores_case() {
        let l = with(&[("required_sections", "## Summary")]);
        assert!(check(&l, MessageDomain::PullRequestBody, "## SUMMARY\nx").is_empty());
    }

    #[test]
    fn the_minimum_length_counts_characters_of_trimmed_text() {
        let l = with(&[("min_length", "20")]);
        assert_eq!(
            check(&l, MessageDomain::PullRequestBody, "   short   "),
            vec!["too-short"]
        );
        assert!(
            check(
                &l,
                MessageDomain::PullRequestBody,
                "a body long enough to pass the check"
            )
            .is_empty()
        );
    }

    #[test]
    fn a_forbidden_pattern_can_carry_its_reason() {
        // The reason is the point: a bare "not permitted" leaves the author
        // guessing why, and they will work around it rather than fix it.
        let l = with(&[(
            "forbidden",
            "staging.internal=internal hosts do not belong in a public record",
        )]);
        let ctx = MessageContext {
            domain:     MessageDomain::PullRequestBody,
            mode:       AgentMode::Assistant,
            message:    "see https://staging.internal/x",
            origin:     "pr-body",
            repo_root:  std::path::Path::new("/tmp"),
            invocation: None,
        };
        let errs = l.check_message(&ctx);
        assert_eq!(errs.len(), 1);
        assert!(
            errs[0].message.contains("internal hosts do not belong"),
            "the configured reason should be shown: {}",
            errs[0].message
        );
    }

    #[test]
    fn a_forbidden_pattern_without_a_reason_still_works() {
        let l = with(&[("forbidden", "wip")]);
        assert_eq!(
            check(&l, MessageDomain::PullRequestBody, "WIP do not merge"),
            vec!["forbidden-pattern"]
        );
    }

    #[test]
    fn several_forbidden_patterns_each_report() {
        let l = with(&[("forbidden", "wip,do not merge")]);
        assert_eq!(
            check(&l, MessageDomain::PullRequestBody, "wip: do not merge"),
            vec!["forbidden-pattern", "forbidden-pattern"]
        );
    }

    #[test]
    fn the_shape_rules_do_not_apply_to_a_comment() {
        // A review comment is not expected to carry a Summary section, so the
        // domain restriction is load-bearing rather than decoration.
        let l = with(&[("required_sections", "## Summary")]);
        assert_eq!(l.domains(), &[MessageDomain::PullRequestBody]);
        // and the runner is what enforces it, so check the declaration itself
        assert!(!l.domains().contains(&MessageDomain::ReviewComment));
    }

    #[test]
    fn every_finding_kind_the_lint_emits_is_declared() {
        let l = with(&[
            ("required_sections", "## Summary"),
            ("min_length", "100"),
            ("forbidden", "wip"),
        ]);
        let declared = l.finding_kinds();
        for kind in check(&l, MessageDomain::PullRequestBody, "wip") {
            assert!(
                declared.contains(&kind.as_str()),
                "`{kind}` is not declared"
            );
        }
    }
}
