use chrono::{DateTime, Utc};
use colored::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Card {
    pub name: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Todo,
    InProgress,
    Done,
    Blocked,
}

impl Card {
    pub fn new(name: String, description: String) -> Self {
        let now = Utc::now();
        Self {
            name,
            status: Status::Todo,
            owner: None,
            description,
            created_at: now,
            updated_at: now,
            tags: Vec::new(),
        }
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.name.trim().is_empty() {
            anyhow::bail!("Card name cannot be empty");
        }
        if self.name.contains('/') || self.name.contains('\\') {
            anyhow::bail!("Card name cannot contain path separators");
        }
        Ok(())
    }
}

impl Status {
    pub fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().as_str() {
            "todo" => Ok(Status::Todo),
            "inprogress" | "in-progress" | "in_progress" => Ok(Status::InProgress),
            "done" => Ok(Status::Done),
            "blocked" => Ok(Status::Blocked),
            _ => anyhow::bail!("Invalid status: '{}'. Valid values are: todo, inprogress, done, blocked", s),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Todo => "todo",
            Status::InProgress => "inprogress",
            Status::Done => "done",
            Status::Blocked => "blocked",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Status::Todo => Color::Yellow,
            Status::InProgress => Color::Blue,
            Status::Done => Color::Green,
            Status::Blocked => Color::Red,
        }
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_card_new() {
        let card = Card::new("Test Card".to_string(), "Test description".to_string());
        assert_eq!(card.name, "Test Card");
        assert_eq!(card.description, "Test description");
        assert_eq!(card.status, Status::Todo);
        assert_eq!(card.owner, None);
        assert!(card.tags.is_empty());
    }

    #[test]
    fn test_card_validate_valid() {
        let card = Card::new("Valid Card".to_string(), "Description".to_string());
        assert!(card.validate().is_ok());
    }

    #[test]
    fn test_card_validate_empty_name() {
        let card = Card::new("   ".to_string(), "Description".to_string());
        assert!(card.validate().is_err());
    }

    #[test]
    fn test_card_validate_path_separators() {
        let card1 = Card::new("Card/Name".to_string(), "Description".to_string());
        assert!(card1.validate().is_err());

        let card2 = Card::new("Card\\Name".to_string(), "Description".to_string());
        assert!(card2.validate().is_err());
    }

    #[test]
    fn test_status_from_str() {
        assert_eq!(Status::from_str("todo").unwrap(), Status::Todo);
        assert_eq!(Status::from_str("TODO").unwrap(), Status::Todo);
        assert_eq!(Status::from_str("inprogress").unwrap(), Status::InProgress);
        assert_eq!(Status::from_str("in-progress").unwrap(), Status::InProgress);
        assert_eq!(Status::from_str("in_progress").unwrap(), Status::InProgress);
        assert_eq!(Status::from_str("done").unwrap(), Status::Done);
        assert_eq!(Status::from_str("blocked").unwrap(), Status::Blocked);
        assert!(Status::from_str("invalid").is_err());
    }

    #[test]
    fn test_status_as_str() {
        assert_eq!(Status::Todo.as_str(), "todo");
        assert_eq!(Status::InProgress.as_str(), "inprogress");
        assert_eq!(Status::Done.as_str(), "done");
        assert_eq!(Status::Blocked.as_str(), "blocked");
    }

    #[test]
    fn test_status_display() {
        assert_eq!(format!("{}", Status::Todo), "todo");
        assert_eq!(format!("{}", Status::InProgress), "inprogress");
        assert_eq!(format!("{}", Status::Done), "done");
        assert_eq!(format!("{}", Status::Blocked), "blocked");
    }

    #[test]
    fn test_status_color() {
        assert_eq!(Status::Todo.color(), Color::Yellow);
        assert_eq!(Status::InProgress.color(), Color::Blue);
        assert_eq!(Status::Done.color(), Color::Green);
        assert_eq!(Status::Blocked.color(), Color::Red);
    }

    #[test]
    fn test_card_serialization() {
        let card = Card::new("Test".to_string(), "Description".to_string());
        let serialized = toml::to_string(&card).unwrap();
        assert!(serialized.contains("name = \"Test\""));
        assert!(serialized.contains("status = \"todo\""));
        assert!(serialized.contains("description = \"Description\""));
    }

    #[test]
    fn test_card_deserialization() {
        let toml_str = r#"
            name = "Test Card"
            status = "inprogress"
            owner = "John"
            description = "Test description"
            created_at = "2025-01-01T00:00:00Z"
            updated_at = "2025-01-01T00:00:00Z"
            tags = ["tag1", "tag2"]
        "#;
        let card: Card = toml::from_str(toml_str).unwrap();
        assert_eq!(card.name, "Test Card");
        assert_eq!(card.status, Status::InProgress);
        assert_eq!(card.owner, Some("John".to_string()));
        assert_eq!(card.description, "Test description");
        assert_eq!(card.tags, vec!["tag1", "tag2"]);
    }
}
