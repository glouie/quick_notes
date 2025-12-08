#![allow(deprecated)]

#[allow(unused_imports)]
use assert_cmd::cargo::CommandCargoExt;
use predicates::prelude::*;
use std::fs;
use std::io::Read;
use std::path::Path;
use tempfile::TempDir;

fn cmd(temp: &TempDir) -> assert_cmd::Command {
    let mut c = assert_cmd::Command::cargo_bin("quick_notes").unwrap();
    c.env("QUICK_NOTES_DIR", temp.path())
        .env("NO_COLOR", "1")
        .env("QUICK_NOTES_NO_FZF", "1");
    c
}

fn read_note(dir: &Path, id: &str) -> String {
    let mut buf = String::new();
    let path = dir.join(format!("{id}.md"));
    let mut f = std::fs::File::open(path).expect("note file");
    f.read_to_string(&mut buf).unwrap();
    buf
}

fn write_note_file(
    dir: &Path,
    id: &str,
    title: &str,
    created: &str,
    updated: &str,
    tags: &[&str],
    body: &str,
) {
    let tags_line = if tags.is_empty() {
        "Tags:".to_string()
    } else {
        let t: Vec<String> = tags
            .iter()
            .map(|t| {
                if t.starts_with('#') {
                    t.to_string()
                } else {
                    format!("#{}", t)
                }
            })
            .collect();
        format!("Tags: {}", t.join(", "))
    };
    let content = format!(
        "Title: {}\nCreated: {}\nUpdated: {}\n{}\n---\n{}\n",
        title, created, updated, tags_line, body
    );
    fs::write(dir.join(format!("{id}.md")), content).unwrap();
}

#[test]
fn view_render_plain_and_tag_guard() {
    let temp = TempDir::new().unwrap();
    cmd(&temp)
        .args(["add", "render body", "-t", "demo"])
        .assert()
        .success();
    let list_out = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let id = String::from_utf8_lossy(&list_out)
        .split_whitespace()
        .next()
        .unwrap()
        .to_string();

    cmd(&temp)
        .args(["view", &id, "--render", "--plain", "-t", "#demo"])
        .assert()
        .success()
        .stdout(predicate::str::contains("render body"))
        .stdout(predicate::str::contains("#"));

    cmd(&temp)
        .args(["view", &id, "--render", "-t", "#missing"])
        .assert()
        .failure();
}

#[test]
fn edit_tag_guard_blocks_mismatch() {
    let temp = TempDir::new().unwrap();
    cmd(&temp)
        .args(["new", "EditMe", "body", "-t", "keep"])
        .assert()
        .success();
    let list_out = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let id = String::from_utf8_lossy(&list_out)
        .split_whitespace()
        .next()
        .unwrap()
        .to_string();

    cmd(&temp)
        .env("EDITOR", "true")
        .args(["edit", &id, "-t", "#keep"])
        .assert()
        .success();

    cmd(&temp)
        .env("EDITOR", "true")
        .args(["edit", &id, "-t", "#other"])
        .assert()
        .failure();
}

#[test]
fn delete_with_tag_filter() {
    let temp = TempDir::new().unwrap();
    cmd(&temp)
        .args(["add", "keep me", "-t", "keep"])
        .assert()
        .success();
    cmd(&temp).args(["add", "drop me"]).assert().success();
    let list_out = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_str = String::from_utf8_lossy(&list_out);
    let mut keep_id = String::new();
    let mut drop_id = String::new();
    for line in list_str.lines() {
        let id = line.split_whitespace().next().unwrap();
        if line.contains("keep me") {
            keep_id = id.to_string();
        } else if line.contains("drop me") {
            drop_id = id.to_string();
        }
    }

    cmd(&temp)
        .args(["delete", &drop_id, "-t", "#keep"])
        .assert()
        .success();
    cmd(&temp).args(["view", &drop_id]).assert().success();

    cmd(&temp).args(["delete", &drop_id]).assert().success();
    cmd(&temp)
        .args(["delete", &keep_id, "-t", "#keep"])
        .assert()
        .success();
    cmd(&temp).args(["view", &keep_id]).assert().failure();
}

#[test]
fn list_sort_created_updated_size() {
    let temp = TempDir::new().unwrap();
    write_note_file(
        temp.path(),
        "a",
        "A",
        "01/01/2020 10:00 AM -00:00",
        "01/01/2020 10:00 AM -00:00",
        &[],
        "short",
    );
    write_note_file(
        temp.path(),
        "b",
        "B",
        "01/02/2020 10:00 AM -00:00",
        "01/02/2020 10:00 AM -00:00",
        &[],
        "long body text here that is longer",
    );
    write_note_file(
        temp.path(),
        "c",
        "C",
        "01/03/2020 10:00 AM -00:00",
        "01/03/2020 10:00 AM -00:00",
        &[],
        "medium body",
    );

    let created_asc = cmd(&temp)
        .args(["list", "--sort", "created", "--asc"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let first = String::from_utf8_lossy(&created_asc)
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_string();
    assert_eq!(first, "a");

    let size_desc = cmd(&temp)
        .args(["list", "--sort", "size", "--desc"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let first_size = String::from_utf8_lossy(&size_desc)
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_string();
    assert_eq!(first_size, "b");
}

#[test]
fn pinned_tags_override_env() {
    let temp = TempDir::new().unwrap();
    let out = cmd(&temp)
        .env("QUICK_NOTES_PINNED_TAGS", "#keep,#demo")
        .args(["tags"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("#keep"));
    assert!(s.contains("#demo"));
}

#[test]
fn path_and_help() {
    let temp = TempDir::new().unwrap();
    cmd(&temp)
        .args(["path"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            temp.path().to_string_lossy().as_ref(),
        ));

    cmd(&temp)
        .args(["help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Quick Notes CLI"));
}

#[test]
fn add_and_list_and_view() {
    let temp = TempDir::new().unwrap();
    // add
    cmd(&temp).args(["add", "hello world"]).assert().success();
    // list
    let output = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let out_str = String::from_utf8_lossy(&output);
    assert!(out_str.contains("hello world"));
    // view render
    let id = out_str.split_whitespace().next().unwrap();
    cmd(&temp)
        .args(["render", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello world"));
}

#[test]
fn new_with_tags_and_search_filter() {
    let temp = TempDir::new().unwrap();
    cmd(&temp)
        .args([
            "new",
            "Project Plan",
            "draft body",
            "-t",
            "work",
            "-t",
            "#plan",
        ])
        .assert()
        .success();
    let list_all = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let all_str = String::from_utf8_lossy(&list_all);
    assert!(all_str.contains("#work"));
    assert!(all_str.contains("#plan"));

    let list_filtered = cmd(&temp)
        .args(["list", "-s", "draft"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let filtered_str = String::from_utf8_lossy(&list_filtered);
    assert!(filtered_str.contains("Project Plan"));

    let list_tag = cmd(&temp)
        .args(["list", "-t", "#work"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let tag_str = String::from_utf8_lossy(&list_tag);
    assert!(tag_str.contains("Project Plan"));
}

#[test]
fn delete_and_delete_all() {
    let temp = TempDir::new().unwrap();
    cmd(&temp).args(["add", "one"]).assert().success();
    cmd(&temp).args(["add", "two"]).assert().success();
    let list_out = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_str = String::from_utf8_lossy(&list_out);
    let ids: Vec<&str> = list_str
        .lines()
        .take(2)
        .map(|l| l.split_whitespace().next().unwrap())
        .collect();
    cmd(&temp).args(["delete", ids[0]]).assert().success();
    // ensure first gone
    cmd(&temp).args(["view", ids[0]]).assert().failure();
    // delete-all removes remainder
    cmd(&temp).args(["delete-all"]).assert().success();
    let after = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(String::from_utf8_lossy(&after).contains("No notes yet"));
}

#[test]
fn seed_with_tags_and_list_sort() {
    let temp = TempDir::new().unwrap();
    cmd(&temp)
        .args(["seed", "3", "--chars", "50", "-t", "bulk"])
        .assert()
        .success();
    let out = cmd(&temp)
        .args(["list", "--sort", "size", "--asc"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let out_str = String::from_utf8_lossy(&out);
    assert!(out_str.contains("#bulk"));
    // ensure three files exist
    let count = fs::read_dir(temp.path())
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .map(|s| s == "md")
                .unwrap_or(false)
        })
        .count();
    assert_eq!(count, 3);
}

#[test]
fn tags_command_shows_pinned_and_counts() {
    let temp = TempDir::new().unwrap();
    cmd(&temp)
        .args(["add", "alpha", "-t", "todo"])
        .assert()
        .success();
    let tags_out = cmd(&temp)
        .args(["tags"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let tags_str = String::from_utf8_lossy(&tags_out);
    assert!(tags_str.contains("#todo"));
    assert!(tags_str.contains("count"));
}

#[test]
fn tags_written_in_header() {
    let temp = TempDir::new().unwrap();
    cmd(&temp)
        .args(["new", "Tagged", "body", "-t", "x", "-t", "#y"])
        .assert()
        .success();
    let list_out = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_str = String::from_utf8_lossy(&list_out);
    let id = list_str.split_whitespace().next().unwrap();
    let note = read_note(temp.path(), id);
    assert!(note.contains("Tags: #x, #y"));
}
