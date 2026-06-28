use crate::models::Config;
use crate::storage;
use anyhow::Result;
use colored::Colorize;
use std::collections::HashMap;

pub fn execute() -> Result<()> {
    let cards = storage::list_cards()?;
    let config = Config::load();
    let total = cards.len();

    if total == 0 {
        println!("No cards.");
        return Ok(());
    }

    let mut by_status: HashMap<String, usize> = HashMap::new();
    let mut by_priority: HashMap<String, usize> = HashMap::new();
    let mut by_owner: HashMap<String, usize> = HashMap::new();

    for card in &cards {
        *by_status.entry(card.status.clone()).or_insert(0) += 1;
        if let Some(ref p) = card.priority {
            *by_priority.entry(p.clone()).or_insert(0) += 1;
        }
        let owner = card.owner.clone().unwrap_or_else(|| "unassigned".to_string());
        *by_owner.entry(owner).or_insert(0) += 1;
    }

    println!("{} ({} total)", "Cards".bold(), total);

    println!("\n{}", "By status".bold());
    for status_def in &config.statuses {
        let count = by_status.get(&status_def.id).copied().unwrap_or(0);
        let color = status_def.terminal_color();
        println!("  {:<12} {}", status_def.id.color(color), count);
    }

    if !by_priority.is_empty() {
        println!("\n{}", "By priority".bold());
        for p in &["high", "medium", "low"] {
            if let Some(&count) = by_priority.get(*p) {
                let colored = match *p {
                    "high" => p.red(),
                    "medium" => p.yellow(),
                    _ => p.green(),
                };
                println!("  {:<12} {}", colored, count);
            }
        }
    }

    println!("\n{}", "By owner".bold());
    let mut owners: Vec<(&String, &usize)> = by_owner.iter().collect();
    owners.sort_by(|a, b| b.1.cmp(a.1));
    for (owner, count) in owners {
        println!("  {:<20} {}", owner, count);
    }

    Ok(())
}
