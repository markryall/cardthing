use crate::commands::parse_due_date;
use crate::models::{Card, Config};
use crate::storage;
use anyhow::{bail, Result};
use colored::Colorize;

pub fn execute(
    name: String,
    description: Option<String>,
    status: String,
    owner: Option<String>,
    tags: Vec<String>,
    due: Option<String>,
) -> Result<()> {
    if storage::card_exists(&name) {
        bail!("Card '{}' already exists", name);
    }

    let config = Config::load();
    let status = config.validate_status(&status)?;

    let mut card = Card::new(name.clone(), description.unwrap_or_default());
    card.status = status;
    card.owner = owner;
    card.tags = tags;
    if let Some(d) = due {
        card.due_at = Some(parse_due_date(&d)?);
    }

    card.validate()?;
    storage::save_card(&card)?;

    println!("{} Created card '{}'", "✓".green(), name.bold());
    Ok(())
}
