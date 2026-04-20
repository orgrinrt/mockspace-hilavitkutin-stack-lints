//! Writing-style heuristics for public-facing docs.
//!
//! Source: `.shared/writing-style-fragment.md` in clause-dev. Threshold-based,
//! not per-occurrence strict: single uses pass, runaway usage blocks.
//!
//! Scope:
//! - `{mock}/PRINCIPLES.md.tmpl`, `{mock}/DESIGN.md.tmpl`, `{mock}/WORKFLOW.md.tmpl`
//! - `{repo_root}/README.md`
//! - `{mock}/crates/**/*.md.tmpl`
//! - `{mock}/agent/**/*.md.tmpl`
//! - Rust `///` / `//!` / `//` comments under `{mock}/crates/**/*.rs`
//!
//! Out of scope (discipline, not gated): `mock/design_rounds/**`.

use std::path::{Path, PathBuf};

use mockspace_lint_rules::{CrossCrateLint, LintContext, LintError, Severity};

const HYPE_WORDS: &[&str] = &[
    "blazing", "seamless", "powerful", "amazing", "incredible",
    "game-changing", "best-in-class",
];

const CORPORATE_JARGON: &[&str] = &[
    "leverage", "utilize", "utilise", "synergy", "holistic", "paradigm",
];

const FILLER_PHRASES: &[&str] = &[
    "it should be noted that",
    "essentially",
    "basically",
    "at the end of the day",
    "for all intents and purposes",
];

const GREETING_OPENERS: &[&str] = &[
    "Sure!",
    "Happy to help!",
    "Let me explain",
];

/// One em-dash per ~10 lines of prose is the threshold.
const EM_DASH_PER_LINES: usize = 10;

pub struct WritingStyle;

impl CrossCrateLint for WritingStyle {
    fn name(&self) -> &'static str { "writing-style" }

    fn source_only(&self) -> bool { false }

    fn default_severity(&self) -> Severity { Severity::PUSH_GATE }

    fn check_all(&self, crates: &[(&str, &LintContext)]) -> Vec<LintError> {
        let workspace_root = match crates.first() {
            Some((_, ctx)) => ctx.workspace_root,
            None => return Vec::new(),
        };

        let mut out = Vec::new();

        // Top-level public docs.
        for name in &["PRINCIPLES.md.tmpl", "DESIGN.md.tmpl", "WORKFLOW.md.tmpl"] {
            let path = workspace_root.join(name);
            check_file(&path, name, "writing-style", &mut out);
        }

        // Per-crate public docs.
        for (crate_name, ctx) in crates {
            for doc in &["README.md.tmpl", "DESIGN.md.tmpl", "BACKLOG.md.tmpl"] {
                let path = workspace_root.join("crates").join(crate_name).join(doc);
                if path.exists() {
                    check_file(&path, crate_name, "writing-style", &mut out);
                }
            }
            // Rust doc comments in lib.rs.
            check_rust_doc_comments(ctx.source, crate_name, &mut out);
        }

        // Agent rules + skills.
        let agent_dir = workspace_root.join("agent");
        if agent_dir.is_dir() {
            walk_md_tmpl(&agent_dir, "agent", &mut out);
        }

        out
    }
}

fn check_file(path: &Path, crate_name: &str, _lint: &str, out: &mut Vec<LintError>) {
    let content = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return,
    };
    check_text(&content, crate_name, out);
}

fn walk_md_tmpl(dir: &Path, crate_name: &str, out: &mut Vec<LintError>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_md_tmpl(&path, crate_name, out);
        } else if path.extension().map(|e| e == "tmpl").unwrap_or(false) {
            if path.file_name().map(|n| n.to_string_lossy().ends_with(".md.tmpl")).unwrap_or(false) {
                check_file(&path, crate_name, "writing-style", out);
            }
        }
    }
}

fn check_text(content: &str, crate_name: &str, out: &mut Vec<LintError>) {
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len().max(1);

    // 1. Em-dash density.
    let em_dash_count = content.matches('—').count();
    let threshold = total_lines / EM_DASH_PER_LINES;
    if em_dash_count > threshold && em_dash_count > 1 {
        out.push(LintError::with_severity(
            crate_name.to_string(),
            1,
            "writing-style",
            format!("em-dash density {em_dash_count} in {total_lines} lines exceeds threshold (1 per {EM_DASH_PER_LINES}); replace most with periods, commas, or parens"),
            Severity::PUSH_GATE,
        ));
    }

    // 2. Hype words, corporate jargon, filler phrases, greeting openers.
    let lower = content.to_lowercase();
    for (word, category) in HYPE_WORDS.iter().map(|w| (*w, "hype"))
        .chain(CORPORATE_JARGON.iter().map(|w| (*w, "jargon")))
        .chain(FILLER_PHRASES.iter().map(|w| (*w, "filler")))
    {
        let count = lower.matches(word).count();
        if count >= 2 {
            let line = line_of_first_match(&lines, word);
            out.push(LintError::with_severity(
                crate_name.to_string(),
                line,
                "writing-style",
                format!("`{word}` ({category}) used {count}x; see .shared/writing-style-fragment.md"),
                Severity::PUSH_GATE,
            ));
        }
    }
    for opener in GREETING_OPENERS {
        if content.contains(opener) {
            let line = line_of_first_match(&lines, opener);
            out.push(LintError::with_severity(
                crate_name.to_string(),
                line,
                "writing-style",
                format!("greeting opener `{opener}` — state the first fact instead"),
                Severity::PUSH_GATE,
            ));
        }
    }

    // 3. Exclamation marks in prose (not in inline code or fenced blocks).
    let excl_count = count_exclamations_in_prose(content);
    if excl_count > 1 {
        out.push(LintError::with_severity(
            crate_name.to_string(),
            1,
            "writing-style",
            format!("{excl_count} exclamation marks in prose; drop them"),
            Severity::PUSH_GATE,
        ));
    }

    // 4. Leading-list smell: first 3-4 sections should be prose, not flat bullets.
    if opens_with_flat_bullet_list(&lines) {
        out.push(LintError::with_severity(
            crate_name.to_string(),
            1,
            "writing-style",
            "opens with a flat bulleted list (no hierarchy) in the first 3-4 sections; frame with prose first".into(),
            Severity::PUSH_GATE,
        ));
    }

    // 5. `- <label>: <short>` label-colon cheat-sheet pattern (forbidden everywhere).
    let label_colon_count = count_label_colon_bullets(&lines);
    if label_colon_count > 3 {
        out.push(LintError::with_severity(
            crate_name.to_string(),
            1,
            "writing-style",
            format!("{label_colon_count} `- <label>: <short description>` bullets; use a glossary table or prose"),
            Severity::PUSH_GATE,
        ));
    }
}

fn line_of_first_match(lines: &[&str], needle: &str) -> usize {
    let lower_needle = needle.to_lowercase();
    for (i, line) in lines.iter().enumerate() {
        if line.to_lowercase().contains(&lower_needle) {
            return i + 1;
        }
    }
    1
}

fn count_exclamations_in_prose(content: &str) -> usize {
    let mut in_code_fence = false;
    let mut count = 0;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence { continue; }
        // Skip inline code `...` spans roughly.
        let mut in_code = false;
        for ch in line.chars() {
            match ch {
                '`' => in_code = !in_code,
                '!' if !in_code => count += 1,
                _ => {}
            }
        }
    }
    count
}

fn opens_with_flat_bullet_list(lines: &[&str]) -> bool {
    // Find the first 3 top-level sections (## or lower).
    let mut section_count = 0;
    let mut scan_from = 0;
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("# ") || line.starts_with("## ") {
            section_count += 1;
            if section_count == 1 {
                scan_from = i + 1;
            }
            if section_count > 3 { break; }
        }
    }
    // Within those top sections, look at the first non-blank non-heading content.
    // If it's a flat bullet list (no sub-bullets, all `- something`), flag it.
    let mut bullets_at_top = 0;
    let mut first_content_seen = false;
    for line in &lines[scan_from..lines.len().min(scan_from + 60)] {
        let t = line.trim();
        if t.is_empty() { continue; }
        if t.starts_with("#") {
            // New section; stop scanning the previous section's body.
            if first_content_seen { break; }
            continue;
        }
        if t.starts_with("- ") || t.starts_with("* ") {
            bullets_at_top += 1;
            first_content_seen = true;
        } else if t.starts_with("|") {
            // Tables are allowed if multi-col and multi-row. Don't flag.
            return false;
        } else {
            let _ = first_content_seen;
            break;
        }
    }
    bullets_at_top >= 4
}

fn count_label_colon_bullets(lines: &[&str]) -> usize {
    let mut n = 0;
    for line in lines {
        let t = line.trim_start();
        let rest = match t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            Some(r) => r,
            None => continue,
        };
        // Pattern: `<label>: <one short line>` where the colon is in the first
        // half of the content and there are no nested bullets.
        if let Some(colon_pos) = rest.find(':') {
            let before = &rest[..colon_pos];
            let after = rest[colon_pos + 1..].trim();
            let is_short_label = before.len() < 40 && !before.contains(' ').then_some(true).unwrap_or(false);
            let short_after = after.len() < 80 && !after.is_empty();
            if is_short_label && short_after {
                n += 1;
            }
        }
    }
    n
}

fn check_rust_doc_comments(source: &str, crate_name: &str, out: &mut Vec<LintError>) {
    // Extract doc comments (`///` and `//!`) as a concatenated corpus, then
    // apply check_text. Line numbers in errors point at the first match.
    let mut corpus = String::new();
    for line in source.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("///").or_else(|| t.strip_prefix("//!")) {
            corpus.push_str(rest);
            corpus.push('\n');
        }
    }
    if corpus.trim().is_empty() { return; }
    check_text(&corpus, crate_name, out);
}

#[allow(dead_code)]
fn _keep_path_alive(_p: PathBuf) {}
