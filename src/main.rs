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

#[derive(Debug, Clone)]
struct Note {
    id: String,
    title: String,
    created: String,
    updated: String,
    body: String,
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
  qn delete [ids...] [--fzf]      Delete one or more notes (fzf multi-select when --fzf or no ids and fzf available)
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

fn list_notes(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut sort_field = "updated".to_string();
    let mut ascending = false;
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

    for n in notes {
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
    let size = fs::metadata(&path)?.len();
    let note = parse_note(&path, size)?;
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
    note.updated = timestamp_string();
    write_note(&note, dir)?;
    println!("Updated {}", note.id);
    Ok(())
}

fn delete_notes(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut use_fzf = false;
    let mut ids: Vec<String> = Vec::new();
    for a in args {
        if a == "--fzf" {
            use_fzf = true;
        } else {
            ids.push(a);
        }
    }

    if ids.is_empty() {
        if !use_fzf && !has_fzf() {
            return Err("Provide ids or install fzf / use --fzf for interactive delete".into());
        }
        let files = list_note_files(dir)?;
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

fn seed_notes(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        return Err("Usage: qn seed <count> [--chars N]".into());
    }
    let count: usize = args[0].parse().map_err(|_| "Count must be a number")?;
    let mut body_len: usize = 400;
    let mut iter = args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        if arg == "--chars" {
            if let Some(v) = iter.next() {
                body_len = v.parse().map_err(|_| "chars must be a number")?;
            } else {
                return Err("Provide a value for --chars".into());
            }
        } else {
            return Err(format!("Unknown flag for seed: {arg}").into());
        }
    }

    for i in 0..count {
        let title = format!("Seed note {}", short_timestamp());
        let body = generate_body(body_len, i);
        let note = create_note(title, body, dir)?;
        if (i + 1) % 50 == 0 || i + 1 == count {
            println!("Generated {}/{} (last id {})", i + 1, count, note.id);
        }
    }
    Ok(())
}

fn create_note(title: String, body: String, dir: &Path) -> Result<Note, Box<dyn Error>> {
    let id = unique_id(dir)?;
    let now = timestamp_string();
    let mut note = Note {
        id: id.clone(),
        title,
        created: now.clone(),
        updated: now,
        body,
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
    let content = format!(
        "Title: {}\nCreated: {}\nUpdated: {}\n---\n{}",
        note.title, note.created, note.updated, body
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
