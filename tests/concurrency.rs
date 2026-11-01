//! Many `cardthing` processes racing on one card, the way parallel wayfinder
//! sessions do. Each test runs the real binary in its own temp board, so no
//! test changes the process-wide current directory.

use std::path::Path;
use std::process::{Command, Output};
use std::thread;

fn cardthing(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cardthing"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

fn board() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    assert!(cardthing(dir.path(), &["init"]).status.success());
    dir
}

fn description(dir: &Path, name: &str) -> String {
    let out = cardthing(dir, &["list", "--format", "json"]);
    let cards: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    cards
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap()["description"]
        .as_str()
        .unwrap()
        .to_string()
}

fn race<F>(n: usize, f: F) -> Vec<Output>
where
    F: Fn(usize) -> Output + Sync,
{
    let f = &f;
    thread::scope(|s| {
        let handles: Vec<_> = (0..n).map(|i| s.spawn(move || f(i))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    })
}

#[test]
fn exactly_one_concurrent_claim_wins() {
    let dir = board();
    let d = dir.path();
    assert!(cardthing(d, &["add", "ticket"]).status.success());

    let outs = race(12, |i| {
        cardthing(d, &["edit", "ticket", "--claim", &format!("session-{}", i)])
    });
    let winners: Vec<_> = outs.iter().filter(|o| o.status.success()).collect();
    assert_eq!(winners.len(), 1, "exactly one claim should succeed");
    let losers = outs.iter().filter(|o| !o.status.success());
    for o in losers {
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(err.contains("already claimed by 'session-"), "{}", err);
    }

    let show = String::from_utf8_lossy(&cardthing(d, &["show", "ticket"]).stdout).to_string();
    let winner = String::from_utf8_lossy(&winners[0].stdout).to_string();
    let owner = show
        .lines()
        .find_map(|l| l.trim().strip_prefix("Owner:"))
        .unwrap()
        .trim()
        .to_string();
    assert!(winner.contains(&format!("claimed by '{}'", owner)));

    // re-claiming as the owner is a no-op success; --owner can't be mixed in
    assert!(cardthing(d, &["edit", "ticket", "--claim", &owner])
        .status
        .success());
    assert!(
        !cardthing(d, &["edit", "ticket", "--claim", "x", "--owner", "y"])
            .status
            .success()
    );
}

#[test]
fn concurrent_appends_to_one_section_are_all_kept() {
    let dir = board();
    let d = dir.path();
    let map = "## Destination\n\nsomewhere\n\n## Decisions so far\n\n## Out of scope\n";
    assert!(cardthing(d, &["add", "map", "--description", map])
        .status
        .success());

    let outs = race(10, |i| {
        cardthing(
            d,
            &[
                "append",
                "map",
                &format!("- ticket {}: decided", i),
                "--section",
                "Decisions so far",
            ],
        )
    });
    assert!(outs.iter().all(|o| o.status.success()));

    let desc = description(d, "map");
    for i in 0..10 {
        assert!(
            desc.contains(&format!("- ticket {}: decided", i)),
            "{}",
            desc
        );
    }
    let decisions = desc
        .split("## Decisions so far")
        .nth(1)
        .unwrap()
        .split("## Out of scope")
        .next()
        .unwrap();
    assert_eq!(
        decisions.lines().filter(|l| l.starts_with("- ")).count(),
        10
    );
    assert!(desc.trim_end().ends_with("## Out of scope"));
}

#[test]
fn append_reads_stdin_for_dash() {
    use std::io::Write;
    use std::process::Stdio;
    let dir = board();
    let d = dir.path();
    assert!(cardthing(
        d,
        &["add", "ticket", "--description", "## Question\n\nWhy?"]
    )
    .status
    .success());
    let mut child = Command::new(env!("CARGO_BIN_EXE_cardthing"))
        .current_dir(d)
        .args(["append", "ticket", "-", "--section", "Answer"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"Because.\n\nTwo paragraphs.\n")
        .unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(
        description(d, "ticket"),
        "## Question\n\nWhy?\n\n## Answer\n\nBecause.\n\nTwo paragraphs."
    );
}
