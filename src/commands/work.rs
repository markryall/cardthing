use crate::models::{Card, Config, WorkerProfile};
use crate::storage;
use anyhow::{Context, Result};
use chrono::Utc;
use colored::Colorize;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

const POLL_SECONDS: u64 = 15;

const ADJECTIVES: &[&str] = &[
    "sparkly", "glittery", "sassy", "fierce", "dazzling", "velvet", "cosmic", "peachy", "snazzy",
    "plucky", "breezy", "jazzy", "zesty", "perky", "swanky", "dreamy",
];

const ANIMALS: &[&str] = &[
    "otter", "flamingo", "axolotl", "quokka", "narwhal", "gecko", "lynx", "puffin", "capybara",
    "fennec", "manatee", "ocelot", "pangolin", "toucan", "wombat", "ibis",
];

pub fn execute(profile: String, max_cards: Option<u32>, agent_cmd: Option<String>) -> Result<()> {
    let config = Config::load();
    let worker = config
        .find_worker(&profile)
        .with_context(|| {
            let known: Vec<&str> = config.workers.iter().map(|w| w.name.as_str()).collect();
            if known.is_empty() {
                "No [[workers]] profiles defined in .cards.toml".to_string()
            } else {
                format!(
                    "Unknown worker profile '{}'. Known profiles: {}",
                    profile,
                    known.join(", ")
                )
            }
        })?
        .clone();

    config.validate_status(&worker.watch)?;
    config.validate_status(&worker.done)?;
    if worker.watch == worker.done {
        anyhow::bail!("Worker '{}': watch and done statuses must differ", worker.name);
    }

    let system_prompt = load_system_prompt(&worker)?;
    let name = generate_worker_name();
    let agent = agent_cmd.unwrap_or_else(|| "claude".to_string());

    println!(
        "{} watching '{}' (done -> '{}', clarification -> needs_human flag)",
        format!("[{}]", name).bold().magenta(),
        worker.watch,
        worker.done,
    );

    let mut processed: u32 = 0;
    loop {
        if let Some(card) = next_unallocated(&worker.watch)? {
            if let Some(card) = claim(&card.name, &worker.watch, &name)? {
                log(&name, &format!("claimed '{}'", card.name));
                process_card(&card, &worker, &name, &agent, &system_prompt)?;
                processed += 1;
                if let Some(max) = max_cards {
                    if processed >= max {
                        log(&name, &format!("processed {} card(s), exiting", processed));
                        return Ok(());
                    }
                }
                continue; // look for the next card immediately
            }
        }
        thread::sleep(Duration::from_secs(POLL_SECONDS));
    }
}

fn log(worker_name: &str, message: &str) {
    println!("{} {}", format!("[{}]", worker_name).magenta(), message);
}

fn load_system_prompt(worker: &WorkerProfile) -> Result<String> {
    match (&worker.prompt, &worker.prompt_file) {
        (Some(_), Some(_)) => anyhow::bail!(
            "Worker '{}': set either prompt or prompt_file, not both",
            worker.name
        ),
        (Some(p), None) => Ok(p.clone()),
        (None, Some(f)) => fs::read_to_string(f)
            .with_context(|| format!("Worker '{}': failed to read prompt_file {}", worker.name, f)),
        (None, None) => anyhow::bail!(
            "Worker '{}': one of prompt or prompt_file is required",
            worker.name
        ),
    }
}

pub fn generate_worker_name() -> String {
    let mut seed = std::process::id() as u64 ^ Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
    let mut next = || {
        // xorshift64
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let adjective = ADJECTIVES[(next() % ADJECTIVES.len() as u64) as usize];
    let animal = ANIMALS[(next() % ANIMALS.len() as u64) as usize];
    let suffix = next() % 100;
    format!("{}-{}-{:02}", adjective, animal, suffix)
}

fn next_unallocated(watch: &str) -> Result<Option<Card>> {
    let mut cards: Vec<Card> = storage::list_cards()?
        .into_iter()
        .filter(|c| c.status == watch && c.owner.is_none() && !c.needs_human)
        .collect();
    cards.sort_by_key(|c| c.updated_at);
    Ok(cards.into_iter().next())
}

fn claims_dir() -> PathBuf {
    storage::get_cards_path().join(".claims")
}

fn logs_dir() -> PathBuf {
    storage::get_cards_path().join(".logs")
}

/// Attempt to claim a card by setting its owner, guarded by an exclusive lock
/// file so sibling workers cannot claim the same card. Returns the claimed
/// card, or None if someone else got there first.
fn claim(card_name: &str, watch: &str, worker_name: &str) -> Result<Option<Card>> {
    fs::create_dir_all(claims_dir())?;
    let lock_path = claims_dir().join(format!("{}.lock", storage::sanitize_filename(card_name)));

    // O_EXCL: creation fails if another worker holds the lock
    if fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .is_err()
    {
        return Ok(None);
    }

    let result = (|| -> Result<Option<Card>> {
        let mut card = match storage::load_card(card_name) {
            Ok(c) => c,
            Err(_) => return Ok(None), // vanished between scan and claim
        };
        if card.status != watch || card.owner.is_some() || card.needs_human {
            return Ok(None); // changed under us
        }
        card.owner = Some(worker_name.to_string());
        card.agent = true;
        card.updated_at = Utc::now();
        storage::save_card(&card)?;
        Ok(Some(card))
    })();

    let _ = fs::remove_file(&lock_path);
    result
}

pub fn render_card(card: &Card) -> String {
    let mut out = format!("# Card: {}\n\n", card.name);
    if let Some(ref p) = card.priority {
        out.push_str(&format!("Priority: {}\n", p));
    }
    if let Some(due) = card.due_at {
        out.push_str(&format!("Due: {}\n", due.format("%Y-%m-%d")));
    }
    if !card.tags.is_empty() {
        out.push_str(&format!("Tags: {}\n", card.tags.join(", ")));
    }
    out.push_str(&format!("\n## Description\n\n{}\n", card.description));
    if !card.checklist.is_empty() {
        out.push_str("\n## Checklist\n\n");
        for item in &card.checklist {
            let mark = if item.checked { "x" } else { " " };
            out.push_str(&format!("- [{}] {}\n", mark, item.text));
        }
    }
    out
}

fn task_prompt(card: &Card, worker: &WorkerProfile, worker_name: &str) -> String {
    format!(
        "You are worker '{worker_name}'. Perform the work described by the card below, \
         in the current directory.\n\n\
         When the work is complete, run: cardthing edit \"{name}\" --status {done}\n\
         If you cannot proceed without a human decision, state your questions clearly in \
         your final response and run: cardthing edit \"{name}\" --needs-human\n\
         Do not change the card's owner or status in any other way.\n\n\
         {card}",
        worker_name = worker_name,
        name = card.name,
        done = worker.done,
        card = render_card(card)
    )
}

fn process_card(
    card: &Card,
    worker: &WorkerProfile,
    worker_name: &str,
    agent: &str,
    system_prompt: &str,
) -> Result<()> {
    let mut cmd = Command::new(agent);
    cmd.arg("-p").arg("--system-prompt").arg(system_prompt);
    cmd.arg("--allowed-tools");
    if worker.allowed_tools.is_empty() {
        cmd.arg("Bash(cardthing:*)");
    } else {
        cmd.args(&worker.allowed_tools);
    }
    if let Some(ref model) = worker.model {
        cmd.arg("--model").arg(model);
    }
    if let Some(ref effort) = worker.effort {
        cmd.arg("--effort").arg(effort);
    }
    cmd.arg(task_prompt(card, worker, worker_name));

    log(worker_name, &format!("running agent on '{}'", card.name));
    let output = cmd.output();

    let (agent_text, failure): (String, Option<String>) = match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();
            let mut text = stdout;
            if !stderr.is_empty() {
                text.push_str(&format!("\n\n[stderr]\n{}", stderr));
            }
            if o.status.success() {
                (text, None)
            } else {
                (text, Some(format!("agent exited with {}", o.status)))
            }
        }
        Err(e) => (String::new(), Some(format!("failed to launch agent: {}", e))),
    };

    let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
    fs::create_dir_all(logs_dir())?;
    let log_path = logs_dir().join(format!(
        "{}-{}.md",
        storage::sanitize_filename(&card.name),
        timestamp
    ));
    fs::write(&log_path, &agent_text)?;

    finish_card(
        &card.name,
        worker,
        worker_name,
        &agent_text,
        failure.as_deref(),
    )
}

/// Post-process a card after the agent run: append the agent's output to the
/// description, flag it for human intervention if the agent neither finished
/// nor asked for help, and release the claim.
pub fn finish_card(
    card_name: &str,
    worker: &WorkerProfile,
    worker_name: &str,
    agent_text: &str,
    failure: Option<&str>,
) -> Result<()> {
    let mut card = match storage::load_card(card_name) {
        Ok(c) => c,
        Err(_) => {
            log(worker_name, &format!("'{}' vanished mid-run, moving on", card_name));
            return Ok(());
        }
    };

    let mut section = format!(
        "\n\n## Agent: {} ({})\n\n{}",
        worker_name,
        Utc::now().format("%Y-%m-%d %H:%M UTC"),
        agent_text
    );
    if let Some(reason) = failure {
        section.push_str(&format!("\n\n[worker note: {}]", reason));
    }
    if card.status == worker.watch && !card.needs_human {
        section.push_str(
            "\n\n[worker note: agent neither completed the card nor asked for help, \
             flagged for human intervention]",
        );
        card.needs_human = true;
    }
    card.description.push_str(&section);

    if card.owner.as_deref() == Some(worker_name) {
        card.owner = None;
    }
    card.agent = false;
    card.updated_at = Utc::now();
    storage::save_card(&card)?;
    let outcome = if card.needs_human {
        format!("'{}' needs a human ('{}')", card.name, card.status)
    } else {
        format!("finished '{}' (now '{}')", card.name, card.status)
    };
    log(worker_name, &outcome);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_worker() -> WorkerProfile {
        WorkerProfile {
            name: "test".into(),
            watch: "todo".into(),
            done: "done".into(),
            prompt: Some("do the thing".into()),
            prompt_file: None,
            model: None,
            effort: None,
            allowed_tools: Vec::new(),
        }
    }

    #[test]
    fn test_generate_worker_name_shape() {
        let name = generate_worker_name();
        let parts: Vec<&str> = name.split('-').collect();
        assert_eq!(parts.len(), 3, "expected adjective-animal-NN, got {}", name);
        assert!(ADJECTIVES.contains(&parts[0]));
        assert!(ANIMALS.contains(&parts[1]));
        assert_eq!(parts[2].len(), 2);
        assert!(parts[2].chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_claim_lock_is_exclusive() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("card.lock");

        let winners: Vec<bool> = std::thread::scope(|s| {
            (0..8)
                .map(|_| {
                    let path = lock_path.clone();
                    s.spawn(move || {
                        fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(path)
                            .is_ok()
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|h| h.join().unwrap())
                .collect()
        });

        assert_eq!(
            winners.iter().filter(|&&w| w).count(),
            1,
            "exactly one thread must win the claim lock"
        );
    }

    #[test]
    fn test_render_card_includes_fields() {
        let mut card = Card::new("Test".into(), "Do stuff".into());
        card.tags = vec!["a".into(), "b".into()];
        card.priority = Some("high".into());
        card.checklist.push(crate::models::ChecklistItem {
            text: "step one".into(),
            checked: true,
        });
        let rendered = render_card(&card);
        assert!(rendered.contains("# Card: Test"));
        assert!(rendered.contains("Do stuff"));
        assert!(rendered.contains("Priority: high"));
        assert!(rendered.contains("Tags: a, b"));
        assert!(rendered.contains("- [x] step one"));
    }

    #[test]
    fn test_task_prompt_contains_instructions() {
        let card = Card::new("My Card".into(), "Details".into());
        let prompt = task_prompt(&card, &test_worker(), "sparkly-otter-42");
        assert!(prompt.contains("cardthing edit \"My Card\" --status done"));
        assert!(prompt.contains("cardthing edit \"My Card\" --needs-human"));
        assert!(prompt.contains("sparkly-otter-42"));
        assert!(prompt.contains("Details"));
    }
}
