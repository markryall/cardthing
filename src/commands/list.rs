use crate::models::{Card, Status};
use crate::storage;
use anyhow::Result;
use colored::Colorize;

pub fn execute(
    status_filter: Option<String>,
    owner_filter: Option<String>,
    tag_filter: Option<String>,
    format: String,
) -> Result<()> {
    let mut cards = storage::list_cards()?;

    // Apply filters
    if let Some(status) = status_filter {
        let status = Status::from_str(&status)?;
        cards.retain(|c| c.status == status);
    }

    if let Some(owner) = owner_filter {
        cards.retain(|c| c.owner.as_ref() == Some(&owner));
    }

    if let Some(tag) = tag_filter {
        cards.retain(|c| c.tags.contains(&tag));
    }

    // Sort by creation date
    cards.sort_by_key(|c| c.created_at);

    // Output
    match format.as_str() {
        "json" => print_json(&cards)?,
        "table" | _ => print_table(&cards)?,
    }

    Ok(())
}

fn print_table(cards: &[Card]) -> Result<()> {
    if cards.is_empty() {
        println!("No cards found.");
        return Ok(());
    }

    // Print header
    println!(
        "{:<20} {:<12} {:<15} {:<40}",
        "NAME".bold(),
        "STATUS".bold(),
        "OWNER".bold(),
        "DESCRIPTION".bold()
    );
    println!("{}", "=".repeat(90));

    // Print cards
    for card in cards {
        let status_str = format!("{}", card.status);
        let status_colored = status_str.color(card.status.color());
        let owner = card.owner.as_deref().unwrap_or("-");
        let desc = truncate(&card.description, 40);

        println!(
            "{:<20} {:<12} {:<15} {:<40}",
            card.name, status_colored, owner, desc
        );
    }

    println!("\nTotal: {} cards", cards.len());
    Ok(())
}

fn print_json(cards: &[Card]) -> Result<()> {
    let json = serde_json::to_string_pretty(cards)?;
    println!("{}", json);
    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
