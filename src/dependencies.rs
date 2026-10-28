use crate::models::Card;
use crate::storage;
use anyhow::{bail, Result};

/// The status a blocker must reach before the cards it blocks can start.
pub const DONE_STATUS: &str = "done";

fn same_card(a: &str, b: &str) -> bool {
    storage::sanitize_filename(a) == storage::sanitize_filename(b)
}

fn find<'a>(name: &str, cards: &'a [Card]) -> Option<&'a Card> {
    cards.iter().find(|c| same_card(&c.name, name))
}

/// Blockers of `card` that are not yet done. A blocker that no longer
/// exists does not block.
pub fn open_blockers<'a>(card: &'a Card, cards: &[Card]) -> Vec<&'a str> {
    card.blocked_by
        .iter()
        .filter(|b| find(b, cards).is_some_and(|c| c.status != DONE_STATUS))
        .map(|b| b.as_str())
        .collect()
}

pub fn is_blocked(card: &Card, cards: &[Card]) -> bool {
    !open_blockers(card, cards).is_empty()
}

/// Open, unclaimed, not waiting on a human and not blocked: the cards that
/// can be picked up right now.
pub fn is_ready(card: &Card, cards: &[Card]) -> bool {
    card.status != DONE_STATUS
        && card.owner.is_none()
        && !card.needs_human
        && !is_blocked(card, cards)
}

/// Resolve `blocker` to its canonical card name, checking it exists, is not
/// `card_name` itself, and would not create a cycle.
pub fn resolve_blocker(card_name: &str, blocker: &str, cards: &[Card]) -> Result<String> {
    let Some(blocker_card) = find(blocker, cards) else {
        bail!("Blocker card '{}' not found", blocker);
    };
    if same_card(card_name, &blocker_card.name) {
        bail!("Card '{}' cannot block itself", card_name);
    }
    if depends_on(&blocker_card.name, card_name, cards) {
        bail!(
            "'{}' already depends on '{}'; blocking would create a cycle",
            blocker_card.name,
            card_name
        );
    }
    Ok(blocker_card.name.clone())
}

/// Whether `from` transitively depends on `target`.
fn depends_on(from: &str, target: &str, cards: &[Card]) -> bool {
    let mut stack = vec![from.to_string()];
    let mut seen = Vec::new();
    while let Some(name) = stack.pop() {
        if same_card(&name, target) {
            return true;
        }
        let slug = storage::sanitize_filename(&name);
        if seen.contains(&slug) {
            continue;
        }
        seen.push(slug);
        if let Some(card) = find(&name, cards) {
            stack.extend(card.blocked_by.iter().cloned());
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(name: &str, status: &str, blocked_by: &[&str]) -> Card {
        let mut c = Card::new(name.to_string(), String::new());
        c.status = status.to_string();
        c.blocked_by = blocked_by.iter().map(|s| s.to_string()).collect();
        c
    }

    #[test]
    fn test_open_blockers_ignores_done_and_missing() {
        let cards = vec![
            card("a", "todo", &[]),
            card("b", "done", &[]),
            card("c", "todo", &["a", "b", "gone"]),
        ];
        assert_eq!(open_blockers(&cards[2], &cards), vec!["a"]);
    }

    #[test]
    fn test_is_ready_requires_unblocked_unowned_open() {
        let mut cards = vec![card("a", "todo", &[]), card("b", "todo", &["a"])];
        assert!(is_ready(&cards[0], &cards));
        assert!(!is_ready(&cards[1], &cards));
        cards[0].status = "done".to_string();
        assert!(!is_ready(&cards[0], &cards));
        assert!(is_ready(&cards[1], &cards));
        cards[1].owner = Some("x".to_string());
        assert!(!is_ready(&cards[1], &cards));
    }

    #[test]
    fn test_resolve_blocker_canonicalises_name() {
        let cards = vec![card("Big Thing", "todo", &[]), card("c", "todo", &[])];
        assert_eq!(
            resolve_blocker("c", "big thing", &cards).unwrap(),
            "Big Thing"
        );
    }

    #[test]
    fn test_resolve_blocker_rejects_missing_self_and_cycles() {
        let cards = vec![
            card("a", "todo", &[]),
            card("b", "todo", &["a"]),
            card("c", "todo", &["b"]),
        ];
        assert!(resolve_blocker("a", "nope", &cards).is_err());
        assert!(resolve_blocker("a", "a", &cards).is_err());
        assert!(resolve_blocker("a", "c", &cards).is_err());
        assert!(resolve_blocker("c", "a", &cards).is_ok());
    }
}
