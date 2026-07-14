use crate::models::{Card, Config, WorkerProfile};
use crate::storage;
use anyhow::{Context, Result};
use chrono::Utc;
use colored::Colorize;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const POLL_SECONDS: u64 = 15;

/// How often we poll a running agent child process for exit / kill requests.
const CHILD_POLL_MILLIS: u64 = 150;

const ADJECTIVES: &[&str] = &[
    "sparkly", "glittery", "sassy", "fierce", "dazzling", "velvet", "cosmic", "peachy", "snazzy",
    "plucky", "breezy", "jazzy", "zesty", "perky", "swanky", "dreamy",
];

const ANIMALS: &[&str] = &[
    "otter", "flamingo", "axolotl", "quokka", "narwhal", "gecko", "lynx", "puffin", "capybara",
    "fennec", "manatee", "ocelot", "pangolin", "toucan", "wombat", "ibis",
];

#[allow(clippy::too_many_arguments)]
pub fn execute(
    profile: String,
    max_cards: Option<u32>,
    watch: Option<String>,
    done: Option<String>,
    prompt_file: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    agent_cmd: Option<String>,
) -> Result<()> {
    let config = Config::load();
    let mut worker = config
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

    apply_overrides(&mut worker, watch, done, prompt_file, model, effort);

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

    let poll_seconds = worker.poll_seconds.unwrap_or(POLL_SECONDS);
    let (_watcher, watch_rx) = spawn_change_watcher();

    // sigint_count tracks how many Ctrl-C presses we've seen:
    //   0 = normal operation
    //   1 = graceful shutdown requested (finish current card, then stop)
    //   2+ = force shutdown requested (kill the running agent, if any)
    // in_progress tracks whether a card is currently being worked on; if
    // Ctrl-C arrives while idle we exit immediately.
    let sigint_count = Arc::new(AtomicUsize::new(0));
    let in_progress = Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let sigint_count = sigint_count.clone();
        let in_progress = in_progress.clone();
        let handler_name = name.clone();
        ctrlc::set_handler(move || {
            let count = sigint_count.fetch_add(1, Ordering::SeqCst) + 1;
            if !in_progress.load(Ordering::SeqCst) {
                log(&handler_name, "idle, exiting immediately");
                std::process::exit(0);
            }
            if count == 1 {
                log(
                    &handler_name,
                    "finishing current card, then exiting (press Ctrl-C again to force stop)",
                );
            } else {
                log(&handler_name, "force stop requested, killing agent process");
            }
        })
        .context("failed to install Ctrl-C handler")?;
    }

    let mut processed: u32 = 0;
    loop {
        if let Some(card) = next_unallocated(&worker.watch)? {
            if let Some(card) = claim(&card.name, &worker.watch, &name)? {
                log(&name, &format!("claimed '{}'", card.name));
                in_progress.store(true, Ordering::SeqCst);
                process_card(&card, &worker, &name, &agent, &system_prompt, &sigint_count)?;
                in_progress.store(false, Ordering::SeqCst);
                processed += 1;
                if sigint_count.load(Ordering::SeqCst) >= 1 {
                    log(&name, "shutting down after Ctrl-C");
                    return Ok(());
                }
                if let Some(max) = max_cards {
                    if processed >= max {
                        log(&name, &format!("processed {} card(s), exiting", processed));
                        return Ok(());
                    }
                }
                continue; // look for the next card immediately
            }
        }
        // Wake early on a .cards/ change; otherwise fall back to polling.
        let _ = watch_rx.recv_timeout(Duration::from_secs(poll_seconds));
    }
}

/// Watch the .cards/ directory for changes, debouncing bursts of events into
/// a single notification. Returns the watcher (which must be kept alive for
/// as long as watching is desired) and a receiver that is notified on
/// change. If the watcher cannot be set up, the receiver simply never fires
/// and callers fall back to polling.
fn spawn_change_watcher() -> (Option<RecommendedWatcher>, mpsc::Receiver<()>) {
    let (tx, rx) = mpsc::channel::<()>();

    let (stx, srx) = mpsc::channel::<notify::Result<notify::Event>>();
    let watcher = match RecommendedWatcher::new(stx, notify::Config::default()) {
        Ok(w) => w,
        Err(_) => return (None, rx),
    };

    let mut watcher = watcher;
    if watcher
        .watch(Path::new(".cards"), RecursiveMode::NonRecursive)
        .is_err()
    {
        return (None, rx);
    }

    thread::spawn(move || {
        while let Ok(Ok(_)) = srx.recv() {
            thread::sleep(Duration::from_millis(50));
            while srx.try_recv().is_ok() {}
            let _ = tx.send(());
        }
    });

    (Some(watcher), rx)
}

/// Apply CLI-provided per-field overrides on top of a worker profile loaded
/// from .cards.toml. Setting prompt_file also clears prompt so the two
/// remain mutually exclusive (load_system_prompt rejects both being set).
fn apply_overrides(
    worker: &mut WorkerProfile,
    watch: Option<String>,
    done: Option<String>,
    prompt_file: Option<String>,
    model: Option<String>,
    effort: Option<String>,
) {
    if let Some(watch) = watch {
        worker.watch = watch;
    }
    if let Some(done) = done {
        worker.done = done;
    }
    if let Some(prompt_file) = prompt_file {
        worker.prompt = None;
        worker.prompt_file = Some(prompt_file);
    }
    if let Some(model) = model {
        worker.model = Some(model);
    }
    if let Some(effort) = effort {
        worker.effort = Some(effort);
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
    sort_by_board_order(&mut cards);
    Ok(cards.into_iter().next())
}

/// Sort cards the same way the web board displays a column (order field
/// ascending, unordered cards last, created_at as tiebreaker) so the top
/// card on the board is the next one picked up.
fn sort_by_board_order(cards: &mut [Card]) {
    cards.sort_by(|a, b| {
        let ao = a.order.unwrap_or(u32::MAX);
        let bo = b.order.unwrap_or(u32::MAX);
        ao.cmp(&bo).then_with(|| a.created_at.cmp(&b.created_at))
    });
}

fn claims_dir() -> PathBuf {
    storage::get_cards_path().join(".claims")
}

fn logs_dir() -> PathBuf {
    storage::get_cards_path().join(".logs")
}

/// A lock file untouched for this long is assumed to be left over from a
/// worker that crashed mid-claim rather than one still legitimately held.
const STALE_LOCK_SECS: u64 = 60;

/// Try to exclusively create `lock_path`, stamping it with the current unix
/// time so staleness can later be judged from its contents. Returns whether
/// the lock was acquired.
fn acquire_lock(lock_path: &Path) -> bool {
    // O_EXCL: creation fails if another worker holds the lock
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock_path)
    {
        Ok(mut f) => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let _ = f.write_all(now.to_string().as_bytes());
            true
        }
        Err(_) => false,
    }
}

/// If `lock_path` holds a timestamp older than [`STALE_LOCK_SECS`], treat it
/// as left over from a crashed worker and delete it. Returns true when the
/// caller should retry acquiring the lock (either it was stolen, or it had
/// already vanished on its own).
fn steal_stale_lock(lock_path: &Path) -> bool {
    let contents = match fs::read_to_string(lock_path) {
        Ok(c) => c,
        Err(_) => return true, // vanished already; a retry will just recreate it
    };
    let created: u64 = match contents.trim().parse() {
        Ok(v) => v,
        Err(_) => return false, // unreadable timestamp; leave a lock we don't understand alone
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if now.saturating_sub(created) >= STALE_LOCK_SECS {
        fs::remove_file(lock_path).is_ok()
    } else {
        false
    }
}

/// Attempt to claim a card by setting its owner, guarded by an exclusive lock
/// file so sibling workers cannot claim the same card. Returns the claimed
/// card, or None if someone else got there first.
fn claim(card_name: &str, watch: &str, worker_name: &str) -> Result<Option<Card>> {
    fs::create_dir_all(claims_dir())?;
    let lock_path = claims_dir().join(format!("{}.lock", storage::sanitize_filename(card_name)));

    if !acquire_lock(&lock_path) {
        // Someone else holds the lock. If it's stale -- left over from a
        // crashed worker -- steal it and retry once.
        if !steal_stale_lock(&lock_path) || !acquire_lock(&lock_path) {
            return Ok(None);
        }
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

fn task_prompt(
    card: &Card,
    worker: &WorkerProfile,
    worker_name: &str,
    workspace: Option<&Path>,
    repo_root: &Path,
) -> String {
    let mut prompt = format!(
        "You are worker '{worker_name}'. Perform the work described by the card below, \
         in the current directory.\n\n\
         When the work is complete, run: cardthing edit \"{name}\" --status {done}\n\
         If you cannot proceed without a human decision, state your questions clearly in \
         your final response and run: cardthing edit \"{name}\" --needs-human\n\
         Do not change the card's owner or status in any other way.\n\n",
        worker_name = worker_name,
        name = card.name,
        done = worker.done,
    );
    if let Some(ws) = workspace {
        prompt.push_str(&format!(
            "Your working copy is the isolated jj workspace at {ws}. Make all code changes \
             there. However, all `cardthing` card commands (edit/show/etc., including the \
             ones above) must be run from the main repository directory at {repo_root} \
             instead, so the live board is updated rather than a stale copy in the \
             workspace.\n\n",
            ws = ws.display(),
            repo_root = repo_root.display(),
        ));
    }
    prompt.push_str(&render_card(card));
    prompt
}

/// True if the current directory is (or is inside) a jj repository.
fn jj_repo_present() -> bool {
    Path::new(".jj").is_dir()
}

/// Sibling path a worker's isolated jj workspace is created at, e.g.
/// ../cardthing-ws-sparkly-otter-42 next to the main repo checkout.
fn workspace_sibling_path(repo_root: &Path, worker_name: &str) -> PathBuf {
    let repo_name = repo_root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("repo");
    let dir_name = format!("{}-ws-{}", repo_name, worker_name);
    match repo_root.parent() {
        Some(parent) => parent.join(dir_name),
        None => PathBuf::from(format!("../{}", dir_name)),
    }
}

/// Create a jj workspace for a worker, returning its path. The workspace is
/// created as a sibling directory of the main repo checkout.
fn create_workspace(repo_root: &Path, worker_name: &str) -> Result<PathBuf> {
    let path = workspace_sibling_path(repo_root, worker_name);
    let status = Command::new("jj")
        .arg("workspace")
        .arg("add")
        .arg("--name")
        .arg(worker_name)
        .arg(&path)
        .current_dir(repo_root)
        .status()
        .context("failed to run 'jj workspace add'")?;
    if !status.success() {
        anyhow::bail!("'jj workspace add' failed for worker '{}'", worker_name);
    }
    Ok(path)
}

/// True if the workspace's working-copy commit has any changes relative to
/// its parent (i.e. the agent actually did something in it).
fn workspace_has_changes(workspace_path: &Path) -> Result<bool> {
    let output = Command::new("jj")
        .arg("log")
        .arg("--no-graph")
        .arg("-r")
        .arg("@")
        .arg("-T")
        .arg(r#"if(empty, "empty", "changed")"#)
        .current_dir(workspace_path)
        .output()
        .context("failed to run 'jj log'")?;
    if !output.status.success() {
        anyhow::bail!(
            "'jj log' failed to check for changes in workspace {}",
            workspace_path.display()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim() != "empty")
}

/// Forget (and remove) a worker's jj workspace, e.g. because it ended up
/// with no changes.
fn forget_workspace(repo_root: &Path, worker_name: &str, workspace_path: &Path) -> Result<()> {
    let status = Command::new("jj")
        .arg("workspace")
        .arg("forget")
        .arg(worker_name)
        .current_dir(repo_root)
        .status()
        .context("failed to run 'jj workspace forget'")?;
    if !status.success() {
        anyhow::bail!("'jj workspace forget' failed for worker '{}'", worker_name);
    }
    let _ = fs::remove_dir_all(workspace_path);
    Ok(())
}

/// After an agent run in an isolated workspace: if it made no changes, clean
/// it up automatically; otherwise leave it in place and note its path on the
/// card so a human can review, merge/squash, and forget it.
fn reconcile_workspace(
    repo_root: &Path,
    card_name: &str,
    worker_name: &str,
    workspace_path: &Path,
) {
    let has_changes = match workspace_has_changes(workspace_path) {
        Ok(v) => v,
        Err(e) => {
            log(
                worker_name,
                &format!(
                    "could not check workspace '{}' for changes ({}), leaving it for review",
                    worker_name, e
                ),
            );
            true
        }
    };

    if has_changes {
        if let Ok(mut card) = storage::load_card(card_name) {
            card.description.push_str(&format!(
                "\n\n[worker note: changes are in jj workspace '{}' at {} — review, \
                 merge/squash them, then run `jj workspace forget {}`]",
                worker_name,
                workspace_path.display(),
                worker_name
            ));
            card.updated_at = Utc::now();
            let _ = storage::save_card(&card);
        }
        log(
            worker_name,
            &format!(
                "workspace '{}' has changes, left for review at {}",
                worker_name,
                workspace_path.display()
            ),
        );
    } else {
        match forget_workspace(repo_root, worker_name, workspace_path) {
            Ok(()) => log(
                worker_name,
                &format!("workspace '{}' had no changes, forgotten", worker_name),
            ),
            Err(e) => log(
                worker_name,
                &format!("failed to forget empty workspace '{}': {}", worker_name, e),
            ),
        }
    }
}

fn process_card(
    card: &Card,
    worker: &WorkerProfile,
    worker_name: &str,
    agent: &str,
    system_prompt: &str,
    sigint_count: &AtomicUsize,
) -> Result<()> {
    let repo_root = std::env::current_dir().context("failed to get current directory")?;

    let workspace_path: Option<PathBuf> = if worker.workspace && jj_repo_present() {
        match create_workspace(&repo_root, worker_name) {
            Ok(path) => {
                log(
                    worker_name,
                    &format!("created jj workspace at {}", path.display()),
                );
                Some(path)
            }
            Err(e) => {
                let msg = format!("failed to create jj workspace: {}", e);
                log(worker_name, &msg);
                return finish_card(&card.name, worker, worker_name, "", Some(&msg));
            }
        }
    } else {
        None
    };

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
    cmd.arg(task_prompt(
        card,
        worker,
        worker_name,
        workspace_path.as_deref(),
        &repo_root,
    ));
    if let Some(ref ws) = workspace_path {
        cmd.current_dir(ws);
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    log(worker_name, &format!("running agent on '{}'", card.name));

    let (agent_text, failure): (String, Option<String>) = match cmd.spawn() {
        Ok(mut child) => {
            let stdout_pipe = child.stdout.take();
            let stderr_pipe = child.stderr.take();
            let stdout_handle = thread::spawn(move || {
                let mut buf = String::new();
                if let Some(mut pipe) = stdout_pipe {
                    let _ = pipe.read_to_string(&mut buf);
                }
                buf
            });
            let stderr_handle = thread::spawn(move || {
                let mut buf = String::new();
                if let Some(mut pipe) = stderr_pipe {
                    let _ = pipe.read_to_string(&mut buf);
                }
                buf
            });

            // Poll rather than block so a second Ctrl-C can kill the child
            // instead of waiting for it to finish on its own.
            let mut killed = false;
            let status = loop {
                match child.try_wait() {
                    Ok(Some(status)) => break Some(status),
                    Ok(None) => {
                        if !killed && sigint_count.load(Ordering::SeqCst) >= 2 {
                            log(worker_name, "force-stopping agent process");
                            let _ = child.kill();
                            killed = true;
                        }
                        thread::sleep(Duration::from_millis(CHILD_POLL_MILLIS));
                    }
                    Err(_) => break None,
                }
            };

            let stdout = stdout_handle.join().unwrap_or_default();
            let stderr = stderr_handle.join().unwrap_or_default();
            let mut text = stdout.trim().to_string();
            if !stderr.trim().is_empty() {
                text.push_str(&format!("\n\n[stderr]\n{}", stderr.trim()));
            }

            if killed {
                (
                    text,
                    Some("cancelled: worker force-stopped after a second Ctrl-C".to_string()),
                )
            } else {
                match status {
                    Some(s) if s.success() => (text, None),
                    Some(s) => (text, Some(format!("agent exited with {}", s))),
                    None => (text, Some("failed to wait for agent process".to_string())),
                }
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
    )?;

    if let Some(ws) = workspace_path {
        reconcile_workspace(&repo_root, &card.name, worker_name, &ws);
    }

    Ok(())
}

/// Maximum length (in characters) of agent output appended to a card's
/// description before it gets truncated in favour of a pointer to the full
/// log file under `.cards/.logs/`.
const MAX_AGENT_TEXT_CHARS: usize = 4000;

/// Truncate `agent_text` to roughly `MAX_AGENT_TEXT_CHARS` characters, noting
/// that the full output is available in the `.cards/.logs/` directory.
fn truncate_agent_text(agent_text: &str) -> String {
    if agent_text.chars().count() <= MAX_AGENT_TEXT_CHARS {
        return agent_text.to_string();
    }
    let truncated: String = agent_text.chars().take(MAX_AGENT_TEXT_CHARS).collect();
    format!(
        "{}\n\n[worker note: output truncated; full output saved in .cards/.logs/]",
        truncated
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
        truncate_agent_text(agent_text)
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
            poll_seconds: None,
            workspace: false,
        }
    }

    #[test]
    fn test_apply_overrides_leaves_profile_untouched_when_all_none() {
        let mut worker = test_worker();
        let original = worker.clone();
        apply_overrides(&mut worker, None, None, None, None, None);
        assert_eq!(worker.watch, original.watch);
        assert_eq!(worker.done, original.done);
        assert_eq!(worker.prompt, original.prompt);
        assert_eq!(worker.prompt_file, original.prompt_file);
        assert_eq!(worker.model, original.model);
        assert_eq!(worker.effort, original.effort);
    }

    #[test]
    fn test_apply_overrides_overrides_each_field() {
        let mut worker = test_worker();
        apply_overrides(
            &mut worker,
            Some("watching".into()),
            Some("finished".into()),
            Some("prompt.txt".into()),
            Some("opus".into()),
            Some("high".into()),
        );
        assert_eq!(worker.watch, "watching");
        assert_eq!(worker.done, "finished");
        assert_eq!(worker.prompt_file, Some("prompt.txt".into()));
        assert_eq!(worker.model, Some("opus".into()));
        assert_eq!(worker.effort, Some("high".into()));
    }

    #[test]
    fn test_apply_overrides_prompt_file_clears_prompt() {
        let mut worker = test_worker();
        assert!(worker.prompt.is_some());
        apply_overrides(&mut worker, None, None, Some("prompt.txt".into()), None, None);
        assert_eq!(worker.prompt, None);
        assert_eq!(worker.prompt_file, Some("prompt.txt".into()));
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
    fn test_steal_stale_lock_removes_old_lock() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("card.lock");
        let created = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .saturating_sub(STALE_LOCK_SECS + 1);
        fs::write(&lock_path, created.to_string()).unwrap();

        assert!(
            steal_stale_lock(&lock_path),
            "a lock older than the stale threshold should be stolen"
        );
        assert!(
            !lock_path.exists(),
            "stealing a stale lock should remove it"
        );
    }

    #[test]
    fn test_steal_stale_lock_leaves_fresh_lock_alone() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("card.lock");
        let created = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        fs::write(&lock_path, created.to_string()).unwrap();

        assert!(
            !steal_stale_lock(&lock_path),
            "a fresh lock must not be stolen"
        );
        assert!(lock_path.exists());
    }

    #[test]
    fn test_claim_recovers_from_crashed_worker_lock() {
        let dir = tempfile::tempdir().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let card = Card::new("Stale Lock Card".into(), "".into());
        storage::save_card(&card).unwrap();

        fs::create_dir_all(claims_dir()).unwrap();
        let lock_path =
            claims_dir().join(format!("{}.lock", storage::sanitize_filename(&card.name)));
        let created = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .saturating_sub(STALE_LOCK_SECS + 1);
        fs::write(&lock_path, created.to_string()).unwrap();

        let result = claim(&card.name, "todo", "new-worker");

        std::env::set_current_dir(original).unwrap();

        let claimed = result
            .unwrap()
            .expect("a stale lock should be stolen and the card claimed");
        assert_eq!(claimed.owner, Some("new-worker".to_string()));
    }

    #[test]
    fn test_sort_by_board_order_matches_column_display() {
        let mut top = Card::new("dragged to top".into(), "".into());
        top.order = Some(0);
        let mut second = Card::new("second".into(), "".into());
        second.order = Some(1);
        let unordered_old = Card::new("never dragged, oldest".into(), "".into());
        let mut unordered_new = Card::new("never dragged, newest".into(), "".into());
        unordered_new.created_at = unordered_old.created_at + chrono::Duration::seconds(10);

        let mut cards = vec![
            unordered_new.clone(),
            second.clone(),
            unordered_old.clone(),
            top.clone(),
        ];
        sort_by_board_order(&mut cards);

        let names: Vec<&str> = cards.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "dragged to top",
                "second",
                "never dragged, oldest",
                "never dragged, newest"
            ]
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
    fn test_truncate_agent_text_leaves_short_output_untouched() {
        let text = "short agent output";
        assert_eq!(truncate_agent_text(text), text);
    }

    #[test]
    fn test_truncate_agent_text_caps_long_output_with_note() {
        let text = "a".repeat(MAX_AGENT_TEXT_CHARS + 500);
        let truncated = truncate_agent_text(&text);
        assert!(
            truncated.len() < text.len(),
            "truncated output should be shorter than the original"
        );
        assert!(truncated.starts_with(&"a".repeat(MAX_AGENT_TEXT_CHARS)));
        assert!(truncated.contains(".cards/.logs/"));
    }

    #[test]
    fn test_task_prompt_contains_instructions() {
        let card = Card::new("My Card".into(), "Details".into());
        let repo_root = PathBuf::from("/repo");
        let prompt = task_prompt(&card, &test_worker(), "sparkly-otter-42", None, &repo_root);
        assert!(prompt.contains("cardthing edit \"My Card\" --status done"));
        assert!(prompt.contains("cardthing edit \"My Card\" --needs-human"));
        assert!(prompt.contains("sparkly-otter-42"));
        assert!(prompt.contains("Details"));
        assert!(!prompt.contains("isolated jj workspace"));
    }

    #[test]
    fn test_task_prompt_mentions_workspace_and_repo_root_when_isolated() {
        let card = Card::new("My Card".into(), "Details".into());
        let repo_root = PathBuf::from("/repo");
        let workspace = PathBuf::from("/repo-ws-sparkly-otter-42");
        let prompt = task_prompt(
            &card,
            &test_worker(),
            "sparkly-otter-42",
            Some(&workspace),
            &repo_root,
        );
        assert!(prompt.contains("/repo-ws-sparkly-otter-42"));
        assert!(prompt.contains("/repo"));
        assert!(prompt.contains("main repository directory"));
    }

    #[test]
    fn test_workspace_sibling_path_is_named_after_repo_and_worker() {
        let repo_root = PathBuf::from("/home/mark/code/cardthing");
        let path = workspace_sibling_path(&repo_root, "sparkly-otter-42");
        assert_eq!(
            path,
            PathBuf::from("/home/mark/code/cardthing-ws-sparkly-otter-42")
        );
    }

    #[test]
    fn test_jj_repo_present_false_outside_jj_repo() {
        let dir = tempfile::tempdir().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let present = jj_repo_present();
        std::env::set_current_dir(original).unwrap();
        assert!(!present, "a scratch tempdir should not look like a jj repo");
    }

    /// Exercises workspace create / has-changes / forget against a real
    /// scratch jj repo, skipping quietly if `jj` isn't on PATH in this
    /// environment.
    #[test]
    fn test_jj_workspace_lifecycle() {
        if Command::new("jj").arg("--version").output().is_err() {
            eprintln!("skipping: jj not found on PATH");
            return;
        }

        let base = tempfile::tempdir().unwrap();
        let repo_root = base.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();

        let init = Command::new("jj")
            .arg("git")
            .arg("init")
            .current_dir(&repo_root)
            .status()
            .unwrap();
        assert!(init.success());

        let path = create_workspace(&repo_root, "test-worker").unwrap();
        assert!(path.is_dir());
        assert_eq!(
            workspace_has_changes(&path).unwrap(),
            false,
            "freshly created workspace should have no changes"
        );

        fs::write(path.join("new-file.txt"), "hello").unwrap();
        assert_eq!(
            workspace_has_changes(&path).unwrap(),
            true,
            "workspace with a new file should report changes"
        );

        // A workspace with changes is left in place for human review.
        assert!(path.is_dir());

        forget_workspace(&repo_root, "test-worker", &path).unwrap();
        assert!(!path.exists(), "forgetting a workspace should remove it");
    }
}
