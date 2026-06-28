use crate::models::Config;
use crate::storage;
use anyhow::Result;
use colored::Colorize;

pub fn execute(query: String) -> Result<()> {
    let query_lower = query.to_lowercase();
    let config = Config::load();

    let cards = storage::list_cards()?;
    let matches: Vec<_> = cards
        .into_iter()
        .filter(|c| {
            c.name.to_lowercase().contains(&query_lower)
                || c.description.to_lowercase().contains(&query_lower)
                || c.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
        })
        .collect();

    if matches.is_empty() {
        println!("No cards matching '{}'.", query);
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

    for card in &matches {
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

    println!("\n{} match(es) for '{}'", matches.len(), query);
    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
