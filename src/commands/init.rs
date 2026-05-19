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
"##;

pub fn execute() -> Result<()> {
    if fs::metadata(CARDS_DIR).is_ok() && fs::metadata(CONFIG_FILE).is_ok() {
        bail!("Already initialised (found {} and {})", CARDS_DIR, CONFIG_FILE);
    }

    if fs::metadata(CARDS_DIR).is_err() {
        fs::create_dir(CARDS_DIR)?;
        println!("{} Created {}/", "✓".green(), CARDS_DIR);
    }

    if fs::metadata(CONFIG_FILE).is_err() {
        fs::write(CONFIG_FILE, DEFAULT_CONFIG)?;
        println!("{} Created {}", "✓".green(), CONFIG_FILE);
    }

    println!("Run {} to start the board.", "cardthing serve".bold());
    Ok(())
}
