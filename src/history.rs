//! Read-only access to past board states stored in git history.
//!
//! The "time machine" in `cardthing serve` uses this to show the board as it
//! was at any commit that touched `.cards/` or `.cards.toml`. Everything is
//! read straight from git objects; the working copy is never touched. jj repos
//! work as long as they are colocated (a `.git` directory is present).

use crate::models::{Card, Config};
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::path::Path;
use std::process::Command;

/// A commit that changed the board's files.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BoardCommit {
    pub sha: String,
    pub short: String,
    pub date: String,
    pub subject: String,
}

/// The board as it was at one commit.
#[derive(Debug)]
pub struct Snapshot {
    pub commit: BoardCommit,
    pub config: Config,
    pub cards: Vec<Card>,
    /// Card files at that commit that could not be parsed (e.g. older formats).
    pub unreadable: Vec<String>,
}

const FIELD_SEP: char = '\u{1f}';

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .context("Failed to run git")?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Commits reachable from HEAD that touched `.cards/` or `.cards.toml`,
/// newest first.
pub fn board_commits(root: &Path) -> Result<Vec<BoardCommit>> {
    let format = "--format=%H%x1f%h%x1f%aI%x1f%s";
    let out = git(root, &["log", format, "--", ".cards", ".cards.toml"])?;
    Ok(out.lines().filter_map(parse_log_line).collect())
}

fn parse_log_line(line: &str) -> Option<BoardCommit> {
    let mut parts = line.splitn(4, FIELD_SEP);
    Some(BoardCommit {
        sha: parts.next()?.to_string(),
        short: parts.next()?.to_string(),
        date: parts.next()?.to_string(),
        subject: parts.next().unwrap_or("").to_string(),
    })
}

/// True if `rev` looks like an abbreviated or full commit hash. Only hashes are
/// accepted so nothing user-supplied is ever interpreted as a git option or
/// revision expression.
pub fn is_commit_hash(rev: &str) -> bool {
    (4..=40).contains(&rev.len()) && rev.chars().all(|c| c.is_ascii_hexdigit())
}

/// Loads the board (cards and column config) as it was at `rev`.
pub fn snapshot_at(root: &Path, rev: &str) -> Result<Snapshot> {
    if !is_commit_hash(rev) {
        bail!("'{}' is not a commit hash", rev);
    }
    let format = "--format=%H%x1f%h%x1f%aI%x1f%s";
    let commit_ref = format!("{}^{{commit}}", rev);
    let log = git(root, &["log", "-1", format, &commit_ref, "--"])
        .with_context(|| format!("Unknown commit '{}'", rev))?;
    let commit = log
        .lines()
        .next()
        .and_then(parse_log_line)
        .with_context(|| format!("Unknown commit '{}'", rev))?;

    let config = git(root, &["show", &format!("{}:./.cards.toml", commit.sha)])
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();

    let mut cards = Vec::new();
    let mut unreadable = Vec::new();
    // ls-tree fails when .cards/ didn't exist yet; that's just an empty board.
    let tree = git(root, &["ls-tree", &commit.sha, "./.cards/"]).unwrap_or_default();
    for path in card_paths(&tree) {
        let parsed = git(root, &["show", &format!("{}:./{}", commit.sha, path)])
            .ok()
            .and_then(|s| toml::from_str::<Card>(&s).ok());
        match parsed {
            Some(card) => cards.push(card),
            None => unreadable.push(path),
        }
    }

    Ok(Snapshot {
        commit,
        config,
        cards,
        unreadable,
    })
}

/// Extracts the `.toml` blob paths from `git ls-tree` output.
fn card_paths(ls_tree: &str) -> Vec<String> {
    ls_tree
        .lines()
        .filter_map(|line| {
            let (meta, path) = line.split_once('\t')?;
            let kind = meta.split_whitespace().nth(1)?;
            (kind == "blob" && path.ends_with(".toml")).then(|| path.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_log_lines_with_separators_in_subject() {
        let line = "abc123\u{1f}abc\u{1f}2026-01-01T00:00:00+00:00\u{1f}fix: a\u{1f}b";
        let c = parse_log_line(line).unwrap();
        assert_eq!(c.sha, "abc123");
        assert_eq!(c.short, "abc");
        assert_eq!(c.subject, "fix: a\u{1f}b");
    }

    #[test]
    fn accepts_only_hex_hashes() {
        assert!(is_commit_hash("2257e64"));
        assert!(is_commit_hash(&"a".repeat(40)));
        assert!(!is_commit_hash("abc"));
        assert!(!is_commit_hash("HEAD"));
        assert!(!is_commit_hash("--output=x"));
        assert!(!is_commit_hash("main~1"));
    }

    fn run_git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(status.status.success(), "{:?}", status);
    }

    fn write_card(dir: &Path, name: &str, status: &str) {
        let mut card = Card::new(name.to_string(), String::new());
        card.status = status.to_string();
        let path = dir.join(".cards").join(format!("{}.toml", name));
        std::fs::write(path, toml::to_string(&card).unwrap()).unwrap();
    }

    #[test]
    fn reads_deleted_cards_from_an_earlier_commit() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        run_git(dir, &["init", "-q"]);
        std::fs::create_dir(dir.join(".cards")).unwrap();
        std::fs::write(dir.join(".cards.toml"), "title = \"Past\"\n").unwrap();
        write_card(dir, "keep", "todo");
        write_card(dir, "gone", "done");
        std::fs::write(dir.join(".cards/broken.toml"), "not = [valid").unwrap();
        run_git(dir, &["add", "."]);
        run_git(dir, &["commit", "-q", "-m", "first"]);
        std::fs::remove_file(dir.join(".cards/gone.toml")).unwrap();
        std::fs::write(dir.join("unrelated.txt"), "x").unwrap();
        run_git(dir, &["add", "-A"]);
        run_git(dir, &["commit", "-q", "-m", "delete gone"]);
        std::fs::write(dir.join("unrelated.txt"), "y").unwrap();
        run_git(dir, &["commit", "-qam", "unrelated only"]);

        let commits = board_commits(dir).unwrap();
        let subjects: Vec<_> = commits.iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(subjects, vec!["delete gone", "first"]);

        let first = snapshot_at(dir, &commits[1].short).unwrap();
        assert_eq!(first.config.title, "Past");
        let mut names: Vec<_> = first.cards.iter().map(|c| c.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["gone", "keep"]);
        assert_eq!(first.unreadable, vec![".cards/broken.toml"]);

        let latest = snapshot_at(dir, &commits[0].sha).unwrap();
        assert_eq!(latest.cards.len(), 1);

        assert!(snapshot_at(dir, "deadbeef").is_err());
        assert!(snapshot_at(dir, "HEAD").is_err());
    }

    #[test]
    fn picks_toml_blobs_from_ls_tree() {
        let out = "100644 blob aaa\t.cards/one.toml\n040000 tree bbb\t.cards/prompts\n100644 blob ccc\t.cards/notes.md\n";
        assert_eq!(card_paths(out), vec![".cards/one.toml"]);
    }
}
