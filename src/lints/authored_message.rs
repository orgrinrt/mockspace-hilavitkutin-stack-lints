//! What a shell command actually authors, pulled out of the command.
//!
//! A message lint running from an agent hook is not handed a message. The hook
//! cannot parse a shell command in shell, so it hands over the serialised tool
//! input whole and lets the lints find what they need in it. That is right for
//! a lint hunting a trailer anywhere in the text, and wrong for one that reads
//! the first line as a subject: the first line of a serialised tool call is
//! JSON, so every subject check fires on every commit and none of the findings
//! is about the commit.
//!
//! So a lint that needs the subject asks here instead, with the command the
//! invocation carries. What comes back is the text the command authors, or
//! nothing at all where the command authors it somewhere this cannot see: an
//! editor session, a file passed with `-F`, a heredoc consumed by git rather
//! than by the shell. Nothing is the honest answer there, and it costs little,
//! because the message written that way still reaches the `commit-msg` hook,
//! which is handed the real file and is the gate that actually stops the write.

/// The message a `git` command authors on its command line, if it does.
///
/// `None` covers three different situations and deliberately does not
/// distinguish them: the command authors no message, it authors one somewhere
/// unreadable from here, or it is not a git command at all.
pub(crate) fn authored_on_the_command_line(command: &str) -> Option<String> {
    let words = split_words(command);
    let mut parts: Vec<String> = Vec::new();
    for segment in git_message_segments(&words) {
        collect_messages(segment, &mut parts);
    }
    if parts.is_empty() {
        return None;
    }
    // Several `-m` arguments are separate paragraphs, which is how git joins
    // them, so the subject stays the first line either way.
    Some(parts.join("\n\n"))
}

/// The body a forge command carries on its command line, if it does.
///
/// The same problem as a commit subject one tool over. A forge body reaches a
/// lint as the serialised tool input, so measuring its length measures the
/// command line, and a body lint with a configured minimum would refuse a
/// perfectly ordinary pull request for a length it never had.
///
/// Which flag carries it depends on the program and the subcommand, and
/// [`FORGES`] is the table of what is read. A row is a shape somebody checked
/// against that tool's own help output; anything absent from it reads as no
/// body rather than as an empty one.
///
/// `Some("")` is a real answer and a different one from `None`: `--body ''` is a
/// body that is deliberately empty, which some conventions require, and a lint
/// that cannot tell it from no body at all cannot check that convention.
pub(crate) fn body_on_the_command_line(command: &str) -> Option<String> {
    let words = split_words(command);
    for (segment, forge) in forge_body_segments(&words) {
        let long_eq = format!("{}=", forge.long);
        let mut i = 0usize;
        while i < segment.len() {
            let w = &segment[i];
            if w == forge.file {
                return None;
            }
            if let Some(rest) = w.strip_prefix(long_eq.as_str()) {
                return unexpanded(rest);
            }
            if w == forge.long || w == forge.short {
                return segment.get(i + 1).and_then(|v| unexpanded(v));
            }
            i += 1;
        }
    }
    None
}

/// A value the shell would have replaced before the tool saw it.
///
/// `--body "$(git log ...)"` reaches a hook as those ten characters, so
/// measuring them measures the substitution rather than the body, and a
/// configured minimum then refuses a release pull request for a length it never
/// had. That command is the workspace's own, written down in its branch rules,
/// so this is not a hypothetical. Declining costs a missed check on one command
/// shape and is the only direction that does not invent a refusal.
fn unexpanded(value: &str) -> Option<String> {
    if value.contains("$(") || value.contains("${") || value.contains('`') {
        return None;
    }
    Some(value.to_string())
}

/// One forge invocation shape, and the flags that tool actually accepts.
///
/// A table rather than one `--body` for everything, because the three programs
/// disagree and two of them do not have a `--body` at all: `gh release create`
/// takes `-n, --notes` and `glab` takes `-d, --description`, both of which
/// answer `unknown flag` to `--body`. A single flag name reads as coverage of
/// all three and is coverage of one.
struct Forge {
    /// The program, as it appears at the head of a segment.
    tool:     &'static str,
    /// The subcommands under it that author a body.
    subjects: &'static [&'static str],
    /// The long flag carrying the body.
    long:     &'static str,
    /// The short flag carrying the body.
    short:    &'static str,
    /// The flag naming a file, which this cannot open, so it declines rather
    /// than measuring the path.
    file:     &'static str,
}

/// What is covered, which is what these rows say and nothing wider.
///
/// Read off `gh pr create --help`, `gh issue create --help`, `gh release create
/// --help`, `glab mr create --help` and `glab issue create --help`. A forge or
/// a subcommand absent from this table is not checked, and the arms below name
/// each row so removing one fails.
const FORGES: &[Forge] = &[
    Forge {
        tool:     "gh",
        subjects: &["pr", "issue"],
        long:     "--body",
        short:    "-b",
        file:     "--body-file",
    },
    Forge {
        tool:     "gh",
        subjects: &["release"],
        long:     "--notes",
        short:    "-n",
        file:     "--notes-file",
    },
    Forge {
        tool:     "glab",
        subjects: &["mr", "issue"],
        long:     "--description",
        short:    "-d",
        file:     "--description-file",
    },
];

/// Every run of words that is one forge invocation carrying a body, with the
/// row saying which flags that invocation accepts.
///
/// Anchored the same way the message scan is, and for the same reason: an
/// unrelated `--body` earlier in a command line is not the pull request's.
fn forge_body_segments(words: &[String]) -> Vec<(&[String], &'static Forge)> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for i in 0 ..= words.len() {
        let is_end = i == words.len() || matches!(words[i].as_str(), "&&" | "||" | ";" | "|" | "&");
        if !is_end {
            continue;
        }
        let segment = &words[start .. i];
        start = i + 1;
        let Some(first) = segment.first() else {
            continue;
        };
        for forge in FORGES {
            let is_tool = first == forge.tool || first.ends_with(&format!("/{}", forge.tool));
            if is_tool && segment.iter().any(|w| forge.subjects.contains(&w.as_str())) {
                out.push((segment, forge));
                break;
            }
        }
    }
    out
}

/// The verbs that author a message a person wrote.
const MESSAGE_VERBS: &[&str] = &["commit", "tag", "notes", "merge", "revert", "cherry-pick", "am"];

/// The short flags `git commit` accepts with no argument of their own, so they
/// can sit in front of the `m` in a cluster.
///
/// Named rather than taken as "any letter", and the list is the no-argument
/// ones only. `-t`, `-c`, `-C` and `-F` each take a value, so `-Fm file` is
/// `-F` with a value of `m` rather than a message flag, and reading it as one
/// would check a path as a subject. `-S` is left out for the same reason: its
/// argument is optional, which makes `-Sm` ambiguous, and declining is the safe
/// direction because it costs a missed check rather than a false refusal.
const CLUSTERED_BEFORE_M: &str = "asnevqoipu";

/// Every run of words that is one `git` invocation authoring a message.
///
/// The anchoring is the point. Without it any `-m` anywhere in the command line
/// is read as the subject, so `install -m 0755 f /usr/bin && git commit -m
/// 'fix: a subject'` checks `0755` and denies a well-formed commit, which is
/// the class this whole module exists to remove rather than to add to.
fn git_message_segments(words: &[String]) -> Vec<&[String]> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for i in 0 ..= words.len() {
        let is_end = i == words.len() || matches!(words[i].as_str(), "&&" | "||" | ";" | "|" | "&");
        if !is_end {
            continue;
        }
        let segment = &words[start .. i];
        start = i + 1;
        let Some(first) = segment.first() else {
            continue;
        };
        // `git`, or a path ending in it, which is how a wrapper or an absolute
        // invocation spells the same thing.
        let is_git = first == "git" || first.ends_with("/git");
        if is_git && segment.iter().any(|w| MESSAGE_VERBS.contains(&w.as_str())) {
            out.push(segment);
        }
    }
    out
}

/// Pull every message argument out of one git invocation.
fn collect_messages(segment: &[String], into: &mut Vec<String>) {
    let mut i = 0usize;
    while i < segment.len() {
        let w = &segment[i];
        if let Some(rest) = w.strip_prefix("--message=") {
            into.push(rest.to_string());
            i += 1;
            continue;
        }
        if w == "--message" {
            if let Some(v) = segment.get(i + 1) {
                into.push(v.clone());
                i += 2;
                continue;
            }
        }
        if let Some(short) = w.strip_prefix('-').filter(|s| !s.starts_with('-')) {
            if let Some(at) = short.find('m') {
                let cluster_is_flags = short[.. at].chars().all(|c| CLUSTERED_BEFORE_M.contains(c));
                let attached = &short[at + 1 ..];
                if cluster_is_flags && !attached.is_empty() {
                    // `-mSubject`, and `-amSubject`. git takes the rest of the
                    // word as the message.
                    into.push(attached.to_string());
                    i += 1;
                    continue;
                }
                if cluster_is_flags && attached.is_empty() {
                    if let Some(v) = segment.get(i + 1) {
                        into.push(v.clone());
                        i += 2;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
}

/// Split a command the way a shell would, as far as quoting goes.
///
/// Not a shell. It resolves single quotes, double quotes and backslash escapes,
/// and joins adjacent runs into one word, which is what makes `'it'\''s'` come
/// back as `it's` rather than as three words. Everything else a shell does,
/// expansion, substitution, redirection, is left exactly as written, because a
/// lint has no business running any of it and a literal `$VAR` in a subject is
/// a subject the author has to fix anyway.
fn split_words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut started = false;
    let mut chars = command.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' | '\n' | '\r' => {
                if started {
                    words.push(std::mem::take(&mut cur));
                    started = false;
                }
            },
            '\'' => {
                started = true;
                for q in chars.by_ref() {
                    if q == '\'' {
                        break;
                    }
                    cur.push(q);
                }
            },
            '"' => {
                started = true;
                while let Some(q) = chars.next() {
                    match q {
                        '"' => break,
                        // Inside double quotes a backslash escapes only these
                        // four; before anything else it is a literal backslash,
                        // which is why the fallback pushes both characters.
                        '\\' => {
                            match chars.peek() {
                                Some('"') | Some('\\') | Some('$') | Some('`') => {
                                    cur.push(chars.next().unwrap_or('\\'));
                                },
                                _ => cur.push('\\'),
                            }
                        },
                        _ => cur.push(q),
                    }
                }
            },
            '\\' => {
                started = true;
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            },
            _ => {
                started = true;
                cur.push(c);
            },
        }
    }
    if started {
        words.push(cur);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_quoted_message_comes_back_whole() {
        assert_eq!(
            authored_on_the_command_line("git commit -m 'fix: a subject'").as_deref(),
            Some("fix: a subject")
        );
    }

    #[test]
    fn a_double_quoted_message_comes_back_whole() {
        assert_eq!(
            authored_on_the_command_line("git commit -m \"fix: a subject\"").as_deref(),
            Some("fix: a subject")
        );
    }

    #[test]
    fn an_explicit_repo_in_front_changes_nothing() {
        assert_eq!(
            authored_on_the_command_line("git -C /some/where commit -m 'fix: a subject'")
                .as_deref(),
            Some("fix: a subject")
        );
    }

    #[test]
    fn a_clustered_short_flag_still_names_the_message() {
        assert_eq!(
            authored_on_the_command_line("git commit -am 'fix: a subject'").as_deref(),
            Some("fix: a subject")
        );
    }

    #[test]
    fn a_long_flag_with_an_equals_names_it_too() {
        assert_eq!(
            authored_on_the_command_line("git commit --message='fix: a subject'").as_deref(),
            Some("fix: a subject")
        );
    }

    #[test]
    fn two_messages_are_two_paragraphs_and_the_first_is_the_subject() {
        let got = authored_on_the_command_line("git commit -m 'fix: a subject' -m 'the body'")
            .expect("both messages");
        assert_eq!(got.lines().next(), Some("fix: a subject"));
        assert!(got.contains("the body"), "the body survived: {got}");
    }

    #[test]
    fn an_escaped_quote_inside_a_subject_survives() {
        // `'it'\''s'` is three adjacent runs a shell joins into one word, and a
        // splitter that emitted three words would hand the lint `it` and call
        // the subject two characters long.
        assert_eq!(
            authored_on_the_command_line(r"git commit -m 'it'\''s fixed'").as_deref(),
            Some("it's fixed")
        );
    }

    #[test]
    fn a_command_carrying_no_message_reads_as_none() {
        assert_eq!(authored_on_the_command_line("git commit"), None);
        assert_eq!(authored_on_the_command_line("git commit --amend"), None);
    }

    #[test]
    fn a_message_in_a_file_reads_as_none_rather_than_as_the_flag() {
        // `-F` names a path this cannot open, and reading the path itself as
        // the subject would be worse than declining.
        assert_eq!(
            authored_on_the_command_line("git commit -F /tmp/msg.txt"),
            None
        );
    }

    #[test]
    fn a_short_flag_that_merely_ends_in_m_is_not_a_message_flag() {
        // The control for the cluster rule, and it needs a word after the flag
        // to mean anything: with the flag last, every reading of the rule
        // returns `None` and the arm certifies nothing. `-Xm` is git's own
        // merge-strategy spelling, so this is not a hypothetical.
        assert_eq!(
            authored_on_the_command_line("git merge -Xm theirs other-branch"),
            None
        );
        assert_eq!(
            authored_on_the_command_line("some-tool -Zm something-else"),
            None
        );
    }

    #[test]
    fn every_no_argument_short_flag_git_commit_takes_can_precede_the_m() {
        // The positive half of the arm above, and it is written from git's own
        // documented no-argument flags rather than from the constant, so a set
        // that had drifted narrow fails here. The first version enumerated the
        // same letters the constant declares, which could not detect that at
        // all, and `-om`, `-im` and `-pm` were in fact missing.
        for flag in ["a", "s", "n", "e", "v", "q", "o", "i", "p", "u"] {
            let cluster = format!("-{flag}m");
            assert_eq!(
                authored_on_the_command_line(&format!("git commit {cluster} 'fix: a subject'"))
                    .as_deref(),
                Some("fix: a subject"),
                "`{cluster}` did not name the message"
            );
        }
        for cluster in ["-am", "-anm", "-sam", "-m"] {
            assert_eq!(
                authored_on_the_command_line(&format!("git commit {cluster} 'fix: a subject'"))
                    .as_deref(),
                Some("fix: a subject"),
                "`{cluster}` did not name the message"
            );
        }
    }

    #[test]
    fn a_flag_that_takes_its_own_argument_does_not_cluster_into_a_message_flag() {
        // `-F`, `-t`, `-c` and `-C` each take a value, so `-Fm file` is `-F`
        // with a value of `m`, and reading it as a message flag would check a
        // path as a subject. Declining is the safe direction here: it costs a
        // missed check rather than a refusal of a well-formed commit.
        for cluster in ["-Fm", "-tm", "-cm", "-Cm", "-Sm"] {
            assert_eq!(
                authored_on_the_command_line(&format!("git commit {cluster} value")),
                None,
                "`{cluster}` was read as a message flag"
            );
        }
    }

    #[test]
    fn the_attached_form_is_the_message() {
        // `git commit -mSubject` with no space, which git accepts and which a
        // scan looking only at the next word does not see at all.
        assert_eq!(
            authored_on_the_command_line("git commit -mfix: a subject").as_deref(),
            Some("fix:")
        );
        assert_eq!(
            authored_on_the_command_line("git commit -m'fix: a subject'").as_deref(),
            Some("fix: a subject")
        );
        assert_eq!(
            authored_on_the_command_line("git commit -am'fix: a subject'").as_deref(),
            Some("fix: a subject")
        );
    }

    #[test]
    fn a_message_flag_belonging_to_another_program_is_not_the_subject() {
        // The anchoring, and the arm that matters most, because getting it
        // wrong denies a well-formed commit rather than merely missing one.
        // `install -m` is a file mode and `python3 -m` is a module name.
        for other in ["install -m 0755 f /usr/bin", "python3 -m venv .venv", "chmod -R 755 ."] {
            assert_eq!(
                authored_on_the_command_line(&format!("{other} && git commit -m 'fix: a subject'"))
                    .as_deref(),
                Some("fix: a subject"),
                "`{other}` leaked into the subject"
            );
        }
    }

    #[test]
    fn a_message_flag_with_no_git_beside_it_at_all_is_nobodys_subject() {
        // The control for the anchoring: without a git invocation there is no
        // message, however many `-m` flags the line carries.
        assert_eq!(
            authored_on_the_command_line("install -m 0755 f /usr/bin"),
            None
        );
        assert_eq!(authored_on_the_command_line("python3 -m venv .venv"), None);
    }

    #[test]
    fn a_git_invocation_with_no_message_verb_authors_nothing() {
        // `git config -m` is not a message, and neither is anything else git
        // does that is not one of the verbs that writes a message somebody
        // wrote. Without the verb test the segment check is only "is it git".
        assert_eq!(
            authored_on_the_command_line("git config --global -m x"),
            None
        );
        assert_eq!(
            authored_on_the_command_line("git log -m --oneline && git status"),
            None
        );
    }

    #[test]
    fn a_command_that_is_not_git_at_all_reads_as_none() {
        assert_eq!(authored_on_the_command_line("cargo test --lib"), None);
        assert_eq!(authored_on_the_command_line("echo hello"), None);
    }

    #[test]
    fn a_commit_after_something_else_is_still_found() {
        assert_eq!(
            authored_on_the_command_line("git add -A && git commit -m 'fix: a subject'").as_deref(),
            Some("fix: a subject")
        );
    }

    #[test]
    fn a_forge_body_comes_out_of_the_command() {
        assert_eq!(
            body_on_the_command_line("gh pr create --title 'x' --body 'the body'").as_deref(),
            Some("the body")
        );
        assert_eq!(
            body_on_the_command_line("gh pr create --body='the body'").as_deref(),
            Some("the body")
        );
    }

    #[test]
    fn the_short_flag_is_read_the_way_the_long_one_is() {
        // `gh pr create --help` documents `-b, --body`, and an agent writing a
        // command by hand reaches for the short form as often as the long one.
        // Reading only the long form leaves every check in the lint returning
        // nothing on a command it was built to read, which is the whole lint
        // bypassed by a flag the tool advertises.
        assert_eq!(
            body_on_the_command_line("gh pr create -b 'the body'").as_deref(),
            Some("the body")
        );
        assert_eq!(
            body_on_the_command_line("gh issue create -b 'the body'").as_deref(),
            Some("the body")
        );
    }

    #[test]
    fn each_forge_is_read_by_the_flag_that_forge_accepts() {
        // One row per entry in `FORGES`, so removing a row fails here. The
        // three programs disagree and two of them have no `--body` at all:
        // `gh release create` answers `unknown flag` to it and takes
        // `-n, --notes`, and `glab` takes `-d, --description`. A single flag
        // name across all three reads as coverage of three and is coverage of
        // one.
        assert_eq!(
            body_on_the_command_line("gh release create v1 --notes 'the notes'").as_deref(),
            Some("the notes")
        );
        assert_eq!(
            body_on_the_command_line("gh release create v1 -n 'the notes'").as_deref(),
            Some("the notes")
        );
        assert_eq!(
            body_on_the_command_line("glab mr create --description 'the body'").as_deref(),
            Some("the body")
        );
        assert_eq!(
            body_on_the_command_line("glab issue create -d 'the body'").as_deref(),
            Some("the body")
        );
    }

    #[test]
    fn a_flag_the_tool_does_not_accept_is_not_read_as_a_body() {
        // The negative half of the row above, and the one the table exists
        // for. Both of these are what the earlier `--body`-everywhere reader
        // returned a body for, and neither command runs: the tools answer
        // `unknown flag`. Answering `Some` here means the lint judges a string
        // that was never published.
        assert_eq!(
            body_on_the_command_line("glab mr create --body 'not a glab flag'"),
            None
        );
        assert_eq!(
            body_on_the_command_line("gh release create v1 --body 'not a release flag'"),
            None
        );
    }

    #[test]
    fn a_substitution_the_shell_never_expanded_is_not_measured() {
        // `--body "$(git log ...)"` reaches a hook as its own text, so
        // measuring it measures ten characters of shell rather than the body,
        // and a configured minimum then refuses a release pull request for a
        // length it never had. That command is the workspace's own.
        assert_eq!(
            body_on_the_command_line(
                "gh pr create --base main --body \"$(git log --format='- %h %s')\""
            ),
            None
        );
        assert_eq!(
            body_on_the_command_line("gh pr create --body '`date`'"),
            None
        );
        assert_eq!(
            body_on_the_command_line("gh pr create --body '${SUMMARY}'"),
            None
        );
    }

    #[test]
    fn an_empty_body_is_a_body_and_not_the_absence_of_one() {
        // `--body ''` is a body somebody chose to leave empty, which is what
        // the convention here requires on a feature pull request, and a lint
        // that cannot tell it from no body at all cannot check that.
        assert_eq!(
            body_on_the_command_line("gh pr create --body ''").as_deref(),
            Some("")
        );
        assert_eq!(body_on_the_command_line("gh pr create --title 'x'"), None);
    }

    #[test]
    fn a_body_flag_belonging_to_something_else_is_not_the_pull_requests() {
        // The anchoring, same as for a subject. `curl --body` is not a forge
        // command, and reading its argument as a pull request body would judge
        // a request payload against a pull request convention.
        assert_eq!(
            body_on_the_command_line("curl --body 'payload' https://example.test"),
            None
        );
        assert_eq!(
            body_on_the_command_line("curl --body 'payload' && gh pr create --body 'real'")
                .as_deref(),
            Some("real")
        );
    }

    #[test]
    fn a_body_in_a_file_reads_as_none_rather_than_as_the_path() {
        // Measuring the length of a path is the shape that reads as a real
        // check and is not one.
        assert_eq!(
            body_on_the_command_line("gh pr create --body-file /tmp/b.md"),
            None
        );
        assert_eq!(
            body_on_the_command_line("gh release create v1 --notes-file /tmp/n.md"),
            None
        );
        assert_eq!(
            body_on_the_command_line("glab mr create --description-file /tmp/d.md"),
            None
        );
        // Both flags at once is the only shape where naming the file flag
        // changes an answer, since the file forms share no spelling with the
        // body forms and fall through to nothing on their own. `gh` refuses
        // such a command outright, so nothing is published and there is
        // nothing to judge; reading the inline one would judge a string the
        // tool never accepted.
        assert_eq!(
            body_on_the_command_line("gh pr create --body-file /tmp/b.md --body 'inline'"),
            None
        );
    }

    #[test]
    fn a_forge_program_with_no_body_carrying_subcommand_authors_nothing() {
        assert_eq!(body_on_the_command_line("gh repo view --json name"), None);
        assert_eq!(body_on_the_command_line("gh pr list"), None);
    }
}
