use crate::commands::{parse_due_date, validate_priority};
use crate::dependencies;
use crate::models::Config;
use crate::storage;
use crate::storage::lock;
use anyhow::{bail, Result};
use chrono::Utc;
use colored::Colorize;

/// Changes to apply to a card. Everything defaults to "leave unchanged".
#[derive(Debug, Default)]
pub struct EditArgs {
    pub description: Option<String>,
    pub status: Option<String>,
    pub owner: Option<String>,
    pub clear_owner: bool,
    /// Take ownership only if nobody else owns the card (atomic claim)
    pub claim: Option<String>,
    pub add_tags: Vec<String>,
    pub remove_tags: Vec<String>,
    pub due: Option<String>,
    pub clear_due: bool,
    pub priority: Option<String>,
    pub needs_human: bool,
    pub clear_needs_human: bool,
    pub add_blocked_by: Vec<String>,
    pub remove_blocked_by: Vec<String>,
}

pub fn execute(name: String, args: EditArgs) -> Result<()> {
    let EditArgs {
        description,
        status,
        owner,
        clear_owner,
        claim,
        add_tags,
        remove_tags,
        due,
        clear_due,
        priority,
        needs_human,
        clear_needs_human,
        add_blocked_by,
        remove_blocked_by,
    } = args;

    if claim.is_some() && (owner.is_some() || clear_owner) {
        bail!("--claim cannot be combined with --owner or --clear-owner");
    }

    // Held until the card is saved, so the read-modify-write below can't
    // interleave with a worker's claim, another edit or an append.
    let _lock = lock::lock_card(&name)?;
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

    if clear_owner {
        card.owner = None;
        changes.push("cleared owner".to_string());
    } else if let Some(owner_value) = owner {
        if owner_value.is_empty() || owner_value == "-" {
            card.owner = None;
            changes.push("removed owner".to_string());
        } else {
            card.owner = Some(owner_value.clone());
            changes.push(format!("owner to '{}'", owner_value));
        }
    }

    if let Some(who) = claim {
        let who = who.trim();
        if who.is_empty() {
            bail!("--claim needs a name");
        }
        match card.owner.as_deref() {
            Some(current) if current == who => {
                changes.push(format!("already claimed by '{}'", who))
            }
            Some(current) => bail!("Card '{}' is already claimed by '{}'", card.name, current),
            None => {
                card.owner = Some(who.to_string());
                changes.push(format!("claimed by '{}'", who));
            }
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

    if needs_human && clear_needs_human {
        bail!("--needs-human and --clear-needs-human are mutually exclusive");
    }
    if needs_human {
        card.needs_human = true;
        changes.push("flagged as needing human intervention".to_string());
    } else if clear_needs_human {
        card.needs_human = false;
        changes.push("cleared human-intervention flag".to_string());
    }

    if !add_blocked_by.is_empty() {
        let cards = storage::list_cards()?;
        for blocker in add_blocked_by {
            let blocker = dependencies::resolve_blocker(&card.name, &blocker, &cards)?;
            if !card.blocked_by.contains(&blocker) {
                changes.push(format!("blocked by '{}'", blocker));
                card.blocked_by.push(blocker);
            }
        }
    }

    for blocker in &remove_blocked_by {
        let slug = storage::sanitize_filename(blocker);
        if let Some(pos) = card
            .blocked_by
            .iter()
            .position(|b| storage::sanitize_filename(b) == slug)
        {
            let removed = card.blocked_by.remove(pos);
            changes.push(format!("no longer blocked by '{}'", removed));
        }
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
