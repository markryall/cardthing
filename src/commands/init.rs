use anyhow::{bail, Result};
use colored::Colorize;
use std::fs;

const CARDS_DIR: &str = ".cards";
const CONFIG_FILE: &str = ".cards.toml";
const DEFAULT_CONFIG: &str = r##"title = "My Board"

[[statuses]]
id = "todo"
label = "Todo"
color = "#f59e0b"

[[statuses]]
id = "inprogress"
label = "In Progress"
color = "#3b82f6"

[[statuses]]
id = "done"
label = "Done"
color = "#10b981"

[[statuses]]
id = "blocked"
label = "Blocked"
color = "#ef4444"

# Uncomment to let 'cardthing work <name>' pick up cards automatically.
# [[workers]]
# name = "worker-1"
# watch = "todo"
# done = "done"
# prompt = "Work through the card and implement what it describes."
# model = "claude"
# effort = "medium"
# allowed_tools = ["Bash(cardthing:*)"]
# poll_seconds = 15
# workspace = false
"##;

const GITIGNORE_FILE: &str = ".gitignore";
const GITIGNORE_ENTRIES: &[&str] = &[".cards/.logs/", ".cards/.claims/"];

pub fn execute() -> Result<()> {
    if fs::metadata(CARDS_DIR).is_ok() && fs::metadata(CONFIG_FILE).is_ok() {
        bail!(
            "Already initialised (found {} and {})",
            CARDS_DIR,
            CONFIG_FILE
        );
    }

    if fs::metadata(CARDS_DIR).is_err() {
        fs::create_dir(CARDS_DIR)?;
        println!("{} Created {}/", "✓".green(), CARDS_DIR);
    }

    if fs::metadata(CONFIG_FILE).is_err() {
        fs::write(CONFIG_FILE, DEFAULT_CONFIG)?;
        println!("{} Created {}", "✓".green(), CONFIG_FILE);
    }

    update_gitignore()?;

    println!("Run {} to start the board.", "cardthing serve".bold());
    Ok(())
}

/// Make sure .cards/.logs/ and .cards/.claims/ are ignored, creating or
/// extending .gitignore as needed.
fn update_gitignore() -> Result<()> {
    let existing = fs::read_to_string(GITIGNORE_FILE).unwrap_or_default();
    let missing: Vec<&str> = GITIGNORE_ENTRIES
        .iter()
        .filter(|entry| !existing.lines().any(|line| line.trim() == **entry))
        .copied()
        .collect();

    if missing.is_empty() {
        return Ok(());
    }

    let mut content = existing.clone();
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    for entry in &missing {
        content.push_str(entry);
        content.push('\n');
    }

    fs::write(GITIGNORE_FILE, content)?;
    if existing.is_empty() {
        println!("{} Created {}", "✓".green(), GITIGNORE_FILE);
    } else {
        println!("{} Updated {}", "✓".green(), GITIGNORE_FILE);
    }

    Ok(())
}
