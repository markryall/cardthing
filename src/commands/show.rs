use crate::models::Config;
use crate::storage;
use anyhow::Result;
use colored::Colorize;

pub fn execute(name: String) -> Result<()> {
    let card = storage::load_card(&name)?;
    let config = Config::load();

    let color = config
        .find_status(&card.status)
        .map(|s| s.terminal_color())
        .unwrap_or(colored::Color::White);

    println!("{}", card.name.bold());
    println!("  Status:  {}", card.status.color(color));
    println!(
        "  Owner:   {}",
        card.owner.as_deref().unwrap_or("-")
    );
    println!("  Created: {}", card.created_at.format("%Y-%m-%d %H:%M UTC"));
    println!("  Updated: {}", card.updated_at.format("%Y-%m-%d %H:%M UTC"));

    if let Some(ref p) = card.priority {
        let colored_priority = match p.as_str() {
            "high" => p.red(),
            "medium" => p.yellow(),
            _ => p.green(),
        };
        println!("  Priority:{}", colored_priority);
    }

    if let Some(due) = card.due_at {
        println!("  Due:     {}", due.format("%Y-%m-%d"));
    }

    if card.agent {
        println!("  Agent:   {}", "working on this card".blue());
    }

    if card.needs_human {
        println!("  Human:   {}", "intervention required".yellow().bold());
    }

    if !card.tags.is_empty() {
        println!("  Tags:    {}", card.tags.join(", "));
    }

    if !card.description.is_empty() {
        println!("\n{}", "Description".bold());
        println!("  {}", card.description);
    }

    if !card.checklist.is_empty() {
        let done = card.checklist.iter().filter(|i| i.checked).count();
        println!(
            "\n{} ({}/{})",
            "Checklist".bold(),
            done,
            card.checklist.len()
        );
        for item in &card.checklist {
            let mark = if item.checked {
                "[x]".green()
            } else {
                "[ ]".normal()
            };
            println!("  {} {}", mark, item.text);
        }
    }

    Ok(())
}
