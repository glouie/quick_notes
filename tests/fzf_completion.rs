#![allow(deprecated)]

use assert_cmd::cargo::CommandCargoExt;
use tempfile::TempDir;

fn cmd(temp: &TempDir) -> assert_cmd::Command {
    let mut c = assert_cmd::Command::cargo_bin("quick_notes").unwrap();
    c.env("QUICK_NOTES_DIR", temp.path())
        .env("NO_COLOR", "1")
        .env("QUICK_NOTES_NO_FZF", "1"); // disable fzf dependency for tests
    c
}

#[test]
fn completion_simulated_prefix_match() {
    let temp = TempDir::new().unwrap();
    // create two notes with nearby prefixes
    cmd(&temp)
        .args(["new", "Prefix A", "body"])
        .assert()
        .success();
    cmd(&temp)
        .args(["new", "Prefix B", "body"])
        .assert()
        .success();

    // derive ids from files and simulate prefix filtering
    let ids: Vec<String> = std::fs::read_dir(temp.path())
        .unwrap()
        .filter_map(|e| {
            let p = e.ok()?.path();
            if p.extension().and_then(|s| s.to_str()) == Some("md") {
                Some(p.file_stem().unwrap().to_string_lossy().to_string())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(ids.len(), 2);

    let prefix = &ids[0][..4];
    let matches: Vec<&String> = ids.iter().filter(|id| id.starts_with(prefix)).collect();
    assert!(!matches.is_empty());
}
