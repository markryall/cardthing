use crate::storage;
use crate::storage::lock;
use anyhow::{bail, Context, Result};
use chrono::Utc;
use colored::Colorize;
use std::io::Read;

/// Append `text` to a card's description, optionally at the end of a markdown
/// section, as one locked read-modify-write so concurrent appends never lose
/// each other's text. `text` of `-` reads stdin.
pub fn execute(name: String, text: String, section: Option<String>) -> Result<()> {
    let text = if text == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("Failed to read text from stdin")?;
        buf
    } else {
        text
    };
    if text.trim().is_empty() {
        bail!("Nothing to append");
    }

    let _lock = lock::lock_card(&name)?;
    let mut card = storage::load_card(&name)?;
    card.description = append_text(&card.description, section.as_deref(), &text);
    card.updated_at = Utc::now();
    storage::save_card(&card)?;

    match section {
        Some(s) => println!(
            "{} Appended to '{}' in '{}'",
            "✓".green(),
            heading_title(&s),
            card.name.bold()
        ),
        None => println!("{} Appended to '{}'", "✓".green(), card.name.bold()),
    }
    Ok(())
}

/// `## Decisions so far` and `Decisions so far` both name the same section.
fn heading_title(section: &str) -> &str {
    section.trim().trim_start_matches('#').trim()
}

/// The level and title of a markdown ATX heading line (`## Title`).
fn heading(line: &str) -> Option<(usize, &str)> {
    let level = line.chars().take_while(|c| *c == '#').count();
    let rest = &line[level..];
    if (1..=6).contains(&level) && (rest.is_empty() || rest.starts_with(' ')) {
        Some((level, rest.trim().trim_end_matches('#').trim()))
    } else {
        None
    }
}

fn is_list_item(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("- ")
        || t.starts_with("* ")
        || t.starts_with("+ ")
        || t.split_once(". ")
            .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

/// Pure core of `append`: returns `description` with `text` added at the end,
/// or at the end of `section` (created as `## <section>` if missing). A list
/// item appended after another list item joins the list; anything else is
/// separated by a blank line. Headings inside fenced code blocks are ignored.
pub fn append_text(description: &str, section: Option<&str>, text: &str) -> String {
    let text = text.trim_end_matches(['\n', '\r']);
    let description = description.trim_end();

    let Some(section) = section.map(heading_title) else {
        return join_blocks(description, text);
    };

    let lines: Vec<&str> = description.lines().collect();
    let mut in_fence = false;
    let mut headings = Vec::new(); // (line index, level, title)
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        } else if !in_fence {
            if let Some((level, title)) = heading(line) {
                headings.push((i, level, title));
            }
        }
    }

    let Some(&(start, level, _)) = headings
        .iter()
        .find(|(_, _, title)| title.eq_ignore_ascii_case(section))
    else {
        return join_blocks(description, &format!("## {}\n\n{}", section, text));
    };

    let end = headings
        .iter()
        .find(|(i, l, _)| *i > start && *l <= level)
        .map(|(i, _, _)| *i)
        .unwrap_or(lines.len());
    let mut last = end; // one past the section's last non-blank line
    while last > start + 1 && lines[last - 1].trim().is_empty() {
        last -= 1;
    }

    let mut out: Vec<&str> = lines[..last].to_vec();
    let continues_list = last > start + 1
        && is_list_item(lines[last - 1])
        && text.lines().next().is_some_and(is_list_item);
    if !continues_list {
        out.push("");
    }
    out.extend(text.lines());
    if end < lines.len() {
        out.push("");
        out.extend(&lines[end..]);
    }
    out.join("\n")
}

fn join_blocks(a: &str, b: &str) -> String {
    if a.is_empty() {
        b.to_string()
    } else {
        format!("{}\n\n{}", a, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &str = "## Destination\n\nA spec.\n\n## Decisions so far\n\n- [A](a): yes\n\n## Not yet specified\n\nfog\n";

    #[test]
    fn appends_to_end_without_section() {
        assert_eq!(append_text("", None, "hi\n"), "hi");
        assert_eq!(append_text("body\n\n", None, "hi"), "body\n\nhi");
    }

    #[test]
    fn list_item_joins_list_at_end_of_section() {
        let got = append_text(MAP, Some("decisions so far"), "- [B](b): no");
        assert_eq!(
            got,
            "## Destination\n\nA spec.\n\n## Decisions so far\n\n- [A](a): yes\n- [B](b): no\n\n## Not yet specified\n\nfog"
        );
    }

    #[test]
    fn first_item_in_empty_section_gets_a_blank_line() {
        let desc = "## Decisions so far\n\n## Out of scope\n";
        let got = append_text(desc, Some("## Decisions so far"), "- x");
        assert_eq!(got, "## Decisions so far\n\n- x\n\n## Out of scope");
        let got = append_text("## Out of scope", Some("Out of scope"), "- y");
        assert_eq!(got, "## Out of scope\n\n- y");
    }

    #[test]
    fn prose_is_a_new_paragraph_and_subheadings_stay_in_section() {
        let desc = "## Answer\n\nfirst\n\n### detail\n\nmore\n\n## Next\n";
        let got = append_text(desc, Some("Answer"), "second");
        assert_eq!(
            got,
            "## Answer\n\nfirst\n\n### detail\n\nmore\n\nsecond\n\n## Next"
        );
    }

    #[test]
    fn missing_section_is_created_at_the_end() {
        let got = append_text("## Question\n\nWhy?", Some("Answer"), "Because.");
        assert_eq!(got, "## Question\n\nWhy?\n\n## Answer\n\nBecause.");
        assert_eq!(append_text("", Some("Answer"), "x"), "## Answer\n\nx");
    }

    #[test]
    fn headings_in_code_fences_are_ignored() {
        let desc = "## Notes\n\n```md\n## Answer\n```\n";
        let got = append_text(desc, Some("Answer"), "real");
        assert_eq!(
            got,
            "## Notes\n\n```md\n## Answer\n```\n\n## Answer\n\nreal"
        );
    }

    #[test]
    fn hashes_without_space_are_not_headings() {
        assert_eq!(heading("#tag"), None);
        assert_eq!(heading("## Out of scope ##"), Some((2, "Out of scope")));
    }
}
