use chrono::{DateTime, FixedOffset, Local};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use terminal_size::{Width, terminal_size};
use yansi::Paint;

const TIME_FMT: &str = "%d%b%y %H:%M %:z";
const LEGACY_TIME_FMT: &str = "%m/%d/%Y %I:%M %p %:z";
const PINNED_TAGS_DEFAULT: &str = "#todo,#meeting,#scratch";
const ID_TS_WIDTH: usize = 9;

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

pub fn entry() -> Result<(), Box<dyn Error>> {
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
        "view" => view_note(args, &dir, true)?,
        "render" => view_note(args, &dir, true)?,
        "edit" => edit_note(args, &dir)?,
        "delete" => delete_notes(args, &dir)?,
        "migrate-ids" => migrate_ids(&dir)?,
        "seed" => seed_notes(args, &dir)?,
        "delete-all" => delete_all_notes(&dir)?,
        "tags" => list_tags(args, &dir)?,
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
                                  List notes (sort by created|updated|size; \
default updated desc)
  qn view <id> [--plain]          Show a note (rendered by default; disable \
color with --plain)
  qn edit <id> [-t|--tag <tag>]   Edit in $EDITOR (updates timestamp; requires \
tag match when provided)
  qn delete [ids...] [--fzf] [-t|--tag <tag>]
                                  Delete one or more notes (fzf multi-select \
when --fzf or no ids and fzf available; optional tag filter)
  qn migrate-ids                  Regenerate note IDs to the current shorter \
format
  qn delete-all                   Delete every note in the notes directory
  qn tags                         List tags with counts and first/last use
  qn seed <count> [--chars N]     Generate test notes (random body of N chars; \
default 400)
  qn path                         Show the notes directory
  qn completion zsh               Print zsh completion script for fzf-powered \
ids
  qn help                         Show this message

Environment:
  QUICK_NOTES_DIR                 Override notes directory \
(default: ~/.quick_notes)
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
        return Err(
            "Provide the note body after tags, e.g. `qn add \"text\" -t #tag`"
                .into(),
        );
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
    let mut relative_time = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--sort" => {
                if let Some(v) = iter.next() {
                    sort_field = v;
                } else {
                    return Err(
                        "Provide a sort field: created|updated|size".into()
                    );
                }
            }
            "--asc" => ascending = true,
            "--desc" => ascending = false,
            "-s" | "--search" => {
                if let Some(v) = iter.next() {
                    search = Some(v);
                } else {
                    return Err(
                        "Provide a search string after -s/--search".into()
                    );
                }
            }
            "-r" | "--relative" => {
                relative_time = true;
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
            n.title.to_lowercase().contains(&ql)
                || n.body.to_lowercase().contains(&ql)
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

    let now = now_fixed();
    let use_color = env::var("NO_COLOR").is_err();
    let previews: Vec<String> =
        notes.iter().map(|n| preview_for_list(n, search.as_deref())).collect();
    let tags_plain: Vec<String> =
        notes.iter().map(|n| n.tags.join(" ")).collect();
    let term_width = terminal_columns().unwrap_or(120);
    let widths = column_widths(
        &notes,
        &previews,
        &tags_plain,
        term_width,
        relative_time,
        &now,
    );

    print_list_header(&widths, use_color, relative_time, &now);

    for (idx, n) in notes.iter().enumerate() {
        let preview = &previews[idx];
        let preview_highlighted =
            highlight_search(preview, search.as_deref(), use_color);
        let line = format_list_row(
            &n.id,
            &n.updated,
            &preview_highlighted,
            if widths.include_tags { Some(n.tags.as_slice()) } else { None },
            &widths,
            use_color,
            relative_time,
            &now,
        );
        println!("{line}");
    }
    Ok(())
}

fn print_list_header(
    widths: &ColumnWidths,
    use_color: bool,
    relative: bool,
    now: &DateTime<FixedOffset>,
) {
    let updated_label = updated_label(relative);
    let tags_header: Option<Vec<String>> =
        if widths.include_tags { Some(vec!["Tags".to_string()]) } else { None };

    let header = format_list_row(
        "ID",
        &updated_label,
        "Preview",
        tags_header.as_deref(),
        widths,
        use_color,
        relative,
        now,
    );
    println!("{header}");
    println!("{}", "=".repeat(display_len(&header)));
}

#[derive(Debug)]
struct ColumnWidths {
    id: usize,
    updated: usize,
    preview: usize,
    tags: usize,
    include_tags: bool,
}

impl ColumnWidths {
    fn total(&self) -> usize {
        let spaces = if self.include_tags { 9 } else { 6 };
        let tags = if self.include_tags { self.tags } else { 0 };
        self.id + self.updated + self.preview + tags + spaces
    }
}

fn column_widths(
    notes: &[Note],
    previews: &[String],
    tags_plain: &[String],
    term_width: usize,
    relative: bool,
    now: &DateTime<FixedOffset>,
) -> ColumnWidths {
    let updated_label = updated_label(relative);
    let updated_data_width = notes
        .iter()
        .map(|n| {
            format_timestamp_table(&n.updated, relative, now).chars().count()
        })
        .max()
        .unwrap_or_else(|| updated_label.len().max("Updated".len()));
    let id_width = notes
        .iter()
        .map(|n| n.id.chars().count())
        .max()
        .unwrap_or(0)
        .max("ID".len());
    // Keep updated column tight to the widest value or header.
    let updated_width = updated_data_width.max(updated_label.len());
    let preview_width = previews
        .iter()
        .map(|p| p.chars().count())
        .max()
        .unwrap_or(0)
        .max("Preview".len());
    let include_tags = notes.iter().any(|n| !n.tags.is_empty());
    let tags_width = if include_tags {
        tags_plain
            .iter()
            .map(|t| t.chars().count())
            .max()
            .unwrap_or(0)
            .max("Tags".len())
    } else {
        0
    };

    let widths = ColumnWidths {
        id: id_width,
        updated: updated_width,
        preview: preview_width,
        tags: tags_width,
        include_tags,
    };

    shrink_widths(widths, term_width, relative)
}

fn terminal_columns() -> Option<usize> {
    if let Ok(cols) = env::var("COLUMNS") {
        if let Ok(v) = cols.parse::<usize>() {
            if v > 0 {
                return Some(v);
            }
        }
    }
    if let Some((Width(w), _)) = terminal_size() {
        if w > 0 {
            return Some(w as usize);
        }
    }
    None
}

fn shrink_widths(
    mut w: ColumnWidths,
    term_width: usize,
    relative: bool,
) -> ColumnWidths {
    let min_preview = 4;
    let min_tags = 4;
    let min_updated = updated_label(relative).len();
    let min_id = "ID".len();

    let reduce = |value: &mut usize, min: usize, excess: &mut isize| {
        if *excess <= 0 {
            return;
        }
        let reducible = (*value as isize - min as isize).max(0);
        let delta = reducible.min(*excess);
        *value = value.saturating_sub(delta as usize);
        *excess -= delta;
    };

    let mut excess = w.total() as isize - term_width as isize;
    if excess > 0 {
        reduce(&mut w.preview, min_preview, &mut excess);
    }
    if excess > 0 && w.include_tags {
        reduce(&mut w.tags, min_tags, &mut excess);
    }
    if excess > 0 {
        reduce(&mut w.updated, min_updated, &mut excess);
    }
    if excess > 0 {
        reduce(&mut w.id, min_id, &mut excess);
    }

    w
}

fn format_list_row(
    id: &str,
    updated: &str,
    preview: &str,
    tags: Option<&[String]>,
    widths: &ColumnWidths,
    use_color: bool,
    relative: bool,
    now: &DateTime<FixedOffset>,
) -> String {
    let id_plain = truncate_with_ellipsis(id, widths.id);
    let id_len = id_plain.chars().count();
    let id_display = format_id(&id_plain, use_color);

    let updated_plain = truncate_with_ellipsis(
        &format_timestamp_table(updated, relative, now),
        widths.updated,
    );
    let updated_len = updated_plain.chars().count();
    let updated_display = format_timestamp(&updated_plain, use_color);

    let preview_plain = truncate_with_ellipsis(preview, widths.preview);
    let preview_len = preview_plain.chars().count();
    let preview_display = preview_plain.clone();

    let (tags_display, tags_len) = if widths.include_tags {
        format_tags_clamped(tags.unwrap_or(&[]), widths.tags, use_color)
    } else {
        (String::new(), 0)
    };

    assemble_row(
        &id_display,
        id_len,
        &updated_display,
        updated_len,
        &preview_display,
        preview_len,
        if widths.include_tags {
            Some((&tags_display, tags_len))
        } else {
            None
        },
        widths,
    )
}

fn assemble_row(
    id_display: &str,
    id_len: usize,
    updated_display: &str,
    updated_len: usize,
    preview_display: &str,
    preview_len: usize,
    tags: Option<(&str, usize)>,
    widths: &ColumnWidths,
) -> String {
    let mut line = String::new();
    line.push_str(&pad_field(id_display, widths.id, id_len));
    line.push_str(" | ");
    line.push_str(&pad_field(updated_display, widths.updated, updated_len));
    line.push_str(" | ");
    line.push_str(&pad_field(preview_display, widths.preview, preview_len));
    if widths.include_tags {
        line.push_str(" | ");
        if let Some((tags_display, tags_len)) = tags {
            line.push_str(&pad_field(tags_display, widths.tags, tags_len));
        } else {
            line.push_str(&" ".repeat(widths.tags));
        }
    }
    line
}

fn pad_field(display: &str, target: usize, plain_len: usize) -> String {
    let mut out = display.to_string();
    let padding = target.saturating_sub(plain_len);
    out.push_str(&" ".repeat(padding));
    out
}

fn truncate_with_ellipsis(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let len = text.chars().count();
    if len <= max_width {
        return text.to_string();
    }
    if max_width == 1 {
        return "…".to_string();
    }
    let mut out =
        text.chars().take(max_width.saturating_sub(1)).collect::<String>();
    out.push('…');
    out
}

#[derive(Default)]
struct IdState {
    last_ts: i64,
    counter: u32,
}

fn encode_base62(num: u64) -> String {
    const ALPHABET: &[u8] =
        b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    if num == 0 {
        return "0".to_string();
    }
    let mut n = num;
    let base = ALPHABET.len() as u64;
    let mut out = Vec::new();
    while n > 0 {
        let idx = (n % base) as usize;
        out.push(ALPHABET[idx] as char);
        n /= base;
    }
    out.iter().rev().collect()
}

fn encode_base62_width(num: u64, width: usize) -> String {
    let base = encode_base62(num);
    if base.len() >= width {
        base
    } else {
        format!("{}{}", "0".repeat(width - base.len()), base)
    }
}

fn highlight_search(
    text: &str,
    query: Option<&str>,
    use_color: bool,
) -> String {
    let Some(q) = query else { return text.to_string() };
    if q.is_empty() {
        return text.to_string();
    }
    let q_lower = q.to_lowercase();
    let mut out = String::new();
    let mut remaining = text;
    while let Some(pos) = remaining.to_lowercase().find(&q_lower) {
        let (before, rest) = remaining.split_at(pos);
        let (matched, after) = rest.split_at(q.len().min(rest.len()));
        out.push_str(before);
        if use_color {
            out.push_str(&Paint::rgb(matched, 243, 139, 168).to_string());
        } else {
            out.push_str(matched);
        }
        remaining = after;
    }
    out.push_str(remaining);
    out
}

fn display_len(s: &str) -> usize {
    let mut len = 0;
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            while let Some(next) = chars.next() {
                if next == 'm' {
                    break;
                }
            }
            continue;
        }
        len += 1;
    }
    len
}

fn pad_ansi(text: &str, width: usize) -> String {
    let len = display_len(text);
    if len >= width {
        return text.to_string();
    }
    format!("{}{}", text, " ".repeat(width - len))
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

fn view_note(
    args: Vec<String>,
    dir: &Path,
    force_render: bool,
) -> Result<(), Box<dyn Error>> {
    let mut args_iter = args.into_iter();
    let mut ids: Vec<String> = Vec::new();
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
            other => {
                if other.starts_with('-') {
                    return Err(
                        format!("Unknown flag for view: {other}").into()
                    );
                }
                ids.push(other.to_string());
            }
        }
    }
    if ids.is_empty() {
        return Err(
            "Usage: qn view <id>... [--render|-r] [--plain] [-t <tag>]".into(),
        );
    }

    let use_color = !plain && env::var("NO_COLOR").is_err();
    let mut errors: Vec<String> = Vec::new();
    for (idx, id) in ids.iter().enumerate() {
        let path = note_path(dir, &id);
        if !path.exists() {
            errors.push(format!("Note {id} not found"));
            continue;
        }
        let size = fs::metadata(&path)?.len();
        let note = parse_note(&path, size)?;
        if !tag_filters.is_empty() && !note_has_tags(&note, &tag_filters) {
            errors.push(format!("Note {id} does not have required tag(s)"));
            continue;
        }
        let title_display = if use_color {
            Paint::rgb(&note.title, 249, 226, 175).bold().to_string()
        } else {
            note.title.clone()
        };
        let header = format!(
            "===== {} ({}) =====\n{} {}\n{} {}\n\n",
            title_display,
            format_id(&note.id, use_color),
            format_header_label("Created:", use_color),
            format_timestamp(&note.created, use_color),
            format_header_label("Updated:", use_color),
            format_timestamp(&note.updated, use_color)
        );

        if render && use_color {
            if let Some(colorizer) = detect_glow() {
                let raw_markdown = format!(
                    "# {} ({})\nCreated: {}\nUpdated: {}\n\n{}",
                    note.title, note.id, note.created, note.updated, note.body
                );
                let mut child = Command::new(colorizer)
                    .arg("-")
                    .stdin(Stdio::piped())
                    .spawn()?;
                if let Some(stdin) = child.stdin.as_mut() {
                    stdin.write_all(raw_markdown.as_bytes())?;
                }
                let status = child.wait()?;
                if status.success() {
                    if idx + 1 != ids.len() {
                        println!();
                    }
                    continue;
                }
            } else {
                eprintln!(
                    "Hint: install `glow` for rich markdown rendering \
(https://github.com/charmbracelet/glow)"
                );
            }
        }

        let body_for_output = if render {
            render_markdown(&note.body, use_color)
        } else {
            note.body.clone()
        };
        print!("{header}{body_for_output}");
        if idx + 1 != ids.len() {
            println!();
        }
    }
    if !errors.is_empty() {
        return Err(errors.remove(0).into());
    }
    Ok(())
}

fn edit_note(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut args_iter = args.into_iter();
    let mut ids: Vec<String> = Vec::new();
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
            other => {
                if other.starts_with('-') {
                    return Err(
                        format!("Unknown flag for edit: {other}").into()
                    );
                }
                ids.push(other.to_string());
            }
        }
    }
    if ids.is_empty() {
        if !has_fzf() {
            return Err("Usage: qn edit <id>... [-t <tag>]".into());
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
            println!("No notes to edit.");
            return Ok(());
        }

        let input = files
            .iter()
            .map(|(p, _)| p.to_string_lossy())
            .collect::<Vec<_>>()
            .join("\n");

        let mut child = Command::new("fzf")
            .arg("--multi")
            .arg("--height")
            .arg("70%")
            .arg("--layout")
            .arg("reverse")
            .arg("--preview")
            .arg("sed -n '1,120p' {}")
            .arg("--preview-window")
            .arg("down:wrap")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(input.as_bytes())?;
        }
        let output = child.wait_with_output()?;
        if !output.status.success() || output.stdout.is_empty() {
            println!("No selection made; nothing opened.");
            return Ok(());
        }
        let selected_paths = String::from_utf8_lossy(&output.stdout);
        ids = selected_paths
            .lines()
            .filter_map(|l| Path::new(l).file_stem()?.to_str())
            .map(|s| s.to_string())
            .collect();
    }

    let mut paths: Vec<(String, PathBuf)> = Vec::new();
    for id in ids {
        let path = note_path(dir, &id);
        if !path.exists() {
            eprintln!("Note {id} not found");
            continue;
        }
        if !tag_filters.is_empty() {
            let size = fs::metadata(&path)?.len();
            if let Ok(note) = parse_note(&path, size) {
                if !note_has_tags(&note, &tag_filters) {
                    eprintln!("Note {id} does not have required tag(s)");
                    continue;
                }
            }
        }
        paths.push((id, path));
    }

    if paths.is_empty() {
        return Err("No editable notes matched the criteria".into());
    }

    let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let status = Command::new(&editor)
        .args(paths.iter().map(|(_, p)| p))
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    if !status.success() {
        return Err("Editor exited with non-zero status".into());
    }

    for (id, path) in paths {
        let size = fs::metadata(&path)?.len();
        let mut note = parse_note(&path, size)?;
        if !tag_filters.is_empty() && !note_has_tags(&note, &tag_filters) {
            eprintln!("Skipped {id} (missing tag filter)");
            continue;
        }
        note.updated = timestamp_string();
        write_note(&note, dir)?;
        println!("Updated {}", note.id);
    }
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
            return Err(
                "Provide ids or install fzf / use --fzf for interactive delete"
                    .into(),
            );
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
            return Err(
                "fzf not available; cannot launch interactive delete".into()
            );
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

fn list_tags(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut search: Option<String> = None;
    let mut relative_time = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-s" | "--search" => {
                if let Some(v) = iter.next() {
                    search = Some(v);
                } else {
                    return Err(
                        "Provide a search string after -s/--search".into()
                    );
                }
            }
            "-r" | "--relative" => {
                relative_time = true;
            }
            other => {
                return Err(format!("Unknown flag for tags: {other}").into());
            }
        }
    }

    let pinned = env::var("QUICK_NOTES_PINNED_TAGS")
        .unwrap_or_else(|_| PINNED_TAGS_DEFAULT.to_string());
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

    let mut stats: std::collections::BTreeMap<String, TagStat> =
        std::collections::BTreeMap::new();
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

    if let Some(q) = &search {
        let ql = q.to_lowercase();
        stats.retain(|tag, _| tag.to_lowercase().contains(&ql));
    }

    if stats.is_empty() {
        println!("No tags found.");
        return Ok(());
    }

    let now = now_fixed();
    let use_color = env::var("NO_COLOR").is_err();
    let mut rows: Vec<(String, String, String, String)> = Vec::new();
    let header_color = |text: &str| {
        if use_color {
            format_header_label(text, true)
        } else {
            text.to_string()
        }
    };
    let first_label = if relative_time {
        "First".to_string()
    } else {
        determine_tz_label()
            .map(|t| format!("First ({t})"))
            .unwrap_or_else(|| "First".to_string())
    };
    let last_label = if relative_time {
        "Last".to_string()
    } else {
        determine_tz_label()
            .map(|t| format!("Last ({t})"))
            .unwrap_or_else(|| "Last".to_string())
    };
    rows.push((
        header_color("Tag"),
        header_color("Count"),
        header_color(&first_label),
        header_color(&last_label),
    ));

    let mut rows_raw: Vec<(String, TagStat)> = stats.into_iter().collect();
    rows_raw.sort_by(|a, b| {
        match (a.1.last, b.1.last) {
            (Some(la), Some(lb)) => lb.cmp(&la),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
        .then_with(|| b.1.count.cmp(&a.1.count))
        .then_with(|| a.0.cmp(&b.0))
    });

    for (tag, stat) in rows_raw {
        let first = stat
            .first
            .map(|d| {
                format_timestamp_table(&format_dt(&d), relative_time, &now)
            })
            .unwrap_or_else(|| "n/a".to_string());
        let last = stat
            .last
            .map(|d| {
                format_timestamp_table(&format_dt(&d), relative_time, &now)
            })
            .unwrap_or_else(|| "n/a".to_string());

        let is_empty = stat.count == 0;
        let tag_label = if is_empty {
            format_id(&tag, use_color)
        } else {
            format_tag_text(&tag, use_color)
        };
        let count_display = if is_empty {
            format_id(&stat.count.to_string(), use_color)
        } else {
            stat.count.to_string()
        };
        let first_display = if is_empty || first == "n/a" {
            format_id(&first, use_color)
        } else {
            format_timestamp(&first, use_color)
        };
        let last_display = if is_empty || last == "n/a" {
            format_id(&last, use_color)
        } else {
            format_timestamp(&last, use_color)
        };

        rows.push((tag_label, count_display, first_display, last_display));
    }

    let widths = (
        rows.iter().map(|r| display_len(&r.0)).max().unwrap_or(0),
        rows.iter().map(|r| display_len(&r.1)).max().unwrap_or(0),
        rows.iter().map(|r| display_len(&r.2)).max().unwrap_or(0),
        rows.iter().map(|r| display_len(&r.3)).max().unwrap_or(0),
    );

    for (i, row) in rows.into_iter().enumerate() {
        let (tag, count, first, last) = row;
        let line = format!(
            "{} | {} | {} | {}",
            pad_ansi(&tag, widths.0),
            pad_ansi(&count, widths.1),
            pad_ansi(&first, widths.2),
            pad_ansi(&last, widths.3),
        );
        if i == 1 {
            let sep_len = display_len(&line);
            println!("{}", "=".repeat(sep_len));
        }
        println!("{line}");
    }
    Ok(())
}

fn seed_notes(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        return Err(
            "Usage: qn seed <count> [--chars N] [-t <tag> ...] [--markdown]"
                .into(),
        );
    }
    let mut count: Option<usize> = None;
    let mut body_len: usize = 400;
    let mut tags: Vec<String> = Vec::new();
    let mut markdown = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--chars" => {
                if let Some(v) = iter.next() {
                    body_len =
                        v.parse().map_err(|_| "chars must be a number")?;
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
            "--markdown" => {
                markdown = true;
            }
            other => {
                if other.starts_with('-') {
                    return Err(
                        format!("Unknown flag for seed: {other}").into()
                    );
                }
                if count.is_none() {
                    count = Some(
                        other.parse().map_err(|_| "Count must be a number")?,
                    );
                }
            }
        }
    }
    let count = count.ok_or("Provide a count for seed")?;

    for i in 0..count {
        let title = format!("Seed note {}", short_timestamp());
        let body = if markdown {
            generate_markdown_body(i)
        } else {
            generate_body(body_len, i)
        };
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

fn migrate_ids(dir: &Path) -> Result<(), Box<dyn Error>> {
    let files = list_note_files(dir)?;
    if files.is_empty() {
        println!("No notes to migrate.");
        return Ok(());
    }

    let mut reserved: HashSet<String> = files
        .iter()
        .filter_map(|(p, _)| {
            p.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
        })
        .collect();
    let mut moves: Vec<(PathBuf, String)> = Vec::new();

    for (path, _) in files {
        let new_id = generate_new_id(dir, &mut reserved)?;
        reserved.insert(new_id.clone());
        moves.push((path, new_id));
    }

    for (old_path, new_id) in moves {
        let new_path = note_path(dir, &new_id);
        fs::rename(&old_path, &new_path)?;
        println!(
            "Migrated {} -> {}",
            old_path.file_stem().and_then(|s| s.to_str()).unwrap_or_default(),
            new_id
        );
    }

    Ok(())
}

fn note_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.md"))
}

fn unique_id(dir: &Path) -> io::Result<String> {
    generate_new_id(dir, &mut HashSet::new())
}

fn generate_new_id(
    dir: &Path,
    reserved: &mut HashSet<String>,
) -> io::Result<String> {
    static ID_STATE: OnceLock<Mutex<IdState>> = OnceLock::new();
    let state = ID_STATE.get_or_init(|| Mutex::new(IdState::default()));

    let mut guard = state.lock().unwrap();
    loop {
        let now = Local::now().timestamp_micros();
        let ts = if now <= guard.last_ts { guard.last_ts + 1 } else { now };

        if ts == guard.last_ts {
            guard.counter = guard.counter.saturating_add(1);
        } else {
            guard.last_ts = ts;
            guard.counter = 0;
        }

        let ts_enc = encode_base62_width(ts.max(0) as u64, ID_TS_WIDTH);
        let id = if guard.counter == 0 {
            ts_enc
        } else {
            format!("{ts_enc}{}", encode_base62(guard.counter as u64))
        };

        if !reserved.contains(&id) && !note_path(dir, &id).exists() {
            return Ok(id);
        }

        reserved.insert(id);
        guard.counter = guard.counter.saturating_add(1);
    }
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
    encode_base62_width(
        Local::now().timestamp_micros().max(0) as u64,
        ID_TS_WIDTH,
    )
}

fn now_fixed() -> DateTime<FixedOffset> {
    Local::now().with_timezone(Local::now().offset())
}

fn timestamp_string() -> String {
    let now = Local::now();
    now.format(TIME_FMT).to_string()
}

fn parse_timestamp(ts: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_str(ts, TIME_FMT)
        .or_else(|_| DateTime::parse_from_str(ts, LEGACY_TIME_FMT))
        .ok()
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
    if env::var("QUICK_NOTES_NO_FZF").is_ok() {
        return false;
    }
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
    let first_line =
        note.body.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    let title = note.title.trim();
    // Suppress default auto-generated titles like "Quick note <id>" when body
    // has content.
    let include_title = !title.to_lowercase().starts_with("quick note ");
    let mut text = if !first_line.is_empty() {
        if include_title {
            format!("{} {}", title, first_line).trim().to_string()
        } else {
            first_line.to_string()
        }
    } else if !title.is_empty() {
        title.to_string()
    } else {
        "[empty]".to_string()
    };
    const MAX_LEN: usize = 100;
    if text.chars().count() > MAX_LEN {
        text = text.chars().take(MAX_LEN).collect::<String>();
        text.push('…');
    }
    text
}

fn preview_for_list(note: &Note, search: Option<&str>) -> String {
    let Some(q) = search else { return preview_line(note) };
    if q.is_empty() {
        return preview_line(note);
    }
    let q_lower = q.to_lowercase();
    if note.title.to_lowercase().contains(&q_lower) {
        return preview_line(note);
    }

    let body_lower = note.body.to_lowercase();
    if let Some(pos) = body_lower.find(&q_lower) {
        let start = pos.saturating_sub(20);
        let end = (pos + q_lower.len() + 80).min(note.body.len());
        let start_byte = note
            .body
            .char_indices()
            .take_while(|(i, _)| *i < start)
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        let end_byte = note
            .body
            .char_indices()
            .take_while(|(i, _)| *i <= end)
            .last()
            .map(|(i, ch)| i + ch.len_utf8())
            .unwrap_or(note.body.len());

        let mut snippet = note.body[start_byte..end_byte]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if start_byte > 0 {
            snippet = format!("… {}", snippet);
        }
        if end_byte < note.body.len() {
            snippet.push('…');
        }
        if !note.title.trim().is_empty() {
            snippet =
                format!("{} {}", note.title.trim(), snippet).trim().to_string();
        }
        return truncate_with_ellipsis(&snippet, 100);
    }

    preview_line(note)
}

fn format_tags_clamped(
    tags: &[String],
    max_width: usize,
    use_color: bool,
) -> (String, usize) {
    if tags.is_empty() || max_width == 0 {
        return (String::new(), 0);
    }

    let mut parts: Vec<String> = Vec::new();
    let mut used = 0usize;

    for tag in tags {
        let tag_len = tag.chars().count();
        let sep_len = if parts.is_empty() { 0 } else { 1 };
        if used + sep_len + tag_len <= max_width {
            if sep_len == 1 {
                parts.push(" ".to_string());
            }
            parts.push(format_tag_text(tag, use_color));
            used += sep_len + tag_len;
        } else {
            let remaining = max_width.saturating_sub(used + sep_len);
            if remaining > 0 {
                if sep_len == 1 {
                    parts.push(" ".to_string());
                    used += sep_len;
                }
                let truncated = truncate_with_ellipsis(tag, remaining);
                parts.push(format_tag_text(&truncated, use_color));
                used += truncated.chars().count();
            }
            break;
        }
    }

    (parts.concat(), used)
}

fn format_tag_text(tag: &str, use_color: bool) -> String {
    if use_color {
        let (r, g, b) = color_for_tag(tag);
        Paint::rgb(tag, r, g, b).bold().to_string()
    } else {
        tag.to_string()
    }
}

fn format_dt(dt: &DateTime<FixedOffset>) -> String {
    dt.format(TIME_FMT).to_string()
}

fn color_for_tag(tag: &str) -> (u8, u8, u8) {
    const PALETTE: &[(u8, u8, u8)] = &[
        (137, 180, 250),
        (166, 227, 161),
        (249, 226, 175),
        (245, 194, 231),
        (255, 169, 167),
        (148, 226, 213),
        (198, 160, 246),
        (240, 198, 198),
        (244, 219, 214),
        (181, 232, 224),
        (135, 176, 249),
        (183, 189, 248),
        (201, 203, 255),
        (255, 214, 165),
        (179, 255, 171),
        (255, 201, 210),
        (196, 181, 255),
        (186, 225, 255),
        (255, 241, 173),
        (204, 255, 229),
        (255, 199, 190),
        (214, 182, 255),
        (255, 214, 235),
        (168, 237, 255),
        (238, 231, 220),
        (211, 228, 205),
        (255, 234, 190),
        (214, 200, 255),
        (255, 210, 198),
        (204, 246, 221),
        (255, 230, 214),
        (196, 222, 255),
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

fn format_id(id: &str, use_color: bool) -> String {
    if use_color {
        Paint::rgb(id, 108, 112, 134).to_string()
    } else {
        id.to_string()
    }
}

fn format_header_label(label: &str, use_color: bool) -> String {
    if use_color {
        Paint::rgb(label, 148, 226, 213).bold().to_string()
    } else {
        label.to_string()
    }
}

fn format_timestamp(ts: &str, use_color: bool) -> String {
    if use_color {
        Paint::rgb(ts, 137, 180, 250).to_string()
    } else {
        ts.to_string()
    }
}

fn format_timestamp_table(
    ts: &str,
    relative: bool,
    now: &DateTime<FixedOffset>,
) -> String {
    if let Some(dt) = parse_timestamp(ts) {
        if relative {
            return format_relative(dt, now);
        }
        return dt.format("%d%b%y %H:%M").to_string();
    }
    ts.split_whitespace().take(2).collect::<Vec<_>>().join(" ")
}

fn updated_label(relative: bool) -> String {
    if relative { "Updated".to_string() } else { updated_label_with_tz() }
}

fn updated_label_with_tz() -> String {
    determine_tz_label()
        .map(|tz| format!("Updated ({tz})"))
        .unwrap_or_else(|| "Updated".to_string())
}

fn determine_tz_label() -> Option<String> {
    parse_timestamp(&timestamp_string()).map(|dt| dt.offset().to_string())
}

fn format_relative(
    dt: DateTime<FixedOffset>,
    now: &DateTime<FixedOffset>,
) -> String {
    let dur = now.signed_duration_since(dt);
    let total_hours = dur.num_hours().max(0);
    let total_days = dur.num_days().max(0);
    if total_days < 30 {
        if total_days == 0 {
            return format!("{total_hours}h ago");
        }
        let hours = (total_hours - total_days * 24).max(0);
        if hours > 0 {
            format!("{total_days}d {hours}h ago")
        } else {
            format!("{total_days}d ago")
        }
    } else if total_days < 365 {
        let months = total_days / 30;
        let days = total_days % 30;
        if days > 0 {
            format!("{months}mo {days}d ago")
        } else {
            format!("{months}mo ago")
        }
    } else {
        let years = total_days / 365;
        let months = (total_days % 365) / 30;
        if months > 0 {
            format!("{years}y {months}mo ago")
        } else {
            format!("{years}y ago")
        }
    }
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
    let base = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Proin \
aliquet, mauris nec facilisis rhoncus, nisl justo viverra dui, vitae placerat \
metus erat sit amet nunc. ";
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

fn generate_markdown_body(seed: usize) -> String {
    format!(
        "# Heading {seed}\n\n\
## Subheading\n\n\
- bullet one\n- bullet two\n- bullet three\n\n\
1. ordered one\n2. ordered two\n\n\
**bold text** and _italic text_ with `inline code`.\n\n\
```rust\nfn example_{seed}() {{ println!(\"hello\"); }}\n```\n\n\
> Blockquote example {seed}\n\n\
---\n\n\
Link: [example](https://example.com)\n\n\
| Feature | Value |\n\
|---------|-------|\n\
| Seed    | {seed} |\n\
| Type    | Markdown |\n"
    )
}

fn render_markdown(input: &str, use_color: bool) -> String {
    if !use_color {
        return input.to_string();
    }

    let mut rendered = String::new();
    let mut in_code_block = false;

    for segment in input.split_inclusive('\n') {
        let (line, newline) = if let Some(stripped) = segment.strip_suffix('\n')
        {
            (stripped, "\n")
        } else {
            (segment, "")
        };
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") {
            rendered.push_str(&push_painted(line, Style::Code, true));
            rendered.push_str(newline);
            in_code_block = !in_code_block;
            continue;
        }

        if in_code_block {
            rendered.push_str(&push_painted(line, Style::Code, true));
            rendered.push_str(newline);
            continue;
        }

        let styled_line = if trimmed.starts_with('#') {
            push_painted(line, Style::Heading, true)
        } else if trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.starts_with("+ ")
            || trimmed
                .split_once('.')
                .map(|(a, _)| a.chars().all(|c| c.is_ascii_digit()))
                .unwrap_or(false)
        {
            push_painted(line, Style::Bullet, true)
        } else if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            push_painted(line, Style::Rule, true)
        } else {
            highlight_inline_code(line)
        };

        rendered.push_str(&styled_line);
        rendered.push_str(newline);
    }

    rendered
}

fn highlight_inline_code(line: &str) -> String {
    if !line.contains('`') {
        return line.to_string();
    }
    let mut out = String::new();
    let mut rest = line;

    while let Some(start) = rest.find('`') {
        let (before, after_tick) = rest.split_at(start);
        out.push_str(before);
        let after_tick = &after_tick[1..];
        if let Some(end) = after_tick.find('`') {
            let (code, after) = after_tick.split_at(end);
            out.push('`');
            out.push_str(&Paint::blue(code).to_string());
            out.push('`');
            rest = &after[1..];
        } else {
            out.push('`');
            out.push_str(after_tick);
            return out;
        }
    }
    out.push_str(rest);
    out
}

fn detect_glow() -> Option<&'static str> {
    if Command::new("glow")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .map_or(false, |s| s.success())
    {
        return Some("glow");
    }
    None
}

fn push_painted(text: &str, style: Style, use_color: bool) -> String {
    if !use_color {
        return text.to_string();
    }
    match style {
        Style::Heading => Paint::cyan(text).bold().to_string(),
        Style::Bullet => Paint::yellow(text).bold().to_string(),
        Style::Rule => Paint::new(text).dim().to_string(),
        Style::Code => Paint::blue(text).to_string(),
    }
}

#[derive(Clone, Copy)]
enum Style {
    Heading,
    Bullet,
    Rule,
    Code,
}
