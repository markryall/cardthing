use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChecklistItem {
    pub text: String,
    #[serde(default)]
    pub checked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Card {
    pub name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checklist: Vec<ChecklistItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_at: Option<DateTime<Utc>>,
}

impl Card {
    pub fn new(name: String, description: String) -> Self {
        let now = Utc::now();
        Self {
            name,
            status: "todo".to_string(),
            owner: None,
            description,
            created_at: now,
            updated_at: now,
            tags: Vec::new(),
            checklist: Vec::new(),
            order: None,
            due_at: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_card_new() {
        let card = Card::new("Test Card".to_string(), "Test description".to_string());
        assert_eq!(card.name, "Test Card");
        assert_eq!(card.description, "Test description");
        assert_eq!(card.status, "todo");
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
        assert_eq!(card.status, "inprogress");
        assert_eq!(card.owner, Some("John".to_string()));
        assert_eq!(card.description, "Test description");
        assert_eq!(card.tags, vec!["tag1", "tag2"]);
    }
}
