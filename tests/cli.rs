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
        .env("COLUMNS", "80")
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

fn list_ids(output: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(output)
        .lines()
        .filter_map(|line| {
            let id = line.split_whitespace().next()?;
            if id.eq_ignore_ascii_case("id")
                || id == "No"
                || id.starts_with('=')
            {
                return None;
            }
            Some(id.to_string())
        })
        .collect()
}

fn parse_added_id(output: &[u8]) -> String {
    String::from_utf8_lossy(output)
        .split_whitespace()
        .nth(2)
        .expect("id in add output")
        .to_string()
}

fn first_list_id(output: &[u8]) -> String {
    list_ids(output).into_iter().next().expect("note id")
}

#[test]
fn view_render_plain_and_tag_guard() {
    let temp = TempDir::new().unwrap();
    cmd(&temp)
        .args(["new", "render body", "", "-t", "demo"])
        .assert()
        .success();
    let list_out = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let id = first_list_id(&list_out);

    cmd(&temp)
        .args(["view", "--render", "--plain", "-t", "#demo", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("render body"))
        .stdout(predicate::str::contains("====="));

    cmd(&temp)
        .args(["view", &id, "--render", "-t", "#missing"])
        .assert()
        .failure();
}

#[test]
fn edit_tag_guard_blocks_mismatch() {
    let temp = TempDir::new().unwrap();
    cmd(&temp).args(["new", "EditMe", "body", "-t", "keep"]).assert().success();
    let list_out = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let id = first_list_id(&list_out);

    cmd(&temp)
        .env("EDITOR", "true")
        .args(["edit", "-t", "#keep", &id])
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
    cmd(&temp).args(["new", "keep me", "", "-t", "keep"]).assert().success();
    cmd(&temp).args(["new", "drop me", "", "-t", "tmp"]).assert().success();
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
    for line in list_str.lines().skip(2) {
        let id = line.split_whitespace().next().unwrap();
        if line.contains("keep me") {
            keep_id = id.to_string();
        } else if line.contains("drop me") {
            drop_id = id.to_string();
        }
    }

    cmd(&temp).args(["delete", &drop_id, "-t", "#keep"]).assert().success();
    cmd(&temp).args(["view", &drop_id]).assert().success();

    cmd(&temp).args(["delete", &drop_id]).assert().success();
    cmd(&temp).args(["delete", &keep_id, "-t", "#keep"]).assert().success();
    cmd(&temp).args(["view", &keep_id]).assert().failure();
}

#[test]
fn list_sort_created_updated_size() {
    let temp = TempDir::new().unwrap();
    write_note_file(
        temp.path(),
        "a",
        "A",
        "01Jan20 10:00 -00:00",
        "01Jan20 10:00 -00:00",
        &[],
        "short",
    );
    write_note_file(
        temp.path(),
        "b",
        "B",
        "02Jan20 10:00 -00:00",
        "02Jan20 10:00 -00:00",
        &[],
        "long body text here that is longer",
    );
    write_note_file(
        temp.path(),
        "c",
        "C",
        "03Jan20 10:00 -00:00",
        "03Jan20 10:00 -00:00",
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
    let first = first_list_id(&created_asc);
    assert_eq!(first, "a");

    let size_desc = cmd(&temp)
        .args(["list", "--sort", "size", "--desc"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let first_size = first_list_id(&size_desc);
    assert_eq!(first_size, "b");
}

#[test]
fn ids_are_short_and_incremental() {
    let temp = TempDir::new().unwrap();
    let first_out = cmd(&temp)
        .args(["new", "one", ""])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second_out = cmd(&temp)
        .args(["new", "two", ""])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let first = parse_added_id(&first_out);
    let second = parse_added_id(&second_out);
    assert!(first.len() <= 11);
    assert!(second.len() <= 11);
    assert!(first < second, "ids should increment: {first} >= {second}");
}

#[test]
fn list_headers_align_to_columns() {
    let temp = TempDir::new().unwrap();
    write_note_file(
        temp.path(),
        "a",
        "A",
        "01Jan20 10:00 -00:00",
        "01Jan20 10:00 -00:00",
        &[],
        "short",
    );
    write_note_file(
        temp.path(),
        "b",
        "B",
        "02Jan20 10:00 -00:00",
        "02Jan20 10:00 -00:00",
        &["tag"],
        "body",
    );

    let out = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let out_str = String::from_utf8_lossy(&out);
    let mut lines = out_str.lines();
    let header = lines.next().unwrap();
    let _separator = lines.next().unwrap();
    let first = lines.next().unwrap();

    let header_cols: Vec<&str> = header.split('|').map(|s| s.trim()).collect();
    let first_cols: Vec<&str> = first.split('|').map(|s| s.trim()).collect();

    assert!(header_cols.len() >= 4);
    assert_eq!(first_cols.len(), header_cols.len());

    let updated_col = first_cols[1];
    let preview_col = first_cols[2];
    let tags_col = first_cols[3];

    assert_eq!(updated_col, "02Jan20 10:00");
    assert_eq!(preview_col, "B body");
    assert_eq!(tags_col, "#tag");
}

#[test]
fn migrate_ids_renames_existing_notes() {
    let temp = TempDir::new().unwrap();
    write_note_file(
        temp.path(),
        "2024010101010101",
        "Old Id One",
        "01Jan20 10:00 -00:00",
        "01Jan20 10:00 -00:00",
        &[],
        "body",
    );
    write_note_file(
        temp.path(),
        "2024010101010102",
        "Old Id Two",
        "01Jan20 10:00 -00:00",
        "02Jan20 10:00 -00:00",
        &[],
        "body",
    );

    cmd(&temp)
        .args(["migrate-ids"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Migrated"));

    let files: Vec<String> = std::fs::read_dir(temp.path())
        .unwrap()
        .filter_map(|e| {
            let p = e.ok()?.path();
            if p.extension().and_then(|s| s.to_str()) == Some("md") {
                p.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect();

    assert_eq!(files.len(), 2);
    for id in files {
        assert!(id.len() <= 11, "id not shortened: {id}");
    }
}

#[test]
fn migrate_copies_notes_and_preserves_timestamps() {
    let temp = TempDir::new().unwrap();
    let src = TempDir::new().unwrap();
    write_note_file(
        src.path(),
        "oldnote1",
        "Imported",
        "01Jan20 10:00 -00:00",
        "01Jan20 10:01 -00:00",
        &["demo"],
        "from elsewhere",
    );

    cmd(&temp)
        .args(["migrate", src.path().to_str().unwrap()])
        .assert()
        .success();

    let list_out = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ids = list_ids(&list_out);
    assert!(ids.contains(&"oldnote1".to_string()));

    let batch_dir = std::fs::read_dir(temp.path().join("migrated"))
        .unwrap()
        .next()
        .expect("batch dir")
        .unwrap()
        .path();
    let migrated = batch_dir.join("oldnote1.md");
    let contents = fs::read_to_string(migrated).unwrap();
    assert!(contents.contains("Created: 01Jan20 10:00 -00:00"));
    assert!(contents.contains("Updated: 01Jan20 10:01 -00:00"));
}

#[test]
fn migrate_generates_new_ids_on_conflict() {
    let temp = TempDir::new().unwrap();
    let src = TempDir::new().unwrap();
    write_note_file(
        temp.path(),
        "dup123",
        "Existing",
        "01Jan20 10:00 -00:00",
        "01Jan20 10:00 -00:00",
        &[],
        "body",
    );
    write_note_file(
        src.path(),
        "dup123",
        "Incoming",
        "02Jan20 11:00 -00:00",
        "02Jan20 11:05 -00:00",
        &[],
        "incoming",
    );

    cmd(&temp)
        .args(["migrate", src.path().to_str().unwrap()])
        .assert()
        .success();

    let list_out = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ids = list_ids(&list_out);
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"dup123".to_string()));
    let new_id = ids.into_iter().find(|id| id != "dup123").unwrap();

    let migrated_root = temp.path().join("migrated");
    let mut migrated_path = None;
    if migrated_root.exists() {
        for batch in fs::read_dir(migrated_root).unwrap() {
            let batch = batch.unwrap().path();
            let candidate = batch.join(format!("{new_id}.md"));
            if candidate.exists() {
                migrated_path = Some(candidate);
                break;
            }
        }
    }
    let migrated_path =
        migrated_path.expect("imported note written to migrated dir");
    let contents = fs::read_to_string(migrated_path).unwrap();
    assert!(contents.contains("Created: 02Jan20 11:00 -00:00"));
}

#[test]
fn list_respects_terminal_width() {
    let temp = TempDir::new().unwrap();
    write_note_file(
        temp.path(),
        "longid1234567890",
        "Very long title",
        "01Jan20 10:00 -00:00",
        "02Jan20 10:00 -00:00",
        &["firsttag", "secondtag", "thirdtag"],
        "This is an extremely long preview body that should be trimmed to fit \
within the provided terminal width for testing purposes.",
    );

    let out = cmd(&temp)
        .env("COLUMNS", "50")
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    for line in String::from_utf8_lossy(&out).lines() {
        assert!(
            line.chars().count() <= 50,
            "line exceeds terminal width: {line}"
        );
    }
}

#[test]
fn view_multiple_notes() {
    let temp = TempDir::new().unwrap();
    write_note_file(
        temp.path(),
        "id1",
        "First",
        "01Jan20 10:00 -00:00",
        "01Jan20 10:00 -00:00",
        &[],
        "body one",
    );
    write_note_file(
        temp.path(),
        "id2",
        "Second",
        "02Jan20 10:00 -00:00",
        "02Jan20 10:00 -00:00",
        &[],
        "body two",
    );

    let out = cmd(&temp)
        .args(["view", "id1", "id2"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("===== First (id1) ====="));
    assert!(s.contains("body one"));
    assert!(s.contains("===== Second (id2) ====="));
    assert!(s.contains("body two"));
}

#[test]
fn edit_multiple_notes_updates_timestamp() {
    let temp = TempDir::new().unwrap();
    write_note_file(
        temp.path(),
        "id1",
        "First",
        "01Jan20 10:00 -00:00",
        "01Jan20 10:00 -00:00",
        &[],
        "body one",
    );
    write_note_file(
        temp.path(),
        "id2",
        "Second",
        "01Jan20 10:00 -00:00",
        "01Jan20 10:00 -00:00",
        &[],
        "body two",
    );

    cmd(&temp)
        .env("EDITOR", "true")
        .args(["edit", "id1", "id2"])
        .assert()
        .success();

    let n1 = read_note(temp.path(), "id1");
    let n2 = read_note(temp.path(), "id2");
    assert!(!n1.contains("Updated: 01Jan20 10:00 -00:00"));
    assert!(!n2.contains("Updated: 01Jan20 10:00 -00:00"));
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
    cmd(&temp).args(["path"]).assert().success().stdout(
        predicate::str::contains(temp.path().to_string_lossy().as_ref()),
    );

    cmd(&temp)
        .args(["help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Quick Notes CLI"));
}

#[test]
fn guide_lists_guides() {
    let temp = TempDir::new().unwrap();
    let out = cmd(&temp)
        .args(["guide"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("(guides)"));
    assert!(s.contains("getting-started"));

    let detail = cmd(&temp)
        .args(["guide", "searching"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let d = String::from_utf8_lossy(&detail);
    assert!(d.contains("searching — Search and filter strategy"));
    assert!(d.contains("usage: qn help searching"));
}

#[test]
fn help_topic_details() {
    let temp = TempDir::new().unwrap();
    let out = cmd(&temp)
        .args(["help", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("usage: qn list"));
    assert!(s.contains("Options:"));
    assert!(s.contains("Substring search"));
}

#[test]
fn add_and_list_and_view() {
    let temp = TempDir::new().unwrap();
    // create
    cmd(&temp).args(["new", "Hello", "hello world"]).assert().success();
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
    let id = first_list_id(&output);
    cmd(&temp)
        .args(["render", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello world"));
}

#[test]
fn add_appends_to_existing_note() {
    let temp = TempDir::new().unwrap();
    cmd(&temp).args(["new", "Append Me", "first line"]).assert().success();
    let list_out = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let id = first_list_id(&list_out);
    let before = read_note(temp.path(), &id);
    cmd(&temp).args(["add", &id, "second line"]).assert().success();
    let after = read_note(temp.path(), &id);
    assert!(after.contains("first line"));
    assert!(after.contains("second line"));
    assert_ne!(before, after);
}

#[test]
fn add_with_body_only_creates_note() {
    let temp = TempDir::new().unwrap();
    let output = cmd(&temp)
        .args(["add", "Capture this standalone line"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let id = parse_added_id(&output);
    let note = read_note(temp.path(), &id);
    assert!(note.contains("Title: Capture this standalone line"));
    assert!(note.contains("Capture this standalone line"));
}

#[test]
fn add_body_only_truncates_long_title() {
    let temp = TempDir::new().unwrap();
    let long_body = "This is a really long capture line that will definitely exceed eighty characters so we can verify truncation is applied to the derived title for the note.";
    let output = cmd(&temp)
        .args(["add", long_body])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let id = parse_added_id(&output);
    let note = read_note(temp.path(), &id);
    let title_line =
        note.lines().find(|l| l.starts_with("Title: ")).expect("title line");
    assert!(title_line.ends_with("..."));
    assert!(title_line.contains("This is a really long capture line"));
    assert!(title_line.len() <= "Title: ".len() + 80);
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
    cmd(&temp).args(["new", "one", ""]).assert().success();
    cmd(&temp).args(["new", "two", ""]).assert().success();
    let list_out = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ids: Vec<String> = list_ids(&list_out).into_iter().take(2).collect();
    cmd(&temp).args(["delete", ids[0].as_str()]).assert().success();
    // ensure first gone from active list
    cmd(&temp).args(["view", ids[0].as_str()]).assert().failure();
    // delete-all moves remainder to trash
    cmd(&temp).args(["delete-all"]).assert().success();
    let after = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(String::from_utf8_lossy(&after).contains("No notes yet"));
    // trash should contain both ids
    let trash_list = cmd(&temp)
        .args(["list-deleted"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let trash_ids = list_ids(&trash_list);
    assert!(trash_ids.contains(&ids[0]));
    assert!(trash_ids.contains(&ids[1]));
}

#[test]
fn soft_delete_and_undelete() {
    let temp = TempDir::new().unwrap();
    cmd(&temp).args(["new", "temp note", ""]).assert().success();
    let list_out = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let id = first_list_id(&list_out);
    cmd(&temp).args(["delete", &id]).assert().success();
    cmd(&temp).args(["view", &id]).assert().failure();
    let trash = cmd(&temp)
        .args(["list-deleted"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(String::from_utf8_lossy(&trash).contains(&id));
    cmd(&temp).args(["undelete", &id]).assert().success();
    cmd(&temp).args(["view", &id]).assert().success();
}

#[test]
fn archive_and_unarchive() {
    let temp = TempDir::new().unwrap();
    cmd(&temp).args(["new", "keep me", ""]).assert().success();
    let list_out = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let id = first_list_id(&list_out);
    cmd(&temp).args(["archive", &id]).assert().success();
    cmd(&temp).args(["view", &id]).assert().failure();
    let archived = cmd(&temp)
        .args(["list-archived"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(String::from_utf8_lossy(&archived).contains(&id));
    cmd(&temp).args(["unarchive", &id]).assert().success();
    cmd(&temp).args(["view", &id]).assert().success();
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
fn seed_with_markdown_samples() {
    let temp = TempDir::new().unwrap();
    cmd(&temp).args(["seed", "--markdown", "-t", "md", "1"]).assert().success();
    let list_out = cmd(&temp)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let id = first_list_id(&list_out);
    let note = read_note(temp.path(), &id);
    assert!(note.contains("# Heading"));
    assert!(note.contains("```rust"));
    assert!(note.contains("- bullet"));
}

#[test]
fn view_render_preserves_lines() {
    let temp = TempDir::new().unwrap();
    cmd(&temp).args(["seed", "--markdown", "1"]).assert().success();
    let id = std::fs::read_dir(temp.path())
        .unwrap()
        .filter_map(|e| {
            let p = e.ok()?.path();
            if p.extension().and_then(|s| s.to_str()) == Some("md") {
                p.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
            } else {
                None
            }
        })
        .next()
        .expect("note id");

    let rendered_vec = cmd(&temp)
        .args(["view", "-r", &id])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let rendered = String::from_utf8_lossy(&rendered_vec).to_string();

    let raw = cmd(&temp)
        .args(["view", &id])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let raw = String::from_utf8_lossy(&raw).to_string();

    let raw_lines: Vec<&str> = raw.lines().collect();
    let ren_lines: Vec<&str> = rendered.lines().collect();
    assert_eq!(
        raw_lines.len(),
        ren_lines.len(),
        "rendered output should keep same line count"
    );
}

#[test]
fn tags_command_shows_pinned_and_counts() {
    let temp = TempDir::new().unwrap();
    cmd(&temp).args(["new", "alpha", "", "-t", "todo"]).assert().success();
    let tags_out = cmd(&temp)
        .args(["tags"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let tags_str = String::from_utf8_lossy(&tags_out);
    assert!(tags_str.contains("#todo"));
    assert!(tags_str.to_lowercase().contains("count"));
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
    let id = first_list_id(&list_out);
    let note = read_note(temp.path(), &id);
    assert!(note.contains("Tags: #x, #y"));
}
