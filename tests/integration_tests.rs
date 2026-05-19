use cardthing::models::Card;
use cardthing::storage;
use std::env;
use std::sync::Mutex;
use tempfile::TempDir;

static TEST_MUTEX: Mutex<()> = Mutex::new(());

fn with_temp_cards_dir<F>(test: F)
where
    F: FnOnce() + std::panic::UnwindSafe,
{
    let _guard = TEST_MUTEX.lock().unwrap();

    let original_dir = env::current_dir().unwrap();
    let temp_dir = TempDir::new().unwrap();
    env::set_current_dir(&temp_dir).unwrap();

    let result = std::panic::catch_unwind(test);

    env::set_current_dir(original_dir).unwrap();
    drop(temp_dir);

    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

#[test]
fn test_add_card() {
    with_temp_cards_dir(|| {
        let card = Card::new("Test Card".to_string(), "Test description".to_string());
        storage::save_card(&card).unwrap();

        assert!(storage::card_exists("Test Card"));

        let loaded = storage::load_card("Test Card").unwrap();
        assert_eq!(loaded.name, "Test Card");
        assert_eq!(loaded.description, "Test description");
        assert_eq!(loaded.status, "todo");
    });
}

#[test]
fn test_add_card_with_owner_and_tags() {
    with_temp_cards_dir(|| {
        let mut card = Card::new("Feature X".to_string(), "Implement feature X".to_string());
        card.status = "inprogress".to_string();
        card.owner = Some("Alice".to_string());
        card.tags = vec!["feature".to_string(), "high-priority".to_string()];

        storage::save_card(&card).unwrap();

        let loaded = storage::load_card("Feature X").unwrap();
        assert_eq!(loaded.owner, Some("Alice".to_string()));
        assert_eq!(loaded.status, "inprogress");
        assert_eq!(loaded.tags, vec!["feature", "high-priority"]);
    });
}

#[test]
fn test_list_cards() {
    with_temp_cards_dir(|| {
        let card1 = Card::new("Card 1".to_string(), "First card".to_string());
        storage::save_card(&card1).unwrap();

        let mut card2 = Card::new("Card 2".to_string(), "Second card".to_string());
        card2.status = "inprogress".to_string();
        card2.owner = Some("Alice".to_string());
        storage::save_card(&card2).unwrap();

        let mut card3 = Card::new("Card 3".to_string(), "Third card".to_string());
        card3.status = "done".to_string();
        storage::save_card(&card3).unwrap();

        let cards = storage::list_cards().unwrap();
        assert_eq!(cards.len(), 3);
    });
}

#[test]
fn test_storage_card_exists() {
    with_temp_cards_dir(|| {
        assert!(!storage::card_exists("Nonexistent"));

        let card = Card::new("Exists".to_string(), "Description".to_string());
        storage::save_card(&card).unwrap();

        assert!(storage::card_exists("Exists"));
    });
}

#[test]
fn test_storage_delete_card() {
    with_temp_cards_dir(|| {
        let card = Card::new("Delete Me".to_string(), "To be deleted".to_string());
        storage::save_card(&card).unwrap();

        assert!(storage::card_exists("Delete Me"));

        storage::delete_card("Delete Me").unwrap();
        assert!(!storage::card_exists("Delete Me"));
    });
}

#[test]
fn test_card_persistence() {
    with_temp_cards_dir(|| {
        let mut card = Card::new("Persist".to_string(), "Test persistence".to_string());
        card.status = "inprogress".to_string();
        card.owner = Some("Bob".to_string());
        card.tags = vec!["tag1".to_string(), "tag2".to_string()];

        storage::save_card(&card).unwrap();

        let loaded = storage::load_card("Persist").unwrap();
        assert_eq!(loaded.name, card.name);
        assert_eq!(loaded.status, card.status);
        assert_eq!(loaded.owner, card.owner);
        assert_eq!(loaded.description, card.description);
        assert_eq!(loaded.tags, card.tags);
    });
}

#[test]
fn test_load_nonexistent_card() {
    with_temp_cards_dir(|| {
        let result = storage::load_card("Nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Card not found"));
    });
}

#[test]
fn test_delete_nonexistent_card() {
    with_temp_cards_dir(|| {
        let result = storage::delete_card("Nonexistent");
        assert!(result.is_err());
    });
}

#[test]
fn test_filename_sanitization() {
    with_temp_cards_dir(|| {
        let card1 = Card::new("My Special Card!".to_string(), "Description".to_string());
        storage::save_card(&card1).unwrap();
        assert!(storage::card_exists("My Special Card!"));

        let card2 = Card::new("Card With Spaces".to_string(), "Description".to_string());
        storage::save_card(&card2).unwrap();
        assert!(storage::card_exists("Card With Spaces"));

        let cards = storage::list_cards().unwrap();
        assert_eq!(cards.len(), 2);
    });
}

#[test]
fn test_multiple_cards_same_prefix() {
    with_temp_cards_dir(|| {
        storage::save_card(&Card::new("Task".to_string(), "First task".to_string())).unwrap();
        storage::save_card(&Card::new("Task-2".to_string(), "Second task".to_string())).unwrap();
        storage::save_card(&Card::new("Task_3".to_string(), "Third task".to_string())).unwrap();

        assert!(storage::card_exists("Task"));
        assert!(storage::card_exists("Task-2"));
        assert!(storage::card_exists("Task_3"));

        let cards = storage::list_cards().unwrap();
        assert_eq!(cards.len(), 3);
    });
}

#[test]
fn test_card_update() {
    with_temp_cards_dir(|| {
        let mut card = Card::new("Update Test".to_string(), "Original".to_string());
        storage::save_card(&card).unwrap();

        let created_at = card.created_at;

        std::thread::sleep(std::time::Duration::from_millis(10));

        card.description = "Updated".to_string();
        card.status = "done".to_string();
        card.updated_at = chrono::Utc::now();
        storage::save_card(&card).unwrap();

        let loaded = storage::load_card("Update Test").unwrap();
        assert_eq!(loaded.description, "Updated");
        assert_eq!(loaded.status, "done");
        assert_eq!(loaded.created_at, created_at);
        assert!(loaded.updated_at > created_at);
    });
}

#[test]
fn test_empty_cards_directory() {
    with_temp_cards_dir(|| {
        let cards = storage::list_cards().unwrap();
        assert_eq!(cards.len(), 0);
    });
}

#[test]
fn test_all_status_values() {
    with_temp_cards_dir(|| {
        for status in &["todo", "inprogress", "done", "blocked"] {
            let mut card = Card::new(status.to_string(), format!("{} card", status));
            card.status = status.to_string();
            storage::save_card(&card).unwrap();
        }

        let cards = storage::list_cards().unwrap();
        assert_eq!(cards.len(), 4);

        let statuses: Vec<&str> = cards.iter().map(|c| c.status.as_str()).collect();
        assert!(statuses.contains(&"todo"));
        assert!(statuses.contains(&"inprogress"));
        assert!(statuses.contains(&"done"));
        assert!(statuses.contains(&"blocked"));
    });
}
