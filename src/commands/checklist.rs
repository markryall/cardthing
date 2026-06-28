use crate::models::ChecklistItem;
use crate::storage;
use anyhow::{bail, Result};
use chrono::Utc;
use colored::Colorize;

pub fn add(card_name: String, text: String) -> Result<()> {
    let mut card = storage::load_card(&card_name)?;
    card.checklist.push(ChecklistItem { text: text.clone(), checked: false });
    card.updated_at = Utc::now();
    storage::save_card(&card)?;
    println!("{} Added checklist item to '{}'", "✓".green(), card_name.bold());
    println!("  [ ] {}", text);
    Ok(())
}

pub fn toggle(card_name: String, index: usize) -> Result<()> {
    let mut card = storage::load_card(&card_name)?;
    if index == 0 || index > card.checklist.len() {
        bail!(
            "Index {} out of range (card has {} checklist items)",
            index,
            card.checklist.len()
        );
    }
    let item = &mut card.checklist[index - 1];
    item.checked = !item.checked;
    let mark = if item.checked { "[x]".green() } else { "[ ]".normal() };
    let text = item.text.clone();
    card.updated_at = Utc::now();
    storage::save_card(&card)?;
    println!("{} Toggled item {} on '{}'", "✓".green(), index, card_name.bold());
    println!("  {} {}", mark, text);
    Ok(())
}

pub fn remove(card_name: String, index: usize) -> Result<()> {
    let mut card = storage::load_card(&card_name)?;
    if index == 0 || index > card.checklist.len() {
        bail!(
            "Index {} out of range (card has {} checklist items)",
            index,
            card.checklist.len()
        );
    }
    let removed = card.checklist.remove(index - 1);
    card.updated_at = Utc::now();
    storage::save_card(&card)?;
    println!("{} Removed checklist item from '{}'", "✓".green(), card_name.bold());
    println!("  {}", removed.text);
    Ok(())
}
