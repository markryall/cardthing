pub mod add;
pub mod checklist;
pub mod edit;
pub mod init;
pub mod list;
pub mod remove;
pub mod serve;
pub mod show;
pub mod stats;

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};

pub(crate) fn parse_due_date(s: &str) -> Result<DateTime<Utc>> {
    let date = NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .context(format!("Invalid date '{}': expected YYYY-MM-DD", s))?;
    Ok(date.and_hms_opt(0, 0, 0).unwrap().and_utc())
}

const VALID_PRIORITIES: &[&str] = &["high", "medium", "low"];

pub(crate) fn validate_priority(p: &str) -> Result<String> {
    let lower = p.to_lowercase();
    if VALID_PRIORITIES.contains(&lower.as_str()) {
        Ok(lower)
    } else {
        anyhow::bail!(
            "Invalid priority: '{}'. Valid values are: {}",
            p,
            VALID_PRIORITIES.join(", ")
        )
    }
}
