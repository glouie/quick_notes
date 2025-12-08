use chrono::{DateTime, FixedOffset, Local};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use std::cmp::Ordering;
use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use yansi::Paint;

const PINNED_TAGS_DEFAULT: &str = "#todo,#meeting,#scratch";

#[derive(Debug, Clone)]
struct Note {
    id: String,
    title: String,
    created: String,
    updated: String,
    body: String,
    tags: Vec<String>,
    size_bytes: u64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        print_help();
        return Ok(());
    }

    let cmd = args.remove(0);
    let dir = notes_dir()?;
    ensure_dir(&dir)?;

    match cmd.as_str() {
        "add" => quick_add(args, &dir)?,
        "new" => new_note(args, &dir)?,
        "list" => list_notes(args, &dir)?,
        "view" => view_note(args, &dir, false)?,
        "render" => view_note(args, &dir, true)?,
        "edit" => edit_note(args, &dir)?,
        "delete" => delete_notes(args, &dir)?,
        "seed" => seed_notes(args, &dir)?,
        "delete-all" => delete_all_notes(&dir)?,
        "tags" => list_tags(&dir)?,
        "path" => println!("{}", dir.display()),
        "completion" => print_completion(args)?,
        "help" => print_help(),
        other => {
            eprintln!("Unknown command: {other}");
            print_help();
        }
    }

    Ok(())
}

fn print_help() {
    println!(
        "\
Quick Notes CLI
Usage:
  qn add \"note text\"              Quick add with generated title
  qn new <title> [body...]        New note with title and optional body
  qn list [--sort <field>] [--asc|--desc] [-s|--search <text>] [-t|--tag <tag>]
                                  List notes (sort by created|updated|size; default updated desc)
  qn view <id> [--render|-r] [--plain]
                                  Show a note (render markdown with --render; disable color with --plain)
  qn render <id>                  Same as: qn view <id> --render
  qn edit <id> [-t|--tag <tag>]   Edit in $EDITOR (updates timestamp; requires tag match when provided)
  qn delete [ids...] [--fzf] [-t|--tag <tag>]
                                  Delete one or more notes (fzf multi-select when --fzf or no ids and fzf available; optional tag filter)
  qn delete-all                   Delete every note in the notes directory
  qn tags                         List tags with counts and first/last use
  qn seed <count> [--chars N]     Generate test notes (random body of N chars; default 400)
  qn path                         Show the notes directory
  qn completion zsh               Print zsh completion script for fzf-powered ids
  qn help                         Show this message

Environment:
  QUICK_NOTES_DIR                 Override notes directory (default: ~/.quick_notes)
"
    );
}

fn notes_dir() -> io::Result<PathBuf> {
    if let Ok(dir) = env::var("QUICK_NOTES_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = env::var("HOME").map_err(|_| {
        io::Error::new(
            io::ErrorKind::Other,
            "HOME not set; set QUICK_NOTES_DIR explicitly",
        )
    })?;
    Ok(PathBuf::from(home).join(".quick_notes"))
}

fn ensure_dir(path: &Path) -> io::Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

fn quick_add(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        return Err("Provide the note body, e.g. `qn add \"text\"`".into());
    }
    let (tags, body_parts) = split_tags(args);
    if body_parts.is_empty() {
        return Err("Provide the note body after tags, e.g. `qn add \"text\" -t #tag`".into());
    }
    let body = body_parts.join(" ");
    let title = format!("Quick note {}", short_timestamp());
    let note = create_note_with_tags(title, body, tags, dir)?;
    println!("Added note {} ({})", note.id, note.title);
    Ok(())
}

fn new_note(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        return Err("Usage: qn new <title> [body]".into());
    }
    let title = args[0].clone();
    let (tags, body_parts) = split_tags(args.into_iter().skip(1).collect());
    let body = body_parts.join(" ");
    let note = create_note_with_tags(title, body, tags, dir)?;
    println!("Created note {} ({})", note.id, note.title);
    Ok(())
}

fn list_notes(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut sort_field = "updated".to_string();
    let mut ascending = false;
    let mut search: Option<String> = None;
    let mut tag_filters: Vec<String> = Vec::new();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--sort" => {
                if let Some(v) = iter.next() {
                    sort_field = v;
                } else {
                    return Err("Provide a sort field: created|updated|size".into());
                }
            }
            "--asc" => ascending = true,
            "--desc" => ascending = false,
            "-s" | "--search" => {
                if let Some(v) = iter.next() {
                    search = Some(v);
                } else {
                    return Err("Provide a search string after -s/--search".into());
                }
            }
            "-t" | "--tag" => {
                if let Some(v) = iter.next() {
                    let tag = normalize_tag(&v);
                    if !tag.is_empty() {
                        tag_filters.push(tag);
                    }
                } else {
                    return Err("Provide a tag after -t/--tag".into());
                }
            }
            other => {
                return Err(format!("Unknown flag for list: {other}").into());
            }
        }
    }

    let mut notes: Vec<Note> = Vec::new();
    for (path, size) in list_note_files(dir)? {
        if let Ok(note) = parse_note(&path, size) {
            notes.push(note);
        }
    }

    if let Some(q) = &search {
        let ql = q.to_lowercase();
        notes.retain(|n| {
            n.title.to_lowercase().contains(&ql) || n.body.to_lowercase().contains(&ql)
        });
    }

    if !tag_filters.is_empty() {
        notes.retain(|n| note_has_tags(n, &tag_filters));
    }

    let comparator = |a: &Note, b: &Note| -> std::cmp::Ordering {
        match sort_field.as_str() {
            "created" => cmp_dt(&a.created, &b.created),
            "updated" => cmp_dt(&a.updated, &b.updated),
            "size" => a.size_bytes.cmp(&b.size_bytes),
            _ => cmp_dt(&a.updated, &b.updated),
        }
    };

    notes.sort_by(|a, b| {
        let ord = comparator(a, b);
        if ascending { ord } else { ord.reverse() }
    });

    if notes.is_empty() {
        println!("No notes yet. Try `qn add \"text\"`.");
        return Ok(());
    }

    let use_color = env::var("NO_COLOR").is_err();
    let max_preview = notes
        .iter()
        .map(|n| preview_line(n).chars().count())
        .max()
        .unwrap_or(0);

    for n in notes {
        let preview = preview_line(&n);
        let mut padded_preview = preview.clone();
        let pad = max_preview.saturating_sub(preview.chars().count());
        padded_preview.push_str(&" ".repeat(pad));
        let tags_text = format_tags(&n.tags, use_color);
        if tags_text.is_empty() {
            println!("{}  | {}  | {}", n.id, n.updated, padded_preview);
        } else {
            println!(
                "{}  | {}  | {}  {}",
                n.id, n.updated, padded_preview, tags_text
            );
        }
    }
    Ok(())
}

fn print_completion(args: Vec<String>) -> Result<(), Box<dyn Error>> {
    let shell = args.get(0).map(|s| s.as_str()).unwrap_or("zsh");
    match shell {
        "zsh" => {
            println!("{}", include_str!("../contrib/quick_notes_fzf.zsh"));
            Ok(())
        }
        _ => Err("Only zsh completion is supported right now".into()),
    }
}

fn view_note(args: Vec<String>, dir: &Path, force_render: bool) -> Result<(), Box<dyn Error>> {
    let mut args_iter = args.into_iter();
    let id = args_iter
        .next()
        .ok_or("Usage: qn view <id> [--render|-r] [--plain] [-t <tag>]")?;
    let mut render = force_render;
    let mut plain = false;
    let mut tag_filters: Vec<String> = Vec::new();
    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "--render" | "-r" | "render" => render = true,
            "--plain" => plain = true,
            "-t" | "--tag" => {
                if let Some(v) = args_iter.next() {
                    let tag = normalize_tag(&v);
                    if !tag.is_empty() {
                        tag_filters.push(tag);
                    }
                } else {
                    return Err("Provide a tag after -t/--tag".into());
                }
            }
            other => return Err(format!("Unknown flag for view: {other}").into()),
        }
    }
    let use_color = !plain && env::var("NO_COLOR").is_err();
    let path = note_path(dir, &id);
    if !path.exists() {
        return Err(format!("Note {id} not found").into());
    }
    let size = fs::metadata(&path)?.len();
    let note = parse_note(&path, size)?;
    if !tag_filters.is_empty() && !note_has_tags(&note, &tag_filters) {
        return Err(format!("Note {id} does not have required tag(s)").into());
    }
    if render {
        let rendered = render_markdown(&note.body, use_color);
        println!(
            "# {} ({})\nCreated: {}\nUpdated: {}\n\n{}",
            note.title, note.id, note.created, note.updated, rendered
        );
    } else {
        println!(
            "# {} ({})\nCreated: {}\nUpdated: {}\n\n{}",
            note.title, note.id, note.created, note.updated, note.body
        );
    }
    Ok(())
}

fn edit_note(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut args_iter = args.into_iter();
    let id = args_iter.next().ok_or("Usage: qn edit <id> [-t <tag>]")?;
    let mut tag_filters: Vec<String> = Vec::new();
    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "-t" | "--tag" => {
                if let Some(v) = args_iter.next() {
                    let tag = normalize_tag(&v);
                    if !tag.is_empty() {
                        tag_filters.push(tag);
                    }
                } else {
                    return Err("Provide a tag after -t/--tag".into());
                }
            }
            other => return Err(format!("Unknown flag for edit: {other}").into()),
        }
    }
    let path = note_path(dir, &id);
    if !path.exists() {
        return Err(format!("Note {id} not found").into());
    }

    let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let mut used_popup = false;
    if has_fzf() {
        used_popup = launch_editor_popup(&editor, &path)?;
        // If popup was canceled, skip updating timestamp.
        if !used_popup {
            println!("Edit canceled.");
            return Ok(());
        }
    }
    if !used_popup {
        let status = Command::new(&editor)
            .arg(&path)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err("Editor exited with non-zero status".into());
        }
    }

    let size = fs::metadata(&path)?.len();
    let mut note = parse_note(&path, size)?;
    if !tag_filters.is_empty() && !note_has_tags(&note, &tag_filters) {
        return Err(format!("Note {id} does not have required tag(s)").into());
    }
    note.updated = timestamp_string();
    write_note(&note, dir)?;
    println!("Updated {}", note.id);
    Ok(())
}

fn delete_notes(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut use_fzf = false;
    let mut ids: Vec<String> = Vec::new();
    let mut tag_filters: Vec<String> = Vec::new();
    let mut iter = args.into_iter();
    while let Some(a) = iter.next() {
        if a == "--fzf" {
            use_fzf = true;
        } else if a == "-t" || a == "--tag" {
            if let Some(v) = iter.next() {
                let tag = normalize_tag(&v);
                if !tag.is_empty() {
                    tag_filters.push(tag);
                }
            } else {
                return Err("Provide a tag after -t/--tag".into());
            }
        } else {
            ids.push(a);
        }
    }

    if ids.is_empty() {
        if !use_fzf && !has_fzf() {
            return Err("Provide ids or install fzf / use --fzf for interactive delete".into());
        }
        let mut files = list_note_files(dir)?;
        if !tag_filters.is_empty() {
            files.retain(|(p, size)| {
                if let Ok(note) = parse_note(p, *size) {
                    note_has_tags(&note, &tag_filters)
                } else {
                    false
                }
            });
        }
        if files.is_empty() {
            println!("No notes to delete.");
            return Ok(());
        }
        if !has_fzf() {
            return Err("fzf not available; cannot launch interactive delete".into());
        }

        let input = files
            .iter()
            .map(|(p, _)| p.to_string_lossy())
            .collect::<Vec<_>>()
            .join("\n");

        let mut child = Command::new("fzf")
            .arg("--multi")
            .arg("--preview")
            .arg("sed -n '1,120p' {}")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(input.as_bytes())?;
        }
        let output = child.wait_with_output()?;
        if !output.status.success() || output.stdout.is_empty() {
            println!("No selection made; nothing deleted.");
            return Ok(());
        }
        let selected_paths = String::from_utf8_lossy(&output.stdout);
        ids = selected_paths
            .lines()
            .filter_map(|l| Path::new(l).file_stem()?.to_str())
            .map(|s| s.to_string())
            .collect();
    }

    if ids.is_empty() {
        println!("No notes deleted.");
        return Ok(());
    }

    let mut deleted = 0;
    for id in ids {
        let path = note_path(dir, &id);
        if path.exists() {
            if !tag_filters.is_empty() {
                let size = fs::metadata(&path)?.len();
                if let Ok(note) = parse_note(&path, size) {
                    if !note_has_tags(&note, &tag_filters) {
                        println!("Skipped {id} (missing tag filter)");
                        continue;
                    }
                }
            }
            fs::remove_file(&path)?;
            println!("Deleted {id}");
            deleted += 1;
        } else {
            println!("Note {id} not found");
        }
    }
    if deleted == 0 {
        println!("No notes deleted.");
    }
    Ok(())
}

fn delete_all_notes(dir: &Path) -> Result<(), Box<dyn Error>> {
    let files = list_note_files(dir)?;
    if files.is_empty() {
        println!("No notes to delete.");
        return Ok(());
    }
    for (path, _) in files {
        fs::remove_file(&path)?;
    }
    println!("Deleted all notes.");
    Ok(())
}

fn list_tags(dir: &Path) -> Result<(), Box<dyn Error>> {
    let pinned =
        env::var("QUICK_NOTES_PINNED_TAGS").unwrap_or_else(|_| PINNED_TAGS_DEFAULT.to_string());
    let pinned_tags: Vec<String> = pinned
        .split(',')
        .map(|t| normalize_tag(t.trim()))
        .filter(|t| !t.is_empty())
        .collect();

    #[derive(Default, Clone)]
    struct TagStat {
        count: usize,
        first: Option<DateTime<FixedOffset>>,
        last: Option<DateTime<FixedOffset>>,
    }

    let mut stats: std::collections::BTreeMap<String, TagStat> = std::collections::BTreeMap::new();
    for (path, size) in list_note_files(dir)? {
        if let Ok(note) = parse_note(&path, size) {
            let created = parse_timestamp(&note.created);
            let updated = parse_timestamp(&note.updated);
            for tag in note.tags {
                let entry = stats.entry(tag).or_default();
                entry.count += 1;
                if let Some(c) = created {
                    entry.first = match entry.first {
                        Some(f) => Some(f.min(c)),
                        None => Some(c),
                    };
                }
                if let Some(u) = updated {
                    entry.last = match entry.last {
                        Some(l) => Some(l.max(u)),
                        None => Some(u),
                    };
                }
            }
        }
    }

    for tag in pinned_tags {
        stats.entry(tag).or_insert(TagStat::default());
    }

    if stats.is_empty() {
        println!("No tags found.");
        return Ok(());
    }

    for (tag, stat) in stats {
        let first = stat
            .first
            .map(|d| d.to_rfc3339())
            .unwrap_or_else(|| "n/a".to_string());
        let last = stat
            .last
            .map(|d| d.to_rfc3339())
            .unwrap_or_else(|| "n/a".to_string());
        println!(
            "{:15} | count {:4} | first {} | last {}",
            tag, stat.count, first, last
        );
    }
    Ok(())
}

fn seed_notes(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        return Err("Usage: qn seed <count> [--chars N] [-t <tag> ...]".into());
    }
    let count: usize = args[0].parse().map_err(|_| "Count must be a number")?;
    let mut body_len: usize = 400;
    let mut tags: Vec<String> = Vec::new();
    let mut iter = args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--chars" => {
                if let Some(v) = iter.next() {
                    body_len = v.parse().map_err(|_| "chars must be a number")?;
                } else {
                    return Err("Provide a value for --chars".into());
                }
            }
            "-t" | "--tag" => {
                if let Some(v) = iter.next() {
                    let tag = normalize_tag(&v);
                    if !tag.is_empty() {
                        tags.push(tag);
                    }
                } else {
                    return Err("Provide a value after -t/--tag".into());
                }
            }
            other => return Err(format!("Unknown flag for seed: {other}").into()),
        }
    }

    for i in 0..count {
        let title = format!("Seed note {}", short_timestamp());
        let body = generate_body(body_len, i);
        let note = create_note(title, body, tags.clone(), dir)?;
        if (i + 1) % 50 == 0 || i + 1 == count {
            println!("Generated {}/{} (last id {})", i + 1, count, note.id);
        }
    }
    Ok(())
}

fn create_note_with_tags(
    title: String,
    body: String,
    tags: Vec<String>,
    dir: &Path,
) -> Result<Note, Box<dyn Error>> {
    create_note(title, body, tags, dir)
}

fn create_note(
    title: String,
    body: String,
    tags: Vec<String>,
    dir: &Path,
) -> Result<Note, Box<dyn Error>> {
    let mut tags: Vec<String> = tags
        .into_iter()
        .map(|t| normalize_tag(&t))
        .filter(|t| !t.is_empty())
        .collect();
    tags.sort();
    tags.dedup();

    let id = unique_id(dir)?;
    let now = timestamp_string();
    let mut note = Note {
        id: id.clone(),
        title,
        created: now.clone(),
        updated: now,
        body,
        tags,
        size_bytes: 0,
    };
    write_note(&note, dir)?;
    note.size_bytes = fs::metadata(note_path(dir, &note.id))?.len();
    Ok(note)
}

fn note_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.md"))
}

fn unique_id(dir: &Path) -> io::Result<String> {
    let base = Local::now().timestamp_micros().to_string();
    for suffix in 0..5000 {
        let candidate = if suffix == 0 {
            base.clone()
        } else {
            format!("{base}-{suffix}")
        };
        if !note_path(dir, &candidate).exists() {
            return Ok(candidate);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        "Could not generate unique id",
    ))
}

fn write_note(note: &Note, dir: &Path) -> io::Result<()> {
    let mut body = note.body.trim_end_matches('\n').to_string();
    body.push('\n');
    let tags_line = if note.tags.is_empty() {
        "Tags:".to_string()
    } else {
        format!("Tags: {}", note.tags.join(", "))
    };
    let content = format!(
        "Title: {}\nCreated: {}\nUpdated: {}\n{}\n---\n{}",
        note.title, note.created, note.updated, tags_line, body
    );
    fs::write(note_path(dir, &note.id), content)
}

fn parse_note(path: &Path, size_bytes: u64) -> io::Result<Note> {
    let raw = fs::read_to_string(path)?;
    let (header, body) = if let Some(idx) = raw.find("\n---\n") {
        let (h, rest) = raw.split_at(idx);
        (h.to_string(), rest[5..].to_string()) // skip the separator
    } else {
        ("".to_string(), raw)
    };

    let mut title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();
    let mut created = "unknown".to_string();
    let mut updated = "unknown".to_string();
    let mut tags: Vec<String> = Vec::new();

    for line in header.lines() {
        if let Some(val) = line.strip_prefix("Title:") {
            title = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("Created:") {
            created = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("Updated:") {
            updated = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("Tags:") {
            tags = val
                .split(',')
                .map(|t| normalize_tag(t.trim()))
                .filter(|t| !t.is_empty())
                .collect();
        }
    }

    Ok(Note {
        id: path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string(),
        title,
        created,
        updated,
        body,
        tags,
        size_bytes,
    })
}

fn short_timestamp() -> String {
    Local::now().timestamp_micros().to_string()
}

fn timestamp_string() -> String {
    let now = Local::now();
    now.format("%m/%d/%Y %I:%M %p %:z").to_string()
}

fn parse_timestamp(ts: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_str(ts, "%m/%d/%Y %I:%M %p %:z").ok()
}

fn cmp_dt(a: &str, b: &str) -> Ordering {
    let a_dt = parse_timestamp(a);
    let b_dt = parse_timestamp(b);
    match (a_dt, b_dt) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        _ => Ordering::Equal,
    }
}

fn has_fzf() -> bool {
    Command::new("fzf")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn list_note_files(dir: &Path) -> io::Result<Vec<(PathBuf, u64)>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.path().extension().and_then(|s| s.to_str()) == Some("md")
        {
            let size = entry.metadata()?.len();
            files.push((entry.path(), size));
        }
    }
    Ok(files)
}

fn preview_line(note: &Note) -> String {
    let first_line = note.body.lines().next().unwrap_or("").trim();
    let title = note.title.trim();
    // Suppress default auto-generated titles like "Quick note <id>" in the preview.
    let include_title = !title.to_lowercase().starts_with("quick note ");
    let mut text = if include_title {
        format!("{} {}", title, first_line).trim().to_string()
    } else {
        first_line.to_string()
    };
    const MAX_LEN: usize = 100;
    if text.chars().count() > MAX_LEN {
        text = text.chars().take(MAX_LEN).collect::<String>();
        text.push('…');
    }
    text
}

fn format_tags(tags: &[String], use_color: bool) -> String {
    if tags.is_empty() {
        return String::new();
    }
    let mut out = Vec::new();
    for tag in tags {
        if use_color {
            let (r, g, b) = color_for_tag(tag);
            out.push(Paint::rgb(tag.as_str(), r, g, b).bold().to_string());
        } else {
            out.push(tag.clone());
        }
    }
    out.join(" ")
}

fn color_for_tag(tag: &str) -> (u8, u8, u8) {
    const PALETTE: &[(u8, u8, u8)] = &[
        (137, 180, 250), // blue
        (166, 227, 161), // green
        (249, 226, 175), // yellow
        (245, 194, 231), // pink
        (255, 169, 167), // red
        (148, 226, 213), // teal
        (198, 160, 246), // purple
        (240, 198, 198), // rose
    ];
    let h = hash_tag(tag);
    PALETTE[(h as usize) % PALETTE.len()]
}

fn hash_tag(tag: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in tag.bytes() {
        h = (h.wrapping_shl(5)).wrapping_add(h) ^ u64::from(b);
    }
    h
}

fn split_tags(args: Vec<String>) -> (Vec<String>, Vec<String>) {
    let mut tags = Vec::new();
    let mut rest = Vec::new();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-t" | "--tag" => {
                if let Some(v) = iter.next() {
                    let tag = normalize_tag(&v);
                    if !tag.is_empty() {
                        tags.push(tag);
                    }
                }
            }
            _ => rest.push(arg),
        }
    }
    (tags, rest)
}

fn normalize_tag(t: &str) -> String {
    let trimmed = t.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.starts_with('#') {
        trimmed.to_string()
    } else {
        format!("#{}", trimmed)
    }
}

fn note_has_tags(note: &Note, tags: &[String]) -> bool {
    tags.iter().all(|t| note.tags.contains(t))
}

fn generate_body(len: usize, seed: usize) -> String {
    let base = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Proin aliquet, mauris nec facilisis rhoncus, nisl justo viverra dui, vitae placerat metus erat sit amet nunc. ";
    let mut out = String::new();
    let mut n = 0;
    while out.len() < len {
        out.push_str(base);
        out.push_str(&format!("Seed chunk {seed} idx {n}. "));
        n += 1;
    }
    out.truncate(len);
    out.push('\n');
    out
}

fn launch_editor_popup(editor: &str, path: &Path) -> io::Result<bool> {
    if !has_fzf() {
        return Ok(false);
    }
    let mut child = Command::new("fzf")
        .arg("--no-multi")
        .arg("--height")
        .arg("70%")
        .arg("--layout")
        .arg("reverse")
        .arg("--border")
        .arg("--preview")
        .arg("sed -n '1,160p' {}")
        .arg("--bind")
        .arg(format!("enter:execute({} {{}} < /dev/tty)+abort", editor))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(path.to_string_lossy().as_bytes())?;
        stdin.write_all(b"\n")?;
    }
    let status = child.wait()?;
    Ok(status.success())
}

fn render_markdown(input: &str, use_color: bool) -> String {
    let mut rendered = String::new();
    let mut list_depth: usize = 0;

    for event in Parser::new(input) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                rendered.push('\n');
                let mark = match level {
                    HeadingLevel::H1 => "# ",
                    HeadingLevel::H2 => "## ",
                    HeadingLevel::H3 => "### ",
                    HeadingLevel::H4 => "#### ",
                    HeadingLevel::H5 => "##### ",
                    _ => "###### ",
                };
                push_styled(&mut rendered, mark, Style::Heading, use_color);
            }
            Event::End(TagEnd::Heading(_)) => rendered.push('\n'),
            Event::Start(Tag::List(_)) => {
                list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                if list_depth > 0 {
                    list_depth -= 1;
                }
                rendered.push('\n');
            }
            Event::Start(Tag::Item) => {
                rendered.push_str(&"  ".repeat(list_depth.saturating_sub(1)));
                push_styled(&mut rendered, "- ", Style::Bullet, use_color);
            }
            Event::Text(t) | Event::Code(t) => {
                push_styled(&mut rendered, &t, Style::Body, use_color)
            }
            Event::SoftBreak | Event::HardBreak => rendered.push('\n'),
            Event::Rule => {
                push_styled(&mut rendered, "\n---\n", Style::Rule, use_color);
            }
            Event::Html(t) => rendered.push_str(&t),
            _ => {}
        }
    }

    rendered.trim().to_string()
}

#[derive(Clone, Copy)]
enum Style {
    Heading,
    Bullet,
    Rule,
    Body,
}

fn push_styled(buf: &mut String, text: &str, style: Style, use_color: bool) {
    if use_color {
        let painted = match style {
            Style::Heading => Paint::cyan(text).bold(),
            Style::Bullet => Paint::yellow(text).bold(),
            Style::Rule => Paint::new(text).dim(),
            Style::Body => Paint::new(text),
        };
        buf.push_str(&painted.to_string());
    } else {
        buf.push_str(text);
    }
}
