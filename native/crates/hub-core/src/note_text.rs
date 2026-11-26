//! How a note's text becomes its three columns, copied from the shipped parser.
//!
//! A note is stored as `title`, `content` and `tags`. The rules that split what the
//! user typed into those are small, exact and shared with the other build, so they
//! live here as pure functions with tests rather than inside a window where the only
//! way to check them is to type and look.
//!
//! Source of truth: `easyflowhub-app/src/lib/noteParser.ts`. Copied rule for rule,
//! because the two builds write the same rows -- a title derived one way here and
//! another way there would make the list title change depending on which window the
//! note was last edited in.
//!
//! Two things are *not* verbatim copies, both deliberate:
//!
//! * [`normalize_newlines`] runs at the boundary. The text control hands back CRLF
//!   and every stored row is LF; without it each save writes `\r` before every
//!   newline, which the browser shows as a stray character at the end of each line.
//! * [`TITLE_MAX_CHARS`] caps the stored title. The parser does not cap, but the one
//!   place in the shipped app that derives a stored title from content caps at 50 --
//!   so a title longer than that would only ever be produced here.

/// Longest stored title, in characters. From `useNotes.updateNoteContent`.
pub const TITLE_MAX_CHARS: usize = 50;

/// The three columns, as parsed out of one editor body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parsed {
    pub title: String,
    pub content: String,
    pub tags: String,
}

/// CRLF and lone CR become LF. Applied to every string that crosses into storage.
pub fn normalize_newlines(raw: &str) -> String {
    if !raw.contains('\r') {
        return raw.to_string();
    }
    raw.replace("\r\n", "\n").replace('\r', "\n")
}

/// Nothing but whitespace, or nothing at all.
pub fn is_blank(raw: &str) -> bool {
    raw.trim().is_empty()
}

/// Split a body into title, content and tags.
///
/// The tag rule is the odd one and comes straight from the shipped parser: **the
/// last line** becomes `tags` when it starts with `#` or `:`, and is removed from
/// the content. A line in the middle does not count -- that is what makes a note
/// whose body begins with a `#` heading keep its heading.
pub fn parse(raw: &str) -> Parsed {
    if is_blank(raw) {
        return Parsed::default();
    }

    // `split('\n')`, not `lines()`: the shipped code keeps a trailing empty line,
    // which decides whether an empty last line can be the tag line (it cannot, and
    // `lines()` would silently drop it and inspect the line before it instead).
    let lines: Vec<&str> = raw.split('\n').collect();
    let title = cap(strip_heading(lines[0].trim()));

    let last = lines[lines.len() - 1].trim();
    if !is_tag_line(last) {
        return Parsed {
            title,
            content: raw.to_string(),
            tags: String::new(),
        };
    }

    Parsed {
        title,
        content: lines[..lines.len() - 1].join("\n"),
        tags: tags_from_line(last),
    }
}

/// The title to show for a note: its stored title, else the first meaningful line.
///
/// This is `deriveDisplayTitle` from the shipped code, and it is why the list can
/// show a heading for a note whose `title` column is empty -- a note typed entirely
/// in the quick note may never have had a title written.
pub fn display_title(stored: &str, content: &str) -> String {
    let stored = stored.trim();
    if !stored.is_empty() {
        return stored.to_string();
    }
    content
        .split('\n')
        .map(|line| strip_heading(line.trim()))
        .find(|line| !line.is_empty())
        .unwrap_or_default()
}

/// Put the tag line back on the body for editing.
///
/// The counterpart of [`parse`]: a note that is opened, untouched and saved again
/// must produce the identical row. Since `parse` removed the last line, showing the
/// content alone would delete the tags on the next autosave -- a loss that looks
/// like "my tags disappeared by themselves".
pub fn format_for_display(content: &str, tags: &str) -> String {
    let list: Vec<&str> = tags.split_whitespace().filter(|tag| !tag.is_empty()).collect();
    if list.is_empty() {
        return content.to_string();
    }
    // The shipped build keeps whichever notation the note already used; tags that
    // contain a colon can only have come from the colon form.
    if list.iter().any(|tag| tag.contains(':')) {
        format!("{content}\n:{}", list.join(":"))
    } else {
        format!("{content}\n# {}", list.join(" "))
    }
}

/// Deduplicated, `#`-stripped, space-joined tags.
///
/// The separators are the shipped `normalizeTags` set, which includes the full-width
/// comma: a CJK keyboard produces `，` without anyone switching layout.
pub fn normalize_tags(raw: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for piece in raw.split(|c: char| c.is_whitespace() || c == ',' || c == '，') {
        let tag = piece.trim_start_matches('#').trim();
        if !tag.is_empty() && !out.contains(&tag) {
            out.push(tag);
        }
    }
    out.join(" ")
}

/// Remove a leading `#` run and the spaces after it (`^#+\s*`).
fn strip_heading(line: &str) -> String {
    match line.strip_prefix('#') {
        Some(rest) => rest.trim_start_matches('#').trim_start().to_string(),
        None => line.to_string(),
    }
}

fn cap(title: String) -> String {
    if title.chars().count() <= TITLE_MAX_CHARS {
        return title;
    }
    title.chars().take(TITLE_MAX_CHARS).collect()
}

/// Is this line a tag line?
///
/// Four accepted shapes, all from the shipped parser: `# 工作`, `#工作`, `标题 # 工作`
/// and `标题 :工作:重要`.
fn is_tag_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with('#') || trimmed.starts_with(':') {
        return true;
    }
    // "a # b": a word, spaces, `#`, then whitespace.
    let hash_after_word = after_first_word(line, '#')
        .is_some_and(|rest| rest.chars().next().is_some_and(char::is_whitespace));
    // "a :b:c": a word, spaces, `:`, spaces, another `:`.
    let colon_pair = after_first_word(line, ':')
        .is_some_and(|rest| rest.trim_start().starts_with(':'));
    hash_after_word || colon_pair
}

/// Everything after the first word and its trailing spaces, minus `marker`.
fn after_first_word<'a>(line: &'a str, marker: char) -> Option<&'a str> {
    let split = line.find(char::is_whitespace)?;
    line[split..].trim_start().strip_prefix(marker)
}

fn tags_from_line(line: &str) -> String {
    if let Some(hash) = line.find('#') {
        return line[hash + 1..]
            .trim()
            .split_whitespace()
            .map(|tag| tag.trim_start_matches('#'))
            .filter(|tag| !tag.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
    }
    if let Some(colon) = line.find(':') {
        // `:工作:重要` -- every `:segment`, stopping at whitespace within a segment.
        return line[colon..]
            .split(':')
            .filter(|segment| !segment.is_empty())
            .filter_map(|segment| segment.split_whitespace().next())
            .collect::<Vec<_>>()
            .join(" ");
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_line_becomes_the_title_with_its_heading_marks_stripped() {
        assert_eq!(parse("买菜清单\n鸡蛋").title, "买菜清单");
        assert_eq!(parse("## 周三\n正文").title, "周三");
        assert_eq!(parse("#标签行\n正文").title, "标签行");
    }

    #[test]
    fn an_empty_first_line_leaves_the_title_empty_for_the_list_to_fill_in() {
        // Deliberate: the stored title is the first line, not the first *meaningful*
        // line. The list falls back to `display_title`, so nothing is lost -- but a
        // note that starts with a blank line must not silently claim the line below.
        let parsed = parse("\n买菜清单");
        assert_eq!(parsed.title, "");
        assert_eq!(display_title("", &parsed.content), "买菜清单");
    }

    #[test]
    fn a_trailing_hash_line_becomes_tags_and_leaves_the_content() {
        let parsed = parse("买菜清单\n鸡蛋\n# 工作 重要");
        assert_eq!(parsed.title, "买菜清单");
        assert_eq!(parsed.content, "买菜清单\n鸡蛋");
        assert_eq!(parsed.tags, "工作 重要");
    }

    #[test]
    fn a_trailing_colon_line_becomes_tags() {
        let parsed = parse("买菜清单\n:工作:重要");
        assert_eq!(parsed.content, "买菜清单");
        assert_eq!(parsed.tags, "工作 重要");
    }

    #[test]
    fn a_hash_in_the_middle_is_content_not_tags() {
        // The rule is "last line", and this is the case that proves it is not
        // "any line that starts with #".
        let parsed = parse("# 周三\n# 不是标签\n正文");
        assert_eq!(parsed.tags, "");
        assert_eq!(parsed.content, "# 周三\n# 不是标签\n正文");
    }

    #[test]
    fn a_trailing_empty_line_is_content_and_cannot_be_a_tag_line() {
        // `split('\n')` keeps the empty line; `lines()` would drop it and inspect
        // `# 工作` instead, turning content into tags that nobody typed.
        let parsed = parse("正文\n# 工作\n");
        assert_eq!(parsed.tags, "");
        assert_eq!(parsed.content, "正文\n# 工作\n");
    }

    #[test]
    fn content_is_never_trimmed_at_the_end() {
        // The shipped parser keeps trailing newlines on purpose: autosave must not
        // delete the newline the user just pressed.
        let body = "买菜\n\n";
        assert_eq!(parse(body).content, body);
    }

    #[test]
    fn blank_bodies_parse_to_nothing() {
        for body in ["", "   ", "\n", "\t\n  "] {
            assert_eq!(parse(body), Parsed::default(), "{body:?}");
            assert!(is_blank(body));
        }
    }

    #[test]
    fn crlf_from_the_control_becomes_lf_before_it_is_parsed() {
        // Measured shape of what the control returns: one CRLF per paragraph break.
        let text = normalize_newlines("买菜清单\r\n鸡蛋\r\n");
        assert_eq!(text, "买菜清单\n鸡蛋\n");
        assert_eq!(parse(&text).title, "买菜清单");
        // And a lone CR (older controls) is a break too, not a character.
        assert_eq!(normalize_newlines("a\rb"), "a\nb");
    }

    #[test]
    fn a_body_with_no_carriage_return_is_returned_untouched() {
        // The fast path: every save calls this, and most saves have nothing to fix.
        let body = "买菜\n鸡蛋";
        assert_eq!(normalize_newlines(body), body);
    }

    #[test]
    fn tags_are_deduplicated_and_stripped_of_their_hash() {
        assert_eq!(normalize_tags("#工作 ##重要 工作"), "工作 重要");
        // Half- and full-width commas both separate, and order is preserved.
        assert_eq!(normalize_tags("工作,重要，待办"), "工作 重要 待办");
    }

    #[test]
    fn the_display_form_reverses_the_parse_so_a_round_trip_is_stable() {
        // The property that matters: open, do nothing, save -- identical row.
        for body in [
            "买菜清单\n鸡蛋\n# 工作 重要",
            "只有一行",
            "多行\n\n\n正文",
        ] {
            let parsed = parse(body);
            let shown = format_for_display(&parsed.content, &parsed.tags);
            let again = parse(&shown);
            assert_eq!(again.content, parsed.content, "content changed for {body:?}");
            assert_eq!(again.tags, parsed.tags, "tags changed for {body:?}");
        }
    }

    #[test]
    fn the_display_form_keeps_the_colon_notation_for_colon_tags() {
        assert_eq!(format_for_display("a", "工作:重要"), "a\n:工作:重要");
        assert_eq!(format_for_display("a", "工作 重要"), "a\n# 工作 重要");
        assert_eq!(format_for_display("a", ""), "a");
    }

    #[test]
    fn a_long_title_is_capped_at_fifty_characters() {
        let body = "一".repeat(120);
        assert_eq!(parse(&body).title.chars().count(), TITLE_MAX_CHARS);
        // ... and the cap counts characters, not bytes: a 60-character CJK title is
        // 180 bytes and must still be cut at 50.
        let cjk = "青羽笔记".repeat(20);
        assert_eq!(parse(&cjk).title.chars().count(), TITLE_MAX_CHARS);
    }
}
