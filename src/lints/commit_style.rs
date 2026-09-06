//! Lint: a commit message matches the project's declared convention.
//!
//! Every knob is configured, and nothing is a default, because a commit
//! convention is exactly the kind of thing no default fits: it is neither
//! universal nor obviously shared between projects. A project declares its style,
//! or declares none and this lint does nothing.
//!
//! Two presets ship: `commit-conventional` for Conventional Commits as written,
//! and `commit-hiisi` for the stricter variant (no parenthesised scope, lowercase
//! subject, no trailing period, under 72). Either is a starting point that every
//! individual knob then overrides.
//!
//! Shape only. Authorship trailers are [`super::message_attribution`]'s job,
//! because they apply to pull-request bodies too and this lint does not.

use std::collections::HashMap;

use mockspace_lint_rules::{Lint, LintError, MessageContext, MessageDomain, MessageLint, Severity};

const LINT_NAME: &str = "commit-style";

/// How the subject's first letter is constrained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SubjectCase {
    /// Must begin lowercase. The imperative-mood convention's usual companion.
    Lower,
    /// Must begin uppercase.
    Sentence,
    /// Unconstrained.
    #[default]
    Any,
}

impl SubjectCase {
    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "lower" | "lowercase" => Some(Self::Lower),
            "sentence" | "upper" | "capital" => Some(Self::Sentence),
            "any" | "off" | "" => Some(Self::Any),
            _ => None,
        }
    }
}

/// The configured convention. Every field starts permissive, so an unconfigured
/// lint reports nothing rather than imposing a convention nobody asked for.
pub struct CommitStyle {
    /// Permitted type prefixes. Empty disables type checking entirely.
    types:                  Vec<String>,
    /// What separates the type from the subject.
    separator:              String,
    /// Suffix marking a breaking change, appended to the type.
    breaking_marker:        String,
    /// Whether a type prefix is required at all.
    require_type:           bool,
    /// Maximum subject length. Zero disables the check.
    max_subject:            usize,
    /// How the subject's first letter is constrained.
    subject_case:           SubjectCase,
    /// Whether `type(scope):` is permitted.
    allow_scope:            bool,
    /// Whether a trailing period on the subject is a violation.
    forbid_trailing_period: bool,
    /// Whether the subject must be blank-line separated from the body.
    require_blank_line:     bool,
}

impl Default for CommitStyle {
    fn default() -> Self {
        Self {
            types:                  Vec::new(),
            separator:              ": ".to_string(),
            breaking_marker:        "!".to_string(),
            require_type:           false,
            max_subject:            0,
            subject_case:           SubjectCase::Any,
            allow_scope:            true,
            forbid_trailing_period: false,
            require_blank_line:     false,
        }
    }
}

impl Lint for CommitStyle {
    fn name(&self) -> &'static str {
        LINT_NAME
    }

    /// The subject is in the command when an agent hook is the caller, so this
    /// lint cannot do its job without the invocation. `subject_source` says
    /// what it does with it.
    fn invocation_wanted(&self) -> bool {
        true
    }

    fn description(&self) -> &'static str {
        "a commit subject matches the project's declared convention"
    }

    fn source_only(&self) -> bool {
        false
    }

    fn default_severity(&self) -> Severity {
        Severity::HARD_ERROR
    }

    fn finding_kinds(&self) -> &[&str] {
        &[
            "missing-type",
            "unknown-type",
            "scope-forbidden",
            "subject-case",
            "subject-length",
            "trailing-period",
            "missing-blank-line",
            "empty-subject",
        ]
    }

    fn config_keys(&self) -> &[&str] {
        &[
            "types",
            "separator",
            "breaking_marker",
            "require_type",
            "max_subject",
            "subject_case",
            "allow_scope",
            "forbid_trailing_period",
            "require_blank_line",
        ]
    }

    fn configure(&mut self, params: &HashMap<String, String>) {
        if let Some(v) = params.get("types") {
            self.types = v
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        if let Some(v) = params.get("separator") {
            self.separator = v.clone();
        }
        if let Some(v) = params.get("breaking_marker") {
            self.breaking_marker = v.clone();
        }
        if let Some(v) = params.get("require_type") {
            self.require_type = truthy(v);
        }
        if let Some(v) = params.get("max_subject") {
            if let Ok(n) = v.trim().parse::<usize>() {
                self.max_subject = n;
            }
        }
        if let Some(v) = params.get("subject_case") {
            if let Some(c) = SubjectCase::parse(v) {
                self.subject_case = c;
            }
        }
        if let Some(v) = params.get("allow_scope") {
            self.allow_scope = truthy(v);
        }
        if let Some(v) = params.get("forbid_trailing_period") {
            self.forbid_trailing_period = truthy(v);
        }
        if let Some(v) = params.get("require_blank_line") {
            self.require_blank_line = truthy(v);
        }
    }
}

impl MessageLint for CommitStyle {
    fn domains(&self) -> &[MessageDomain] {
        &[MessageDomain::CommitMessage]
    }

    fn check_message(&self, ctx: &MessageContext) -> Vec<LintError> {
        let mut out = Vec::new();
        let Some(text) = subject_source(ctx) else {
            return out;
        };
        let body = authored_body(&text);
        let Some(subject) = body.lines().next() else {
            return out;
        };
        let subject = subject.trim_end();

        // A fixup or merge subject is generated by git rather than authored, so
        // holding it to an authored convention would fail commits the user never
        // wrote the subject for.
        if is_generated_subject(subject) {
            return out;
        }

        if subject.trim().is_empty() {
            out.push(finding(ctx, "empty-subject", "the commit subject is empty"));
            return out;
        }

        let rest = self.check_type(ctx, subject, &mut out);

        if self.max_subject > 0 && subject.chars().count() > self.max_subject {
            out.push(finding(
                ctx,
                "subject-length",
                &format!(
                    "the subject is {} characters, over the configured {}",
                    subject.chars().count(),
                    self.max_subject
                ),
            ));
        }

        if let Some(first) = rest.trim_start().chars().next() {
            match self.subject_case {
                SubjectCase::Lower if first.is_uppercase() => {
                    out.push(finding(
                        ctx,
                        "subject-case",
                        "the subject should begin lowercase",
                    ));
                },
                SubjectCase::Sentence if first.is_lowercase() => {
                    out.push(finding(
                        ctx,
                        "subject-case",
                        "the subject should begin uppercase",
                    ));
                },
                _ => {},
            }
        }

        if self.forbid_trailing_period && subject.ends_with('.') && !subject.ends_with("..") {
            out.push(finding(
                ctx,
                "trailing-period",
                "the subject should not end with a period",
            ));
        }

        if self.require_blank_line {
            let mut lines = body.lines();
            let _ = lines.next();
            if let Some(second) = lines.next() {
                if !second.trim().is_empty() {
                    out.push(finding(
                        ctx,
                        "missing-blank-line",
                        "the subject and body should be separated by a blank line",
                    ));
                }
            }
        }

        out
    }
}

impl CommitStyle {
    /// Check the type prefix, returning the subject text after it.
    fn check_type<'m>(
        &self,
        ctx: &MessageContext,
        subject: &'m str,
        out: &mut Vec<LintError>,
    ) -> &'m str {
        if self.types.is_empty() && !self.require_type {
            return subject;
        }
        let Some(sep_at) = subject.find(&self.separator) else {
            if self.require_type {
                out.push(finding(
                    ctx,
                    "missing-type",
                    &format!(
                        "the subject should begin with a type and `{}`, one of: {}",
                        self.separator,
                        self.types.join(", ")
                    ),
                ));
            }
            return subject;
        };
        let (head, rest) = subject.split_at(sep_at);
        let rest = &rest[self.separator.len() ..];

        // Strip the breaking marker before matching, so `feat!` is the `feat`
        // type rather than an unknown one.
        let mut ty = head;
        if !self.breaking_marker.is_empty() {
            ty = ty.strip_suffix(&self.breaking_marker).unwrap_or(ty);
        }

        // A parenthesised scope, as Conventional Commits permits.
        let scoped = ty
            .split_once('(')
            .map(|(base, tail)| (base, tail.ends_with(')')));
        let base = match scoped {
            Some((base, closed)) => {
                if !self.allow_scope {
                    out.push(finding(
                        ctx,
                        "scope-forbidden",
                        "a parenthesised scope is not permitted; drop the parentheses",
                    ));
                } else if !closed {
                    out.push(finding(
                        ctx,
                        "unknown-type",
                        "the scope's opening parenthesis is unclosed",
                    ));
                }
                base
            },
            None => ty,
        };

        if !self.types.is_empty() && !self.types.iter().any(|t| t == base) {
            out.push(finding(
                ctx,
                "unknown-type",
                &format!(
                    "`{base}` is not one of the configured types: {}",
                    self.types.join(", ")
                ),
            ));
        }
        rest
    }
}

/// The authored part of a commit message.
///
/// Drops comment lines and everything from git's verbose-diff scissors marker,
/// so a convention violation quoted in the commented help text, or anything in an
/// attached diff, cannot trip the lint.
/// The text whose first line is the subject, which is not always `ctx.message`.
///
/// Two callers hand this lint two different things. The `commit-msg` git hook
/// passes the message file, and that is the message. An agent's PreToolUse hook
/// passes the serialised tool input, because it cannot parse a shell command in
/// shell, and the first line of that is JSON. Reading the second as a subject
/// reports a type of `{"command":"git commit -m 'fix` and a length that is the
/// length of the command line, on every commit, however well written.
///
/// The invocation is what tells them apart: it is present only on the hook
/// path. There, the subject comes out of the command or it does not come at
/// all, and declining is right rather than cautious, since a message authored
/// where the command line cannot show it still reaches the `commit-msg` hook
/// with the real file in hand.
fn subject_source(ctx: &MessageContext) -> Option<String> {
    let Some(command) = ctx.invocation.as_ref().and_then(|i| i.command) else {
        return Some(ctx.message.to_string());
    };
    super::authored_message::authored_on_the_command_line(command)
}

fn authored_body(message: &str) -> String {
    message
        .split("# ------------------------ >8")
        .next()
        .unwrap_or(message)
        .lines()
        .filter(|l| !l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether git generated this subject rather than a person authoring it.
fn is_generated_subject(subject: &str) -> bool {
    const GENERATED: &[&str] = &["merge ", "revert ", "fixup! ", "squash! ", "amend! ", "rebase "];
    let lower = subject.to_ascii_lowercase();
    GENERATED.iter().any(|p| lower.starts_with(p))
}

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "yes" | "on" | "1"
    )
}

fn finding(ctx: &MessageContext, kind: &'static str, message: &str) -> LintError {
    // Carrying the finding kind is what lets a project set a per-kind severity,
    // so a convention can block on an unknown type while only warning on length.
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
    use super::*;

    fn hiisi() -> CommitStyle {
        let mut l = CommitStyle::default();
        let mut p = HashMap::new();
        p.insert("types".into(), "feat,fix,refactor,docs,test,chore".into());
        p.insert("require_type".into(), "true".into());
        p.insert("max_subject".into(), "72".into());
        p.insert("subject_case".into(), "lower".into());
        p.insert("allow_scope".into(), "false".into());
        p.insert("forbid_trailing_period".into(), "true".into());
        p.insert("require_blank_line".into(), "true".into());
        l.configure(&p);
        l
    }

    fn conventional() -> CommitStyle {
        let mut l = CommitStyle::default();
        let mut p = HashMap::new();
        p.insert(
            "types".into(),
            "feat,fix,docs,style,refactor,test,chore".into(),
        );
        p.insert("require_type".into(), "true".into());
        p.insert("allow_scope".into(), "true".into());
        l.configure(&p);
        l
    }

    fn check(l: &CommitStyle, msg: &str) -> Vec<String> {
        let ctx = MessageContext {
            domain:     MessageDomain::CommitMessage,
            mode:       mockspace_lint_rules::AgentMode::Assistant,
            message:    msg,
            origin:     "COMMIT_EDITMSG",
            repo_root:  std::path::Path::new("/tmp"),
            invocation: None,
        };
        l.check_message(&ctx)
            .into_iter()
            .map(|e| e.finding_kind.unwrap_or("none").to_string())
            .collect()
    }

    /// What an agent's PreToolUse hook actually hands over: the serialised tool
    /// input as the text, and the command beside it as the invocation.
    fn check_from_a_hook(l: &CommitStyle, command: &str) -> Vec<String> {
        let serialised = format!(
            "{{\"command\":\"{}\",\"description\":\"commit the change\"}}",
            command.replace('"', "\\\"")
        );
        let ctx = MessageContext {
            domain:     MessageDomain::CommitMessage,
            mode:       mockspace_lint_rules::AgentMode::Assistant,
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
    fn a_good_subject_through_a_hook_is_not_a_finding() {
        // The defect this pair exists for. The text handed over is JSON, whose
        // first line begins `{"command":"git commit -m 'fix`, so reading it as
        // the subject reported a bad type and a length that was the length of
        // the command. Every commit from an agent's shell was refused, and the
        // message it was refused for was never the message.
        let l = hiisi();
        assert_eq!(
            check_from_a_hook(&l, "git commit -m 'fix: a subject well under the limit'"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_bad_subject_through_a_hook_is_still_caught() {
        // The half that keeps the arm above from being a hole: if reading the
        // command meant reading nothing, every commit would pass instead of
        // every commit failing, which is worse and quieter.
        let l = hiisi();
        let found = check_from_a_hook(
            &l,
            "git commit -m 'Added Some Things And This Subject Runs Well Past The Seventy Two Character Limit.'",
        );
        assert!(
            found.contains(&"subject-length".to_string()),
            "the length went unreported: {found:?}"
        );
        assert!(
            found.contains(&"subject-case".to_string()),
            "the case went unreported: {found:?}"
        );
    }

    #[test]
    fn a_hook_call_that_authors_no_message_reports_nothing() {
        // An editor commit, or one whose message is in a file. The subject is
        // somewhere this cannot read, and the `commit-msg` hook is handed the
        // real file moments later, so declining costs no coverage and guessing
        // would refuse a message nobody has seen yet.
        let l = hiisi();
        assert_eq!(check_from_a_hook(&l, "git commit"), Vec::<String>::new());
        assert_eq!(
            check_from_a_hook(&l, "git commit -F /tmp/msg.txt"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn the_commit_msg_hook_path_is_untouched() {
        // The control for the three above: with no invocation the text is the
        // message, which is how the git hook calls it, and every existing arm
        // in this file goes through that path.
        let l = hiisi();
        assert!(check(&l, "fix: a subject well under the limit").is_empty());
        assert!(!check(&l, "Added Some Thing.").is_empty());
    }

    #[test]
    fn an_unconfigured_lint_imposes_nothing() {
        // The load-bearing default: declaring no convention means no findings,
        // because no commit convention fits every project.
        let l = CommitStyle::default();
        assert!(check(&l, "Added Some Thing.").is_empty());
        assert!(check(&l, "whatever").is_empty());
    }

    #[test]
    fn the_hiisi_preset_accepts_its_own_form() {
        let l = hiisi();
        assert!(check(&l, "feat: add catalogue entry validation").is_empty());
        assert!(check(&l, "fix: handle missing loadout").is_empty());
        assert!(check(&l, "feat!: change the language trait signature").is_empty());
        assert!(check(&l, "docs: update the data store design\n\nbody here").is_empty());
    }

    #[test]
    fn the_hiisi_preset_rejects_each_thing_it_forbids() {
        let l = hiisi();
        assert_eq!(check(&l, "feat(data): add thing"), vec!["scope-forbidden"]);
        assert_eq!(check(&l, "feat: Add Data Store"), vec!["subject-case"]);
        assert_eq!(check(&l, "feat: add data store."), vec!["trailing-period"]);
        // both, and correctly so: it has no type prefix and it is capitalised.
        // With no type to strip, the case rule applies to the whole subject.
        assert_eq!(check(&l, "Added new feature"), vec![
            "missing-type",
            "subject-case"
        ]);
        assert_eq!(check(&l, "wip: something"), vec!["unknown-type"]);
        assert_eq!(check(&l, "feat: x\nbody with no blank line"), vec![
            "missing-blank-line"
        ]);
    }

    #[test]
    fn conventional_permits_the_scope_that_hiisi_forbids() {
        // The two presets must genuinely differ, or shipping both is pointless.
        let c = conventional();
        assert!(check(&c, "feat(parser): add lookahead").is_empty());
        assert_eq!(check(&hiisi(), "feat(parser): add lookahead"), vec![
            "scope-forbidden"
        ]);
    }

    #[test]
    fn conventional_leaves_case_and_period_alone() {
        // It constrains the type vocabulary and nothing else, so a project
        // adopting it is not silently also adopting the stricter rules.
        let c = conventional();
        assert!(check(&c, "feat: Add A Thing.").is_empty());
    }

    #[test]
    fn the_subject_length_limit_counts_characters_not_bytes() {
        let mut l = CommitStyle::default();
        let mut p = HashMap::new();
        p.insert("max_subject".into(), "10".into());
        l.configure(&p);
        // ten multi-byte characters is ten characters, not thirty bytes
        assert!(check(&l, "ääääääääää").is_empty());
        assert_eq!(check(&l, "ãããããããããää"), vec!["subject-length"]);
    }

    #[test]
    fn git_generated_subjects_are_left_alone() {
        // The user did not author these, so holding them to an authored
        // convention would block commits nobody wrote the subject for.
        let l = hiisi();
        for msg in [
            "Merge branch 'dev' into feat/x",
            "Revert \"feat: add thing\"",
            "fixup! feat: add thing",
            "squash! feat: add thing",
        ] {
            assert!(check(&l, msg).is_empty(), "{msg} should be exempt");
        }
    }

    #[test]
    fn commented_lines_and_the_diff_are_not_the_subject() {
        // git's template puts help text and, under --verbose, a whole diff into
        // the file. Neither is authored, so neither can be a violation.
        let l = hiisi();
        let msg = "feat: add thing\n\n# Please enter the commit message...\n# feat(bad): Example.\n\
                   # ------------------------ >8 ------------------------\ndiff --git a/x b/x\n+Added Thing.";
        assert!(check(&l, msg).is_empty());
    }

    #[test]
    fn an_empty_subject_is_reported_once() {
        let l = hiisi();
        assert_eq!(check(&l, ""), Vec::<String>::new());
        assert_eq!(check(&l, "   \n\nbody"), vec!["empty-subject"]);
    }

    #[test]
    fn a_breaking_marker_does_not_make_the_type_unknown() {
        let l = hiisi();
        assert!(check(&l, "refactor!: change the trait").is_empty());
    }

    #[test]
    fn an_unclosed_scope_parenthesis_is_reported() {
        let c = conventional();
        assert_eq!(check(&c, "feat(parser: add lookahead"), vec![
            "unknown-type"
        ]);
    }

    #[test]
    fn every_finding_kind_the_lint_emits_is_declared() {
        // Undeclared kinds cannot be given a per-finding severity in config, so
        // the emitted set and the declared set must agree.
        let l = hiisi();
        let declared = l.finding_kinds();
        let mut emitted: Vec<String> = Vec::new();
        for msg in [
            "feat(data): add thing",
            "feat: Add Thing",
            "feat: add thing.",
            "Added a thing",
            "wip: x",
            "feat: x\nbody",
            "   \n\nbody",
            &format!("feat: {}", "x".repeat(200)),
        ] {
            emitted.extend(check(&l, msg));
        }
        for kind in emitted {
            assert!(
                declared.contains(&kind.as_str()),
                "emitted `{kind}` is not in finding_kinds()"
            );
        }
    }
}
