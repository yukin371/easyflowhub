//! Markdown checkbox lines: what counts as a todo, and the three rewrites.
//!
//! Source of truth: `easyflowhub-app/src/lib/todoParser.ts`. Todos are **not a
//! table** -- the product keeps them as `- [ ]` lines inside note bodies -- so the
//! card and the shipped build have to agree on the exact line grammar. A rule
//! copied by eye would drift the first time someone typed a numbered list, and the
//! symptom ("my todo vanished from the card") would be blamed on the card.
//!
//! Everything here is a pure function over the body, which is why it lives in
//! `hub-core` instead of in the window: the write path has to be testable without
//! a desktop, and the card must not be the only place that knows what `- [X]`
//! means.

/// One checkbox line, as parsed out of a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// 0-based line index in the body.
    pub index: usize,
    /// The text without its list marker, checkbox and `@done:` suffix.
    pub text: String,
    pub done: bool,
    /// The `@done:` stamp the line carried, kept verbatim so a rewrite can put it
    /// back instead of minting a new instant for an old completion.
    pub done_at: Option<String>,
    /// Everything before the marker, kept so a rewrite does not re-indent a list.
    indent: String,
    /// `-`, `*` or `1.` -- kept for the same reason.
    marker: String,
}

/// Why a rewrite was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The line is not a checkbox line (any more).
    NotATodo,
    /// The replacement text is empty.
    Empty,
}

/// Where a snapshot's line now lives in the current body.
///
/// The frontend's F05 rule, copied: a line index from another read is a *hint*, and
/// the text is the identity. A body that gained a line above the todo must not turn
/// a tick into a tick of the wrong item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locate {
    Exact(usize),
    Relocated(usize),
    /// More than one todo carries that text: refuse rather than guess.
    Conflict,
    Missing,
}

/// Split one line into `(indent, marker, done, text)`.
///
/// The grammar is the frontend's regex, hand-written because this crate has no
/// regex dependency: leading whitespace, a marker (`-`, `*`, or digits plus `.`),
/// optional whitespace, `[`, a space or `x`/`X`, `]`, optional whitespace, text.
/// A line with a checkbox and no text still parses -- it is a todo the list simply
/// does not show, and a tick on it must still be able to add text later.
fn split(raw: &str) -> Option<(String, String, bool, String)> {
    let indent_len = raw.len() - raw.trim_start_matches([' ', '\t']).len();
    let (indent, rest) = raw.split_at(indent_len);

    let (marker, after) = if let Some(after) = rest.strip_prefix('-') {
        ("-".to_string(), after)
    } else if let Some(after) = rest.strip_prefix('*') {
        ("*".to_string(), after)
    } else {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let after = rest[digits..].strip_prefix('.')?;
        (rest[..digits + 1].to_string(), after)
    };

    let after = after.trim_start().strip_prefix('[')?;
    let mut chars = after.chars();
    let done = match chars.next()? {
        ' ' => false,
        'x' | 'X' => true,
        _ => return None,
    };
    let after = chars.as_str().strip_prefix(']')?;
    // `.` in the regex, so the line has to carry at least one character after the
    // bracket. Trailing spaces count as "a character" there and as empty text
    // here, exactly like the frontend's `.trim()` + `if (text)` pair.
    if after.is_empty() {
        return None;
    }
    Some((indent.to_string(), marker, done, after.trim().to_string()))
}

/// The `@done:` timestamp at the end of `text`, if there is one.
fn done_at(text: &str) -> Option<&str> {
    let trimmed = text.trim_end();
    let start = trimmed.rfind("@done:")?;
    let stamp = trimmed[start + "@done:".len()..].trim();
    if stamp.is_empty() {
        return None;
    }
    // Deliberately lax about the ISO shape and strict about the alphabet: anything
    // that is not a timestamp character means the `@done:` was part of the sentence
    // ("see @done:later"), and it must stay in the text.
    if !stamp
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '-' | ':' | '.' | '+' | 'T' | 'Z' | 'z'))
    {
        return None;
    }
    // `@done:` glued to a word is not a suffix either (`note@done:2026`).
    if start > 0 && !trimmed[..start].ends_with(char::is_whitespace) {
        return None;
    }
    Some(stamp)
}

/// The text without its trailing `@done:` stamp.
fn strip_done(text: &str) -> &str {
    match done_at(text) {
        Some(stamp) => text.trim_end()[..text.trim_end().len() - stamp.len() - "@done:".len()]
            .trim_end(),
        None => text.trim_end(),
    }
}

/// Every todo line in a body, in document order.
pub fn lines(content: &str) -> Vec<Line> {
    content
        .split('\n')
        .enumerate()
        .filter_map(|(index, raw)| {
            let (indent, marker, done, text) = split(raw)?;
            let done_at = done_at(&text).map(str::to_string);
            let text = strip_done(&text).to_string();
            if text.is_empty() {
                return None;
            }
            Some(Line {
                index,
                text,
                done,
                done_at,
                indent,
                marker,
            })
        })
        .collect()
}

/// Tick or untick the todo on line `index`, stamping `at` on a tick.
///
/// `at` is an ISO timestamp from [`crate::clock::now_iso`], passed in rather than
/// read here so the rewrite is a pure function and a test can name the instant.
pub fn toggle(content: &str, index: usize, at: &str) -> Result<String, Refused> {
    rewrite(content, index, |line| {
        if line.done {
            Ok(format!("{}{} [ ] {}", line.indent, line.marker, line.text))
        } else {
            Ok(format!(
                "{}{} [x] {} @done:{at}",
                line.indent, line.marker, line.text
            ))
        }
    })
}

/// Replace the text of the todo on line `index`, keeping its state.
pub fn set_text(content: &str, index: usize, text: &str) -> Result<String, Refused> {
    let next = text.trim();
    if next.is_empty() {
        return Err(Refused::Empty);
    }
    rewrite(content, index, |line| {
        let suffix = line
            .done_at
            .as_ref()
            .map(|stamp| format!(" @done:{stamp}"))
            .unwrap_or_default();
        let mark = if line.done { "x" } else { " " };
        Ok(format!(
            "{}{} [{mark}] {next}{suffix}",
            line.indent, line.marker
        ))
    })
}

/// Append an open todo to the end of the body.
pub fn append(content: &str, text: &str) -> Result<String, Refused> {
    let next = text.trim();
    if next.is_empty() {
        return Err(Refused::Empty);
    }
    if content.trim().is_empty() {
        return Ok(format!("- [ ] {next}"));
    }
    Ok(format!("{}\n- [ ] {next}", content.trim_end()))
}

fn rewrite<F>(content: &str, index: usize, f: F) -> Result<String, Refused>
where
    F: FnOnce(&Line) -> Result<String, Refused>,
{
    let mut all: Vec<String> = content.split('\n').map(str::to_string).collect();
    let raw = all.get(index).ok_or(Refused::NotATodo)?;
    let (indent, marker, done, text) = split(raw).ok_or(Refused::NotATodo)?;
    let done_at = done_at(&text).map(str::to_string);
    let text = strip_done(&text).to_string();
    let line = Line {
        index,
        text,
        done,
        done_at,
        indent,
        marker,
    };
    all[index] = f(&line)?;
    Ok(all.join("\n"))
}

/// Find where the todo named `text` lives now.
///
/// `index` is the line the caller last saw it on. It is used only to distinguish
/// `Exact` from `Relocated` -- the answer is decided by the text, because a tick
/// applies to a *task*, not to whatever happened to be on row 7.
pub fn locate(content: &str, index: usize, text: &str) -> Locate {
    let found: Vec<usize> = lines(content)
        .into_iter()
        .filter(|line| line.text == text)
        .map(|line| line.index)
        .collect();
    match found.as_slice() {
        [] => Locate::Missing,
        [only] if *only == index => Locate::Exact(*only),
        [only] => Locate::Relocated(*only),
        _ => Locate::Conflict,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: &str = "2026-11-28T04:00:00.000Z";

    #[test]
    fn every_marker_the_frontend_accepts_is_a_todo() {
        let body = "- [ ] a\n* [x] b\n1. [ ] c\n12. [X] d\n  - [ ] indented\n";
        let got: Vec<(String, bool)> = lines(body)
            .into_iter()
            .map(|line| (line.text, line.done))
            .collect();
        assert_eq!(
            got,
            vec![
                ("a".to_string(), false),
                ("b".to_string(), true),
                ("c".to_string(), false),
                ("d".to_string(), true),
                ("indented".to_string(), false),
            ]
        );
    }

    #[test]
    fn a_plain_bullet_or_an_empty_checkbox_is_not_a_todo() {
        assert!(lines("- 只是列表\n- [ ]\n- [] x\n文字 [ ] y").is_empty());
    }

    #[test]
    fn ticking_stamps_the_instant_and_unticking_removes_it() {
        let body = "- [ ] 买菜\n";
        let ticked = toggle(body, 0, AT).expect("tick");
        assert_eq!(ticked, format!("- [x] 买菜 @done:{AT}\n"));
        let unticked = toggle(&ticked, 0, AT).expect("untick");
        assert_eq!(unticked, "- [ ] 买菜\n");
    }

    #[test]
    fn a_rewrite_keeps_the_list_marker_and_the_indent() {
        let body = "  1. [ ] 第一项\n";
        let ticked = toggle(body, 0, AT).expect("tick");
        assert_eq!(ticked, format!("  1. [x] 第一项 @done:{AT}\n"));
    }

    #[test]
    fn rewriting_the_text_keeps_the_state_and_the_timestamp() {
        let body = format!("- [x] 旧文字 @done:{AT}\n");
        let next = set_text(&body, 0, "  新文字  ").expect("rewrite");
        assert_eq!(next, format!("- [x] 新文字 @done:{AT}\n"));
    }

    #[test]
    fn empty_text_is_refused_instead_of_erasing_the_task() {
        assert_eq!(set_text("- [ ] x", 0, "   "), Err(Refused::Empty));
        assert_eq!(append("- [ ] x", ""), Err(Refused::Empty));
    }

    #[test]
    fn appending_starts_a_list_in_an_empty_body_and_extends_one_otherwise() {
        assert_eq!(append("", " 新待办 ").unwrap(), "- [ ] 新待办");
        assert_eq!(append("正文\n\n", "新待办").unwrap(), "正文\n- [ ] 新待办");
    }

    #[test]
    fn a_done_word_inside_the_sentence_is_not_a_timestamp() {
        // `@done:` glued to a word, and a suffix that is not a timestamp alphabet,
        // both stay part of the text.
        assert_eq!(lines("- [x] 见 @done:later")[0].text, "见 @done:later");
        assert_eq!(lines("- [x] note@done:2026")[0].text, "note@done:2026");
        assert_eq!(
            lines("- [x] 完成 @done:2026-11-27T10:00:00.000Z")[0].text,
            "完成"
        );
    }

    #[test]
    fn locating_prefers_the_text_over_a_stale_line_number() {
        let body = "新增一行\n- [ ] 买菜\n";
        assert_eq!(locate(body, 1, "买菜"), Locate::Exact(1));
        assert_eq!(locate(body, 0, "买菜"), Locate::Relocated(1));
        assert_eq!(locate(body, 0, "洗衣"), Locate::Missing);
    }

    #[test]
    fn two_tasks_with_the_same_text_are_a_conflict_not_a_coin_flip() {
        let body = "- [ ] 买菜\n- [ ] 买菜\n";
        assert_eq!(locate(body, 0, "买菜"), Locate::Conflict);
    }

    #[test]
    fn a_refused_rewrite_does_not_change_the_body() {
        let body = "- 普通列表\n";
        assert_eq!(toggle(body, 0, AT), Err(Refused::NotATodo));
        assert_eq!(toggle(body, 9, AT), Err(Refused::NotATodo));
    }
}
