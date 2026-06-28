use crate::commands::{parse_due_date, validate_priority};
use crate::models::Config;
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
    due: Option<String>,
    clear_due: bool,
    priority: Option<String>,
) -> Result<()> {
    let mut card = storage::load_card(&name)?;
    let mut changes = Vec::new();

    if let Some(desc) = description {
        card.description = desc.clone();
        changes.push(format!("description to '{}'", desc));
    }

    if let Some(status_str) = status {
        let config = Config::load();
        let new_status = config.validate_status(&status_str)?;
        card.status = new_status.clone();
        changes.push(format!("status to '{}'", new_status));
    }

    if let Some(owner_value) = owner {
        if owner_value.is_empty() || owner_value == "-" {
            card.owner = None;
            changes.push("removed owner".to_string());
        } else {
            card.owner = Some(owner_value.clone());
            changes.push(format!("owner to '{}'", owner_value));
        }
    }

    for tag in add_tags {
        if !card.tags.contains(&tag) {
            card.tags.push(tag.clone());
            changes.push(format!("added tag '{}'", tag));
        }
    }

    for tag in &remove_tags {
        if let Some(pos) = card.tags.iter().position(|t| t == tag) {
            card.tags.remove(pos);
            changes.push(format!("removed tag '{}'", tag));
        }
    }

    if clear_due {
        card.due_at = None;
        changes.push("cleared due date".to_string());
    } else if let Some(d) = due {
        card.due_at = Some(parse_due_date(&d)?);
        changes.push(format!("due date to '{}'", d));
    }

    if let Some(p) = priority {
        let validated = validate_priority(&p)?;
        changes.push(format!("priority to '{}'", validated));
        card.priority = Some(validated);
    }

    if changes.is_empty() {
        bail!("No changes specified");
    }

    card.updated_at = Utc::now();
    card.validate()?;
    storage::save_card(&card)?;

    println!("{} Updated card '{}'", "✓".green(), name.bold());
    for change in changes {
        println!("  • {}", change);
    }

    Ok(())
}
