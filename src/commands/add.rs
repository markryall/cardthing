use crate::models::{Card, Status};
use crate::storage;
use anyhow::{bail, Result};
use colored::Colorize;

pub fn execute(
    name: String,
    description: Option<String>,
    status: String,
    owner: Option<String>,
    tags: Vec<String>,
) -> Result<()> {
    // Check if card already exists
    if storage::card_exists(&name) {
        bail!("Card '{}' already exists", name);
    }

    // Parse status
    let status = Status::from_str(&status)?;

    // Create card
    let mut card = Card::new(name.clone(), description.unwrap_or_default());
    card.status = status;
    card.owner = owner;
    card.tags = tags;

    // Validate
    card.validate()?;

    // Save
    storage::save_card(&card)?;

    println!("{} Created card '{}'", "✓".green(), name.bold());
    Ok(())
}
