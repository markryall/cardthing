use anyhow::{bail, Result};
use colored::Colorize;

pub fn execute() -> Result<()> {
    match crate::snapshot::refresh()? {
        Some(path) => {
            println!("{} Wrote {}", "✓".green(), path.display());
            Ok(())
        }
        None => bail!("No snapshot configured. Add snapshot = \".cards/README.md\" to .cards.toml"),
    }
}
