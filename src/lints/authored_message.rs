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
    let mut i = 0usize;
    while i < words.len() {
        let w = &words[i];
        if let Some(rest) = w.strip_prefix("--message=") {
            parts.push(rest.to_string());
            i += 1;
            continue;
        }
        // `-m` on its own, and the clustered short forms `git commit` accepts
        // before it: `-am`, `-sm`, `-anm`. The letters allowed in front of the
        // `m` are named rather than taken as any letter, because "any letter
        // then m" swallows the argument after every unrelated flag that happens
        // to end in one, and the flag it swallows is then checked as a subject.
        const CLUSTERED_BEFORE_M: &str = "asnevqS";
        let takes_next = w == "--message"
            || (w.len() >= 2
                && w.starts_with('-')
                && !w.starts_with("--")
                && w.ends_with('m')
                && w[1 .. w.len() - 1]
                    .chars()
                    .all(|c| CLUSTERED_BEFORE_M.contains(c)));
        if takes_next {
            if let Some(v) = words.get(i + 1) {
                parts.push(v.clone());
                i += 2;
                continue;
            }
        }
        i += 1;
    }
    if parts.is_empty() {
        return None;
    }
    // Several `-m` arguments are separate paragraphs, which is how git joins
    // them, so the subject stays the first line either way.
    Some(parts.join("\n\n"))
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
    fn the_flags_git_actually_clusters_before_m_are_all_taken() {
        // The positive half of the arm above. Naming the letters is only right
        // if the named set is the real one, and a set that had drifted narrow
        // would show up here rather than as commits refused months later.
        for cluster in ["-am", "-sm", "-anm", "-sam", "-qm"] {
            assert_eq!(
                authored_on_the_command_line(&format!("git commit {cluster} 'fix: a subject'"))
                    .as_deref(),
                Some("fix: a subject"),
                "`{cluster}` did not name the message"
            );
        }
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
}
