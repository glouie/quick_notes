//! Core library for the Quick Notes CLI.
//! Handles note storage, listing, rendering, and tag management.

//! Quick Notes core library.
//!
//! This crate houses the command-line implementation for `quick_notes` and the
//! `qn` symlink, providing fast note creation, listing, search, tagging, and
//! fuzzy selection via `fzf`. Notes are stored as UTF-8 markdown files under
//! `~/.quick_notes` (or `QUICK_NOTES_DIR`) and are addressed by second-level
//! timestamps to stay unique and stable. Rendering is terminal-native: plain
//! text by default, ANSI-colorized when allowed, and `glow` is used if present
//! for rich markdown output.
//!
//! The crate is intentionally a single module to keep the binary lean for
//! `cargo install`, but the code is split into focused helpers:
//! - `entry` parses top-level subcommands and dispatches to helpers (add/new,
//!   list/view/render/edit/delete/delete-all/seed/tags/path/help/completion).
//! - `Note` parsing/serialization lives in the `parse_note`, `write_note_file`,
//!   and `note_path` helpers.
//! - Rendering helpers (`render_markdown`, `highlight_inline_code`) keep line
//!   structure intact for tests while supporting colored output.
//! - CLI integration with fzf completion is provided via the `completion`
//!   handler and the shell script in `contrib/`.
//!
//! See `CONTRIBUTE.md` for architecture notes and development workflows, and
//! `AGENTS.md` for usage expectations that tests enforce.

mod help;
mod note;
mod render;
mod table;

#[derive(Clone, Copy)]
enum Area {
    Active,
    Trash,
    Archive,
}

use crate::note::{
    Note, TIME_FMT, cmp_dt, ensure_dir, generate_new_id, note_path, notes_dir,
    now_fixed, parse_note, parse_timestamp, short_timestamp, timestamp_string,
    unique_id, write_note,
};
use crate::render::{detect_glow, render_markdown};
use crate::table::{
    display_len, pad_field, render_table, truncate_with_ellipsis,
};
use chrono::{DateTime, FixedOffset};
use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use terminal_size::{Height, Width, terminal_size};
use yansi::Paint;

const PINNED_TAGS_DEFAULT: &str = "#todo,#meeting,#scratch";

/// Dispatch CLI arguments to the right subcommand.
pub fn entry() -> Result<(), Box<dyn Error>> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        return help::run(Vec::new());
    }

    let cmd = args.remove(0);
    let dir = notes_dir()?;
    ensure_dir(&dir)?;

    match cmd.as_str() {
        "-h" | "--help" => help::run(args)?,
        "add" => quick_add(args, &dir)?,
        "new" => new_note(args, &dir)?,
        "list" => list_notes(args, &dir)?,
        "view" => view_note(args, &dir, true)?,
        "render" => view_note(args, &dir, true)?,
        "edit" => edit_note(args, &dir)?,
        "delete" => delete_notes(args, &dir)?,
        "list-deleted" => list_deleted(args, &dir)?,
        "list-archived" => list_archived(args, &dir)?,
        "archive" => archive_notes(args, &dir)?,
        "undelete" => undelete_notes(args, &dir)?,
        "unarchive" => unarchive_notes(args, &dir)?,
        "migrate-ids" => migrate_ids(&dir)?,
        "seed" => seed_notes(args, &dir)?,
        "delete-all" => delete_all_notes(&dir)?,
        "tags" => list_tags(args, &dir)?,
        "stats" => stats(&dir)?,
        "path" => println!("{}", dir.display()),
        "completion" => print_completion(args)?,
        "help" => help::run(args)?,
        "guide" => help::run_guides(args)?,
        other => {
            eprintln!("Unknown command: {other}");
            help::run(Vec::new())?;
        }
    }

    Ok(())
}

/// Append text to an existing note (requires an id).
fn quick_add(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    if args.len() < 2 {
        return Err("Usage: qn add <id> \"text to append\"".into());
    }
    let id = args[0].clone();
    let text = args[1..].join(" ");
    if text.trim().is_empty() {
        return Err("Provide text to append".into());
    }
    let path = note_path(dir, &id);
    if !path.exists() {
        return Err(format!("Note {id} not found").into());
    }
    let size = fs::metadata(&path)?.len();
    let mut note = parse_note(&path, size)?;
    if !note.body.ends_with('\n') {
        note.body.push('\n');
    }
    note.body.push_str(text.trim());
    note.body.push('\n');
    note.updated = timestamp_string();
    write_note(&note, dir)?;
    println!("Appended to {id}");
    Ok(())
}

/// Handle `qn new`, creating a note with explicit title/body and tags.
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

/// List notes with sorting, search, optional tag filters, and relative time.
fn area_dir(base: &Path, area: Area) -> PathBuf {
    match area {
        Area::Active => base.to_path_buf(),
        Area::Trash => base.join("trash"),
        Area::Archive => base.join("archive"),
    }
}

fn list_notes(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    list_notes_in(args, dir, Area::Active)
}

fn list_deleted(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    list_notes_in(args, &area_dir(dir, Area::Trash), Area::Trash)
}

fn list_archived(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    list_notes_in(args, &area_dir(dir, Area::Archive), Area::Archive)
}

fn list_notes_in(
    args: Vec<String>,
    dir: &Path,
    area: Area,
) -> Result<(), Box<dyn Error>> {
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

    ensure_dir(dir)?;
    if let Area::Trash = area {
        let _ = clean_trash(dir);
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
        match area {
            Area::Active => println!("No notes yet. Try `qn add \"text\"`."),
            Area::Trash => println!("No deleted notes."),
            Area::Archive => println!("No archived notes."),
        }
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
        area,
    );

    let mut lines: Vec<String> = Vec::new();
    let header_preview =
        truncate_with_ellipsis("Preview", widths.preview).to_string();
    let header_preview_len = display_len(&header_preview);
    let header_tags: Option<Vec<String>> =
        if widths.include_tags { Some(vec!["Tags".to_string()]) } else { None };
    let created_header =
        widths.include_created.then(|| time_label("Created", relative_time));
    let moved_header = if widths.include_moved {
        match area {
            Area::Trash => Some(time_label("Deleted", relative_time)),
            Area::Archive => Some(time_label("Archived", relative_time)),
            Area::Active => None,
        }
    } else {
        None
    };
    let header = format_list_row(
        "ID",
        created_header.as_deref(),
        &updated_label(relative_time),
        moved_header.as_deref(),
        &header_preview,
        header_preview_len,
        header_tags.as_deref(),
        &widths,
        use_color,
        relative_time,
        &now,
        area,
        true,
    );
    lines.push(header.clone());
    lines.push("=".repeat(display_len(&header)));
    for (idx, n) in notes.iter().enumerate() {
        let preview_raw =
            truncate_with_ellipsis(&previews[idx], widths.preview);
        let preview_len = display_len(&preview_raw);
        let preview_highlighted =
            highlight_search(&preview_raw, search.as_deref(), use_color);
        let created = if widths.include_created {
            Some(n.created.as_str())
        } else {
            None
        };
        let moved = if widths.include_moved {
            match area {
                Area::Trash => {
                    n.deleted_at.as_deref().or_else(|| Some(n.updated.as_str()))
                }
                Area::Archive => n
                    .archived_at
                    .as_deref()
                    .or_else(|| Some(n.updated.as_str())),
                Area::Active => None,
            }
        } else {
            None
        };
        let line = format_list_row(
            &n.id,
            created,
            &n.updated,
            moved,
            &preview_highlighted,
            preview_len,
            if widths.include_tags { Some(n.tags.as_slice()) } else { None },
            &widths,
            use_color,
            relative_time,
            &now,
            area,
            false,
        );
        lines.push(line);
    }
    paginate_and_print(&lines)?;
    Ok(())
}

#[derive(Debug)]
struct ColumnWidths {
    id: usize,
    updated: usize,
    created: usize,
    moved: usize,
    preview: usize,
    tags: usize,
    include_tags: bool,
    include_created: bool,
    include_moved: bool,
}

impl ColumnWidths {
    fn total(&self) -> usize {
        let separator_count = 2
            + self.include_created as usize
            + self.include_moved as usize
            + self.include_tags as usize;
        let spaces = separator_count * 3;
        let created = if self.include_created { self.created } else { 0 };
        let moved = if self.include_moved { self.moved } else { 0 };
        let tags = if self.include_tags { self.tags } else { 0 };
        self.id + created + self.updated + moved + self.preview + tags + spaces
    }
}

fn column_widths(
    notes: &[Note],
    previews: &[String],
    tags_plain: &[String],
    term_width: usize,
    relative: bool,
    now: &DateTime<FixedOffset>,
    area: Area,
) -> ColumnWidths {
    let updated_label = updated_label(relative);
    let updated_data_width = notes
        .iter()
        .map(|n| {
            display_timestamp(area, &n.updated, relative, now).chars().count()
        })
        .max()
        .unwrap_or_else(|| updated_label.len().max("Updated".len()));
    let include_created = matches!(area, Area::Trash | Area::Archive);
    let created_label = time_label("Created", relative);
    let created_width = if include_created {
        notes
            .iter()
            .map(|n| {
                format_timestamp_table(&n.created, relative, now)
                    .chars()
                    .count()
            })
            .max()
            .unwrap_or(0)
            .max(created_label.len())
    } else {
        0
    };
    let include_moved = matches!(area, Area::Trash | Area::Archive);
    let moved_label = match area {
        Area::Trash => time_label("Deleted", relative),
        Area::Archive => time_label("Archived", relative),
        Area::Active => time_label("Moved", relative),
    };
    let moved_width = if include_moved {
        notes
            .iter()
            .map(|n| {
                moved_ts(area, n)
                    .map(|ts| {
                        display_timestamp_moved(area, ts, relative, now)
                            .chars()
                            .count()
                    })
                    .unwrap_or(0)
            })
            .max()
            .unwrap_or(0)
            .max(moved_label.len())
    } else {
        0
    };
    let mut timestamp_width = updated_data_width.max(updated_label.len());
    if include_created {
        timestamp_width =
            timestamp_width.max(created_width).max(created_label.len());
    }
    if include_moved {
        timestamp_width =
            timestamp_width.max(moved_width).max(moved_label.len());
    }
    let id_width = notes
        .iter()
        .map(|n| n.id.chars().count())
        .max()
        .unwrap_or(0)
        .max("ID".len());
    // Keep updated column tight to the widest value or header.
    let updated_width = timestamp_width;
    let created_width = if include_created { timestamp_width } else { 0 };
    let moved_width = if include_moved { timestamp_width } else { 0 };
    let moved_width = moved_width.max(moved_label.len());
    let preview_width = previews
        .iter()
        .map(|p| p.chars().count())
        .max()
        .unwrap_or(0)
        .max("Preview".len());
    let include_tags = if matches!(area, Area::Trash | Area::Archive) {
        false
    } else {
        notes.iter().any(|n| !n.tags.is_empty())
    };
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
        created: created_width,
        moved: moved_width,
        preview: preview_width,
        tags: tags_width,
        include_tags,
        include_created,
        include_moved,
    };

    shrink_widths(widths, term_width, relative, area)
}

pub(crate) fn terminal_columns() -> Option<usize> {
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

fn terminal_rows() -> Option<usize> {
    if let Ok(rows) = env::var("ROWS") {
        if let Ok(v) = rows.parse::<usize>() {
            if v > 0 {
                return Some(v);
            }
        }
    }
    if let Some((_, Height(h))) = terminal_size() {
        if h > 0 {
            return Some(h as usize);
        }
    }
    None
}

fn shrink_widths(
    mut w: ColumnWidths,
    term_width: usize,
    relative: bool,
    area: Area,
) -> ColumnWidths {
    let min_preview = 4;
    let min_tags = 4;
    let min_updated = updated_label(relative).len();
    let min_created = time_label("Created", relative).len();
    let min_moved = match area {
        Area::Trash => time_label("Deleted", relative).len(),
        Area::Archive => time_label("Archived", relative).len(),
        Area::Active => time_label("Moved", relative).len(),
    };
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
    if excess > 0 && w.include_moved {
        reduce(&mut w.moved, min_moved, &mut excess);
    }
    if excess > 0 && w.include_created {
        reduce(&mut w.created, min_created, &mut excess);
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

pub(crate) fn paginate_and_print(lines: &[String]) -> io::Result<()> {
    if !io::stdout().is_terminal() {
        for l in lines {
            println!("{l}");
        }
        return Ok(());
    }

    let rows = terminal_rows().unwrap_or(usize::MAX);
    if rows == usize::MAX || rows == 0 {
        for l in lines {
            println!("{l}");
        }
        return Ok(());
    }

    let page = rows.saturating_sub(2).max(1);
    let mut idx = 0;
    while idx < lines.len() {
        let end = (idx + page).min(lines.len());
        for l in &lines[idx..end] {
            println!("{l}");
        }
        idx = end;
        if idx < lines.len() {
            print!("-- more -- press Enter to continue --");
            io::stdout().flush()?;
            let mut buf = String::new();
            io::stdin().read_line(&mut buf)?;
        }
    }
    Ok(())
}

fn format_list_row(
    id: &str,
    created: Option<&str>,
    updated: &str,
    moved: Option<&str>,
    preview_display: &str,
    preview_len: usize,
    tags: Option<&[String]>,
    widths: &ColumnWidths,
    use_color: bool,
    relative: bool,
    now: &DateTime<FixedOffset>,
    area: Area,
    is_header: bool,
) -> String {
    let id_plain = truncate_with_ellipsis(id, widths.id);
    let id_len = display_len(&id_plain);
    let id_display = if is_header {
        format_header_label(&id_plain, use_color)
    } else {
        format_id(&id_plain, use_color)
    };

    let (created_display, created_len) = if widths.include_created {
        if let Some(c) = created {
            let created_source = if is_header {
                c.to_string()
            } else {
                format_timestamp_table(c, relative, now)
            };
            let created_plain =
                truncate_with_ellipsis(&created_source, widths.created);
            let len = display_len(&created_plain);
            let disp = if is_header {
                format_header_label(&created_plain, use_color)
            } else {
                format_timestamp(&created_plain, use_color)
            };
            (Some(disp), len)
        } else {
            (Some(String::new()), 0)
        }
    } else {
        (None, 0)
    };

    let updated_source = if is_header {
        updated.to_string()
    } else {
        display_timestamp(area, updated, relative, now)
    };
    let updated_plain = truncate_with_ellipsis(&updated_source, widths.updated);
    let updated_len = display_len(&updated_plain);
    let updated_display = if is_header {
        format_header_label(&updated_plain, use_color)
    } else {
        format_timestamp(&updated_plain, use_color)
    };

    let moved_source_holder;
    let (moved_display, moved_len) = if widths.include_moved {
        if let Some(m) = moved {
            moved_source_holder = if is_header {
                m.to_string()
            } else {
                display_timestamp_moved(area, m, relative, now)
            };
            let moved_plain =
                truncate_with_ellipsis(&moved_source_holder, widths.moved);
            let len = display_len(&moved_plain);
            let disp = if is_header {
                format_header_label(&moved_plain, use_color)
            } else {
                format_timestamp(&moved_plain, use_color)
            };
            (Some(disp), len)
        } else {
            (Some(String::new()), 0)
        }
    } else {
        (None, 0)
    };

    let preview_holder;
    let preview_for_row = if is_header {
        preview_holder = format_header_label(preview_display, use_color);
        &preview_holder
    } else {
        preview_display
    };

    let (tags_display, tags_len) = if widths.include_tags {
        format_tags_clamped(tags.unwrap_or(&[]), widths.tags, use_color)
    } else {
        (String::new(), 0)
    };

    assemble_row(
        &id_display,
        id_len,
        created_display.as_deref().map(|s| (s, created_len)),
        &updated_display,
        updated_len,
        moved_display.as_deref().map(|s| (s, moved_len)),
        preview_for_row,
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
    created_display: Option<(&str, usize)>,
    updated_display: &str,
    updated_len: usize,
    moved_display: Option<(&str, usize)>,
    preview_display: &str,
    preview_len: usize,
    tags: Option<(&str, usize)>,
    widths: &ColumnWidths,
) -> String {
    let mut line = String::new();
    line.push_str(&pad_field(id_display, widths.id, id_len));
    line.push_str(" | ");
    if let Some((created, len)) = created_display {
        line.push_str(&pad_field(created, widths.created, len));
        line.push_str(" | ");
    }
    line.push_str(&pad_field(updated_display, widths.updated, updated_len));
    line.push_str(" | ");
    if let Some((mv, len)) = moved_display {
        line.push_str(&pad_field(mv, widths.moved, len));
        line.push_str(" | ");
    }
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

/// Render or show raw notes; supports multiple ids, tag guard, and fzf.
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
            "--plain" | "-p" => plain = true,
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
            "Usage: qn view <id>... [--render|-r] [--plain|-p] [-t <tag>]"
                .into(),
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

/// Edit one or more notes, with optional tag guard and fzf multi-select.
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

/// Delete notes by id or via fzf multi-select; supports tag guards.
fn delete_notes(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut use_fzf = false;
    let mut ids: Vec<String> = Vec::new();
    let mut tag_filters: Vec<String> = Vec::new();
    let trash_dir = area_dir(dir, Area::Trash);
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

    ensure_dir(&trash_dir)?;
    clean_trash(&trash_dir)?;

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
        if !path.exists() {
            println!("Note {id} not found");
            continue;
        }
        if !tag_filters.is_empty() {
            let size = fs::metadata(&path)?.len();
            if let Ok(note) = parse_note(&path, size) {
                if !note_has_tags(&note, &tag_filters) {
                    println!("Skipped {id} (missing tag filter)");
                    continue;
                }
            }
        }
        move_note_with_timestamp(dir, &trash_dir, &id, Area::Trash)?;
        println!("Moved {id} to trash");
        deleted += 1;
    }
    if deleted == 0 {
        println!("No notes deleted.");
    }
    Ok(())
}

/// Remove every note in the current notes directory.
fn delete_all_notes(dir: &Path) -> Result<(), Box<dyn Error>> {
    let trash_dir = area_dir(dir, Area::Trash);
    ensure_dir(&trash_dir)?;
    clean_trash(&trash_dir)?;
    let files = list_note_files(dir)?;
    if files.is_empty() {
        println!("No notes to delete.");
        return Ok(());
    }
    for (path, _) in files {
        if let Some(id) = path.file_stem().and_then(|s| s.to_str()) {
            move_note_with_timestamp(dir, &trash_dir, id, Area::Trash)?;
        }
    }
    println!("Moved all notes to trash.");
    Ok(())
}

fn archive_notes(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut use_fzf = false;
    let mut ids: Vec<String> = Vec::new();
    let mut iter = args.into_iter();
    while let Some(a) = iter.next() {
        if a == "--fzf" {
            use_fzf = true;
        } else {
            ids.push(a);
        }
    }

    let archive_dir = area_dir(dir, Area::Archive);
    ensure_dir(&archive_dir)?;

    if ids.is_empty() {
        if !use_fzf && !has_fzf() {
            return Err(
                "Provide ids or install fzf / use --fzf for interactive archive"
                    .into(),
            );
        }
        if !has_fzf() {
            return Err(
                "fzf not available; cannot launch interactive archive".into()
            );
        }

        let files = list_note_files(dir)?;
        if files.is_empty() {
            println!("No notes to archive.");
            return Ok(());
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
            println!("No selection made; nothing archived.");
            return Ok(());
        }
        let selected_paths = String::from_utf8_lossy(&output.stdout);
        ids = selected_paths
            .lines()
            .filter_map(|l| Path::new(l).file_stem()?.to_str())
            .map(|s| s.to_string())
            .collect();
    }

    let mut moved = 0;
    for id in ids {
        let src = note_path(dir, &id);
        if !src.exists() {
            println!("Note {id} not found");
            continue;
        }
        move_note_with_timestamp(dir, &archive_dir, &id, Area::Archive)?;
        println!("Archived {id}");
        moved += 1;
    }
    if moved == 0 {
        println!("No notes archived.");
    }
    Ok(())
}

fn undelete_notes(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        return Err("Usage: qn undelete <id>...".into());
    }
    let trash_dir = area_dir(dir, Area::Trash);
    ensure_dir(&trash_dir)?;
    clean_trash(&trash_dir)?;
    let mut restored = 0;
    for id in args {
        match restore_note(&id, &trash_dir, dir) {
            Ok(new_id) => {
                println!("Restored {new_id}");
                restored += 1;
            }
            Err(e) => eprintln!("{e}"),
        }
    }
    if restored == 0 {
        println!("No notes restored.");
    }
    Ok(())
}

fn unarchive_notes(
    args: Vec<String>,
    dir: &Path,
) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        return Err("Usage: qn unarchive <id>...".into());
    }
    let archive_dir = area_dir(dir, Area::Archive);
    ensure_dir(&archive_dir)?;
    let mut restored = 0;
    for id in args {
        match restore_note(&id, &archive_dir, dir) {
            Ok(new_id) => {
                println!("Unarchived {new_id}");
                restored += 1;
            }
            Err(e) => eprintln!("{e}"),
        }
    }
    if restored == 0 {
        println!("No notes unarchived.");
    }
    Ok(())
}

/// Show tags with counts and first/last usage; supports search and relative time.
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
    let mut rows_raw: Vec<(String, TagStat)> = stats.into_iter().collect();
    let mut rows: Vec<(String, String, String, String)> = Vec::new();
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

    let headers = vec![
        header_color("Tag"),
        header_color("Count"),
        header_color(&first_label),
        header_color(&last_label),
    ];
    let rows_render: Vec<Vec<String>> =
        rows.into_iter().map(|(t, c, f, l)| vec![t, c, f, l]).collect();
    let table = render_table(&headers, &rows_render);
    let lines: Vec<String> = table.lines().map(|l| l.to_string()).collect();
    paginate_and_print(&lines)?;
    Ok(())
}

/// Generate bulk test notes with optional markdown bodies and tags.
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
        deleted_at: None,
        archived_at: None,
        body,
        tags,
        size_bytes: 0,
    };
    write_note(&note, dir)?;
    note.size_bytes = fs::metadata(note_path(dir, &note.id))?.len();
    Ok(note)
}

/// Rewrite existing note filenames to the short incremental id scheme.
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

fn trash_retention_days() -> i64 {
    env::var("QUICK_NOTES_TRASH_RETENTION_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v: &i64| *v >= 0)
        .unwrap_or(30)
}

fn clean_trash(trash_dir: &Path) -> io::Result<()> {
    let retention = trash_retention_days();
    if retention == 0 {
        return Ok(());
    }
    let cutoff = now_fixed() - chrono::Duration::days(retention);
    for (path, size) in list_note_files(trash_dir)? {
        if let Ok(note) = parse_note(&path, size) {
            let ts_str = note.deleted_at.as_deref().unwrap_or(&note.updated);
            if let Some(ts) = parse_timestamp(ts_str) {
                if ts < cutoff {
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }
    Ok(())
}

fn restore_note(
    id: &str,
    from_dir: &Path,
    to_dir: &Path,
) -> Result<String, Box<dyn Error>> {
    let src = note_path(from_dir, id);
    if !src.exists() {
        return Err(format!("Note {id} not found").into());
    }
    ensure_dir(to_dir)?;
    let mut final_id = id.to_string();
    let dst = note_path(to_dir, &final_id);
    let size = fs::metadata(&src)?.len();
    if dst.exists() {
        let mut reserved = HashSet::new();
        let new_id = generate_new_id(to_dir, &mut reserved)?;
        let mut note = parse_note(&src, size)?;
        note.id = new_id.clone();
        note.deleted_at = None;
        note.archived_at = None;
        write_note(&note, to_dir)?;
        fs::remove_file(src)?;
        final_id = new_id;
    } else {
        let mut note = parse_note(&src, size)?;
        note.deleted_at = None;
        note.archived_at = None;
        write_note(&note, to_dir)?;
        fs::rename(src, &dst)?;
    }
    Ok(final_id)
}

fn move_note_with_timestamp(
    from_dir: &Path,
    to_dir: &Path,
    id: &str,
    area: Area,
) -> Result<(), Box<dyn Error>> {
    let src = note_path(from_dir, id);
    if !src.exists() {
        return Err(format!("Note {id} not found").into());
    }
    let size = fs::metadata(&src)?.len();
    let mut note = parse_note(&src, size)?;
    match area {
        Area::Trash => {
            note.deleted_at = Some(timestamp_string());
            note.archived_at = None;
        }
        Area::Archive => {
            note.archived_at = Some(timestamp_string());
            note.deleted_at = None;
        }
        Area::Active => {
            note.deleted_at = None;
            note.archived_at = None;
        }
    }
    ensure_dir(to_dir)?;
    write_note(&note, to_dir)?;
    fs::remove_file(src)?;
    Ok(())
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
    time_label("Updated", relative)
}

fn time_label(base: &str, relative: bool) -> String {
    if relative {
        base.to_string()
    } else {
        determine_tz_label()
            .map(|tz| format!("{base} ({tz})"))
            .unwrap_or_else(|| base.to_string())
    }
}

fn display_timestamp(
    area: Area,
    ts: &str,
    relative: bool,
    now: &DateTime<FixedOffset>,
) -> String {
    match area {
        Area::Active => format_timestamp_table(ts, relative, now),
        Area::Trash | Area::Archive => match parse_timestamp(ts) {
            Some(dt) => {
                if relative {
                    format_relative(dt, now)
                } else {
                    dt.format("%d%b%y %H:%M").to_string()
                }
            }
            None => String::new(),
        },
    }
}

fn display_timestamp_moved(
    _area: Area,
    ts: &str,
    relative: bool,
    now: &DateTime<FixedOffset>,
) -> String {
    format_timestamp_table(ts, relative, now)
}

fn moved_ts<'a>(area: Area, note: &'a Note) -> Option<&'a str> {
    match area {
        Area::Trash => note.deleted_at.as_deref(),
        Area::Archive => note.archived_at.as_deref(),
        Area::Active => None,
    }
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

fn stats(dir: &Path) -> Result<(), Box<dyn Error>> {
    let active = list_note_files(dir)?.len();
    let trash_dir = area_dir(dir, Area::Trash);
    let archive_dir = area_dir(dir, Area::Archive);
    ensure_dir(&trash_dir)?;
    ensure_dir(&archive_dir)?;
    let trashed = list_note_files(&trash_dir)?.len();
    let archived = list_note_files(&archive_dir)?.len();

    let headers = vec!["Area".to_string(), "Count".to_string()];
    let rows = vec![
        vec!["Active".to_string(), active.to_string()],
        vec!["Trash".to_string(), trashed.to_string()],
        vec!["Archive".to_string(), archived.to_string()],
    ];
    let table = render_table(&headers, &rows);
    println!("{table}");
    Ok(())
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
