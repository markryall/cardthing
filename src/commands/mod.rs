pub mod add;
pub mod edit;
pub mod init;
pub mod list;
pub mod remove;
pub mod serve;
pub mod show;

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};

pub(crate) fn parse_due_date(s: &str) -> Result<DateTime<Utc>> {
    let date = NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .context(format!("Invalid date '{}': expected YYYY-MM-DD", s))?;
    Ok(date.and_hms_opt(0, 0, 0).unwrap().and_utc())
}
