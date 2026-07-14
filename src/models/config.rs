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
pub struct WorkerProfile {
    pub name: String,
    pub watch: String,
    pub done: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// Tools the agent may use without prompting (claude --allowed-tools).
    /// Defaults to just enough to move the card: Bash(cardthing:*)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_tools: Vec<String>,
    /// Fallback poll interval in seconds, used in case filesystem change
    /// notifications are missed or unavailable. Defaults to 15.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll_seconds: Option<u64>,
    /// When true (and a .jj directory is present), 'cardthing work' isolates
    /// each agent run in its own jj workspace instead of sharing the main
    /// working-copy commit, so concurrent workers don't squash each other's
    /// changes together. Defaults to false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub workspace: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_title")]
    pub title: String,
    #[serde(default = "default_statuses")]
    pub statuses: Vec<StatusDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workers: Vec<WorkerProfile>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            title: default_title(),
            statuses: default_statuses(),
            workers: Vec::new(),
        }
    }
}

fn default_title() -> String {
    "Board".to_string()
}

impl Config {
    pub fn load() -> Self {
        fs::read_to_string(".cards.toml")
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| anyhow::anyhow!("Failed to serialise config: {}", e))?;
        fs::write(".cards.toml", content)?;
        Ok(())
    }

    pub fn find_status(&self, id: &str) -> Option<&StatusDef> {
        self.statuses.iter().find(|s| s.id.eq_ignore_ascii_case(id))
    }

    pub fn find_worker(&self, name: &str) -> Option<&WorkerProfile> {
        self.workers
            .iter()
            .find(|w| w.name.eq_ignore_ascii_case(name))
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
        StatusDef {
            id: "todo".into(),
            label: "Todo".into(),
            color: "#f59e0b".into(),
        },
        StatusDef {
            id: "inprogress".into(),
            label: "In Progress".into(),
            color: "#3b82f6".into(),
        },
        StatusDef {
            id: "done".into(),
            label: "Done".into(),
            color: "#10b981".into(),
        },
        StatusDef {
            id: "blocked".into(),
            label: "Blocked".into(),
            color: "#ef4444".into(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_profile_poll_seconds_defaults_to_none() {
        let toml_str = r#"
            name = "test"
            watch = "todo"
            done = "done"
        "#;
        let worker: WorkerProfile = toml::from_str(toml_str).unwrap();
        assert_eq!(worker.poll_seconds, None);
    }

    #[test]
    fn test_worker_profile_poll_seconds_parses_when_set() {
        let toml_str = r#"
            name = "test"
            watch = "todo"
            done = "done"
            poll_seconds = 5
        "#;
        let worker: WorkerProfile = toml::from_str(toml_str).unwrap();
        assert_eq!(worker.poll_seconds, Some(5));
    }

    #[test]
    fn test_worker_profile_workspace_defaults_to_false() {
        let toml_str = r#"
            name = "test"
            watch = "todo"
            done = "done"
        "#;
        let worker: WorkerProfile = toml::from_str(toml_str).unwrap();
        assert!(!worker.workspace);
    }

    #[test]
    fn test_worker_profile_workspace_parses_when_set() {
        let toml_str = r#"
            name = "test"
            watch = "todo"
            done = "done"
            workspace = true
        "#;
        let worker: WorkerProfile = toml::from_str(toml_str).unwrap();
        assert!(worker.workspace);
    }
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
