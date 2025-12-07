use chrono::{DateTime, FixedOffset, Local};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use yansi::Paint;

#[derive(Debug, Clone)]
struct Note {
    id: String,
    title: String,
    created: String,
    updated: String,
    body: String,
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
        "list" => list_notes(&dir)?,
        "view" => view_note(args, &dir, false)?,
        "render" => view_note(args, &dir, true)?,
        "edit" => edit_note(args, &dir)?,
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
  qn list                         List notes
  qn view <id> [--render|-r] [--plain]
                                  Show a note (render markdown with --render; disable color with --plain)
  qn render <id>                  Same as: qn view <id> --render
  qn edit <id>                    Edit in $EDITOR (updates timestamp)
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
    let body = args.join(" ");
    let title = format!("Quick note {}", short_timestamp());
    let note = create_note(title, body, dir)?;
    println!("Added note {} ({})", note.id, note.title);
    Ok(())
}

fn new_note(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        return Err("Usage: qn new <title> [body]".into());
    }
    let title = args[0].clone();
    let body = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        String::new()
    };
    let note = create_note(title, body, dir)?;
    println!("Created note {} ({})", note.id, note.title);
    Ok(())
}

fn list_notes(dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut notes: Vec<(i64, Note)> = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.path().extension().and_then(|s| s.to_str()) == Some("md")
        {
            if let Ok(note) = parse_note(&entry.path()) {
                let sort_key = parse_timestamp(&note.updated)
                    .map(|d| d.timestamp())
                    .unwrap_or_default();
                notes.push((sort_key, note));
            }
        }
    }
    notes.sort_by(|a, b| b.0.cmp(&a.0));

    if notes.is_empty() {
        println!("No notes yet. Try `qn add \"text\"`.");
        return Ok(());
    }

    for (_, n) in notes {
        let preview = preview_line(&n);
        println!("{}  | {}  | {}", n.id, n.updated, preview);
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
    let id = args
        .get(0)
        .ok_or("Usage: qn view <id> [--render|-r] [--plain]")?;
    let render = force_render
        || args
            .iter()
            .any(|a| a == "--render" || a == "-r" || a == "render");
    let plain = args.iter().any(|a| a == "--plain");
    let use_color = !plain && env::var("NO_COLOR").is_err();
    let path = note_path(dir, id);
    if !path.exists() {
        return Err(format!("Note {id} not found").into());
    }
    let note = parse_note(&path)?;
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
    let id = args.get(0).ok_or("Usage: qn edit <id>")?;
    let path = note_path(dir, id);
    if !path.exists() {
        return Err(format!("Note {id} not found").into());
    }

    let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let status = Command::new(editor)
        .arg(&path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    if !status.success() {
        return Err("Editor exited with non-zero status".into());
    }

    let mut note = parse_note(&path)?;
    note.updated = timestamp_string();
    write_note(&note, dir)?;
    println!("Updated {}", note.id);
    Ok(())
}

fn create_note(title: String, body: String, dir: &Path) -> Result<Note, Box<dyn Error>> {
    let id = unique_id(dir)?;
    let now = timestamp_string();
    let note = Note {
        id: id.clone(),
        title,
        created: now.clone(),
        updated: now,
        body,
    };
    write_note(&note, dir)?;
    Ok(note)
}

fn note_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.md"))
}

fn unique_id(dir: &Path) -> io::Result<String> {
    let base = Local::now().format("%Y%m%d%H%M%S").to_string();
    for suffix in 0..1000 {
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
    let content = format!(
        "Title: {}\nCreated: {}\nUpdated: {}\n---\n{}",
        note.title, note.created, note.updated, body
    );
    fs::write(note_path(dir, &note.id), content)
}

fn parse_note(path: &Path) -> io::Result<Note> {
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

    for line in header.lines() {
        if let Some(val) = line.strip_prefix("Title:") {
            title = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("Created:") {
            created = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("Updated:") {
            updated = val.trim().to_string();
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
    })
}

fn short_timestamp() -> String {
    Local::now().format("%Y%m%d%H%M%S").to_string()
}

fn timestamp_string() -> String {
    let now = Local::now();
    now.format("%m/%d/%Y %I:%M %p %:z").to_string()
}

fn parse_timestamp(ts: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_str(ts, "%m/%d/%Y %I:%M %p %:z").ok()
}

fn preview_line(note: &Note) -> String {
    let mut text = format!(
        "{} {}",
        note.title.trim(),
        note.body.lines().next().unwrap_or("").trim()
    )
    .trim()
    .to_string();
    const MAX_LEN: usize = 100;
    if text.chars().count() > MAX_LEN {
        text = text.chars().take(MAX_LEN).collect::<String>();
        text.push('…');
    }
    text
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
