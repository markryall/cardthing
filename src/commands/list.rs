use crate::models::{Card, Config};
use crate::storage;
use anyhow::Result;
use colored::Colorize;

pub fn execute(
    status_filter: Option<String>,
    owner_filter: Option<String>,
    tag_filter: Option<String>,
    format: String,
) -> Result<()> {
    let config = Config::load();
    let mut cards = storage::list_cards()?;

    if let Some(status) = status_filter {
        let status = config.validate_status(&status)?;
        cards.retain(|c| c.status == status);
    }

    if let Some(owner) = owner_filter {
        cards.retain(|c| c.owner.as_ref() == Some(&owner));
    }

    if let Some(tag) = tag_filter {
        cards.retain(|c| c.tags.contains(&tag));
    }

    cards.sort_by_key(|c| c.created_at);

    match format.as_str() {
        "json" => print_json(&cards)?,
        _ => print_table(&cards, &config)?,
    }

    Ok(())
}

fn print_table(cards: &[Card], config: &Config) -> Result<()> {
    if cards.is_empty() {
        println!("No cards found.");
        return Ok(());
    }

    println!(
        "{:<20} {:<12} {:<15} {:<40}",
        "NAME".bold(),
        "STATUS".bold(),
        "OWNER".bold(),
        "DESCRIPTION".bold()
    );
    println!("{}", "=".repeat(90));

    for card in cards {
        let color = config
            .find_status(&card.status)
            .map(|s| s.terminal_color())
            .unwrap_or(colored::Color::White);
        let status_colored = card.status.color(color);
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
