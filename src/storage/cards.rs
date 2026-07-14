use crate::models::Card;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

const CARDS_DIR: &str = ".cards";

pub fn get_cards_path() -> PathBuf {
    PathBuf::from(CARDS_DIR)
}

pub fn ensure_cards_directory() -> Result<()> {
    let path = get_cards_path();
    if !path.exists() {
        fs::create_dir(&path).context("Failed to create .cards directory")?;
    }
    Ok(())
}

pub fn save_card(card: &Card) -> Result<()> {
    ensure_cards_directory()?;
    let filename = sanitize_filename(&card.name);
    let path = get_cards_path().join(format!("{}.toml", filename));
    let content = toml::to_string_pretty(card)?;
    fs::write(&path, content).context(format!("Failed to write card: {}", card.name))?;
    Ok(())
}

pub fn load_card(name: &str) -> Result<Card> {
    let filename = sanitize_filename(name);
    let path = get_cards_path().join(format!("{}.toml", filename));
    let content = fs::read_to_string(&path).context(format!("Card not found: {}", name))?;
    let card: Card = toml::from_str(&content)?;
    Ok(card)
}

pub fn list_cards() -> Result<Vec<Card>> {
    ensure_cards_directory()?;
    let path = get_cards_path();
    let mut cards = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("toml") {
            let content = fs::read_to_string(&path)?;
            let card: Card = toml::from_str(&content)?;
            cards.push(card);
        }
    }

    Ok(cards)
}

pub fn card_exists(name: &str) -> bool {
    let filename = sanitize_filename(name);
    let path = get_cards_path().join(format!("{}.toml", filename));
    path.exists()
}

pub fn delete_card(name: &str) -> Result<()> {
    let filename = sanitize_filename(name);
    let path = get_cards_path().join(format!("{}.toml", filename));
    fs::remove_file(&path).context(format!("Failed to delete card: {}", name))?;
    Ok(())
}

pub fn sanitize_filename(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' | '-' | '_' => c,
            ' ' => '-',
            _ => '_',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("My Card"), "my-card");
        assert_eq!(sanitize_filename("Test_123"), "test_123");
        assert_eq!(sanitize_filename("Special!@#"), "special___");
    }
}
