use crate::models::Status;
use crate::storage;
use anyhow::{bail, Result};
use chrono::Utc;
use colored::Colorize;

pub fn execute(
    name: String,
    description: Option<String>,
    status: Option<String>,
    owner: Option<String>,
    add_tags: Vec<String>,
    remove_tags: Vec<String>,
) -> Result<()> {
    // Load existing card
    let mut card = storage::load_card(&name)?;

    let mut changes = Vec::new();

    // Update description if provided
    if let Some(desc) = description {
        card.description = desc.clone();
        changes.push(format!("description to '{}'", desc));
    }

    // Update status if provided
    if let Some(status_str) = status {
        let new_status = Status::from_str(&status_str)?;
        card.status = new_status;
        changes.push(format!("status to '{}'", new_status));
    }

    // Update owner if provided
    if let Some(owner_value) = owner {
        if owner_value.is_empty() || owner_value == "-" {
            card.owner = None;
            changes.push("removed owner".to_string());
        } else {
            card.owner = Some(owner_value.clone());
            changes.push(format!("owner to '{}'", owner_value));
        }
    }

    // Add tags
    for tag in add_tags {
        if !card.tags.contains(&tag) {
            card.tags.push(tag.clone());
            changes.push(format!("added tag '{}'", tag));
        }
    }

    // Remove tags
    for tag in &remove_tags {
        if let Some(pos) = card.tags.iter().position(|t| t == tag) {
            card.tags.remove(pos);
            changes.push(format!("removed tag '{}'", tag));
        }
    }

    // Check if any changes were made
    if changes.is_empty() {
        bail!("No changes specified");
    }

    // Update timestamp
    card.updated_at = Utc::now();

    // Validate
    card.validate()?;

    // Save
    storage::save_card(&card)?;

    println!("{} Updated card '{}'", "✓".green(), name.bold());
    for change in changes {
        println!("  • {}", change);
    }

    Ok(())
}
