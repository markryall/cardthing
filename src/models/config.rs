use colored::Color;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusDef {
    pub id: String,
    pub label: String,
    pub color: String,
}

impl StatusDef {
    pub fn terminal_color(&self) -> Color {
        parse_hex_color(&self.color).unwrap_or(Color::White)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_statuses")]
    pub statuses: Vec<StatusDef>,
}

impl Default for Config {
    fn default() -> Self {
        Config { statuses: default_statuses() }
    }
}

impl Config {
    pub fn load() -> Self {
        fs::read_to_string(".cardthing.toml")
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn find_status(&self, id: &str) -> Option<&StatusDef> {
        self.statuses.iter().find(|s| s.id.eq_ignore_ascii_case(id))
    }

    pub fn validate_status(&self, id: &str) -> anyhow::Result<String> {
        match self.find_status(id) {
            Some(s) => Ok(s.id.clone()),
            None => {
                let valid: Vec<&str> = self.statuses.iter().map(|s| s.id.as_str()).collect();
                anyhow::bail!(
                    "Invalid status: '{}'. Valid values are: {}",
                    id,
                    valid.join(", ")
                )
            }
        }
    }
}

fn default_statuses() -> Vec<StatusDef> {
    vec![
        StatusDef { id: "todo".into(),       label: "Todo".into(),        color: "#f59e0b".into() },
        StatusDef { id: "inprogress".into(), label: "In Progress".into(), color: "#3b82f6".into() },
        StatusDef { id: "done".into(),       label: "Done".into(),        color: "#10b981".into() },
        StatusDef { id: "blocked".into(),    label: "Blocked".into(),     color: "#ef4444".into() },
    ]
}

fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::TrueColor { r, g, b })
}
