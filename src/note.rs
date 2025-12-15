use chrono::{DateTime, FixedOffset, Local};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub const TIME_FMT: &str = "%d%b%y %H:%M %:z";
pub const LEGACY_TIME_FMT: &str = "%m/%d/%Y %I:%M %p %:z";
pub const ID_TS_WIDTH: usize = 9;

#[derive(Debug, Clone)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub created: String,
    pub updated: String,
    pub deleted_at: Option<String>,
    pub archived_at: Option<String>,
    pub body: String,
    pub tags: Vec<String>,
    pub size_bytes: u64,
}

pub fn notes_dir() -> io::Result<PathBuf> {
    if let Ok(dir) = std::env::var("QUICK_NOTES_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var("HOME").map_err(|_| {
        io::Error::other(
            "HOME not set; set QUICK_NOTES_DIR explicitly",
        )
    })?;
    Ok(PathBuf::from(home).join(".quick_notes"))
}

pub fn ensure_dir(path: &Path) -> io::Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

pub fn note_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.md"))
}

pub fn unique_id(dir: &Path) -> io::Result<String> {
    generate_new_id(dir, &mut HashSet::new())
}

pub fn short_timestamp() -> String {
    encode_base62_width(
        Local::now().timestamp_micros().max(0) as u64,
        ID_TS_WIDTH,
    )
}

pub fn now_fixed() -> DateTime<FixedOffset> {
    Local::now().with_timezone(Local::now().offset())
}

pub fn timestamp_string() -> String {
    let now = Local::now();
    now.format(TIME_FMT).to_string()
}

pub fn parse_timestamp(ts: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_str(ts, TIME_FMT)
        .or_else(|_| DateTime::parse_from_str(ts, LEGACY_TIME_FMT))
        .ok()
}

pub fn cmp_dt(a: &str, b: &str) -> Ordering {
    let a_dt = parse_timestamp(a);
    let b_dt = parse_timestamp(b);
    match (a_dt, b_dt) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        _ => Ordering::Equal,
    }
}

pub fn write_note(note: &Note, dir: &Path) -> io::Result<()> {
    let mut body = note.body.trim_end_matches('\n').to_string();
    body.push('\n');
    let tags_line = if note.tags.is_empty() {
        "Tags:".to_string()
    } else {
        format!("Tags: {}", note.tags.join(", "))
    };
    let deleted_line = note
        .deleted_at
        .as_ref()
        .map(|d| format!("Deleted: {d}\n"))
        .unwrap_or_default();
    let archived_line = note
        .archived_at
        .as_ref()
        .map(|d| format!("Archived: {d}\n"))
        .unwrap_or_default();
    let content = format!(
        "Title: {}\nCreated: {}\nUpdated: {}\n{}{}{}\n---\n{}",
        note.title,
        note.created,
        note.updated,
        deleted_line,
        archived_line,
        tags_line,
        body
    );
    fs::write(note_path(dir, &note.id), content)
}

pub fn parse_note(path: &Path, size_bytes: u64) -> io::Result<Note> {
    let raw = fs::read_to_string(path)?;
    let (header, body) = if let Some(idx) = raw.find("\n---\n") {
        raw.split_at(idx + 5)
    } else {
        ("", raw.as_str())
    };

    let mut title = String::new();
    let mut created = String::new();
    let mut updated = String::new();
    let mut deleted_at: Option<String> = None;
    let mut archived_at: Option<String> = None;
    let mut tags: Vec<String> = Vec::new();

    for line in header.lines() {
        if let Some(val) = line.strip_prefix("Title:") {
            title = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("Created:") {
            created = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("Updated:") {
            updated = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("Deleted:") {
            deleted_at = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("Archived:") {
            archived_at = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("Tags:") {
            tags = val
                .split(',')
                .map(|t| super::normalize_tag(t.trim()))
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
        deleted_at,
        archived_at,
        body: body.to_string(),
        tags,
        size_bytes,
    })
}

#[derive(Default)]
struct IdState {
    last_ts: i64,
    counter: u32,
}

pub(crate) fn generate_new_id(
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
