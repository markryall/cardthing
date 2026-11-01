//! Per-card lock files, shared by agent workers, `edit` and `append`, so a
//! read-modify-write of one card never interleaves with another process's.
//!
//! A lock is a file in `.cards/.claims/<slug>.lock` created with O_EXCL and
//! stamped with the unix time; one older than [`STALE_LOCK_SECS`] is assumed to
//! be left over from a crashed process and may be stolen.

use super::{get_cards_path, sanitize_filename};
use anyhow::{bail, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// A lock file untouched for this long is assumed to be left over from a
/// process that crashed while holding it rather than one still legitimately held.
pub const STALE_LOCK_SECS: u64 = 60;

/// How long [`lock_card`] waits for another process to release a card.
const WAIT: Duration = Duration::from_secs(5);

pub fn lock_dir() -> PathBuf {
    get_cards_path().join(".claims")
}

pub fn lock_path(dir: &Path, card_name: &str) -> PathBuf {
    dir.join(format!("{}.lock", sanitize_filename(card_name)))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Try to exclusively create `lock_path`, stamping it with the current unix
/// time so staleness can later be judged from its contents. Returns whether
/// the lock was acquired.
pub fn try_acquire(lock_path: &Path) -> bool {
    // O_EXCL: creation fails if someone else holds the lock
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock_path)
    {
        Ok(mut f) => {
            let _ = f.write_all(now_secs().to_string().as_bytes());
            true
        }
        Err(_) => false,
    }
}

/// If `lock_path` holds a timestamp older than [`STALE_LOCK_SECS`], treat it
/// as left over from a crashed process and delete it. Returns true when the
/// caller should retry acquiring the lock (either it was stolen, or it had
/// already vanished on its own).
pub fn steal_stale(lock_path: &Path) -> bool {
    let contents = match fs::read_to_string(lock_path) {
        Ok(c) => c,
        Err(_) => return true, // vanished already; a retry will just recreate it
    };
    let created: u64 = match contents.trim().parse() {
        Ok(v) => v,
        Err(_) => return false, // unreadable timestamp; leave a lock we don't understand alone
    };
    if now_secs().saturating_sub(created) >= STALE_LOCK_SECS {
        fs::remove_file(lock_path).is_ok()
    } else {
        false
    }
}

/// A held card lock, released when dropped.
#[derive(Debug)]
pub struct CardLock {
    path: PathBuf,
}

impl Drop for CardLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Lock `card_name` for a read-modify-write, waiting briefly if another
/// process holds it.
pub fn lock_card(card_name: &str) -> Result<CardLock> {
    let dir = lock_dir();
    fs::create_dir_all(&dir)?;
    lock_at(lock_path(&dir, card_name), WAIT)
}

/// Acquire the lock file at `path`, polling for up to `wait`.
pub fn lock_at(path: PathBuf, wait: Duration) -> Result<CardLock> {
    let deadline = Instant::now() + wait;
    loop {
        if try_acquire(&path) || (steal_stale(&path) && try_acquire(&path)) {
            return Ok(CardLock { path });
        }
        if Instant::now() >= deadline {
            bail!(
                "Card is locked by another cardthing process ({}); try again, or delete the lock file if nothing is running",
                path.display()
            );
        }
        thread::sleep(Duration::from_millis(25));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    #[test]
    fn lock_is_released_on_drop_and_blocks_while_held() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("card.lock");
        let held = lock_at(path.clone(), Duration::ZERO).unwrap();
        assert!(lock_at(path.clone(), Duration::from_millis(60)).is_err());
        drop(held);
        assert!(!path.exists());
        assert!(lock_at(path, Duration::ZERO).is_ok());
    }

    #[test]
    fn waiters_serialise_a_read_modify_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("card.lock");
        let counter = dir.path().join("counter");
        fs::write(&counter, "0").unwrap();
        let barrier = Arc::new(Barrier::new(8));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let (path, counter, barrier) = (path.clone(), counter.clone(), barrier.clone());
                thread::spawn(move || {
                    barrier.wait();
                    let _lock = lock_at(path, Duration::from_secs(5)).unwrap();
                    let n: u32 = fs::read_to_string(&counter).unwrap().parse().unwrap();
                    thread::sleep(Duration::from_millis(5));
                    fs::write(&counter, (n + 1).to_string()).unwrap();
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(fs::read_to_string(&counter).unwrap(), "8");
    }

    #[test]
    fn stale_lock_is_stolen_fresh_one_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("card.lock");
        fs::write(&path, now_secs().to_string()).unwrap();
        assert!(!steal_stale(&path));
        fs::write(&path, (now_secs() - STALE_LOCK_SECS - 1).to_string()).unwrap();
        assert!(lock_at(path, Duration::ZERO).is_ok());
    }
}
