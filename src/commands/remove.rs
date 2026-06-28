use crate::storage;
use anyhow::{bail, Result};
use colored::Colorize;

pub fn execute(name: String) -> Result<()> {
    if !storage::card_exists(&name) {
        bail!("Card not found: {}", name);
    }
    storage::delete_card(&name)?;
    println!("{} Deleted card '{}'", "✓".green(), name.bold());
    Ok(())
}
