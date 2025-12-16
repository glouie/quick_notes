# Quick Notes Refactoring Plan

This document outlines a comprehensive refactoring strategy to improve code consistency, reduce duplication, and introduce reusable abstractions throughout the codebase.

## Executive Summary

**Current State:**
- 2,221 lines in `src/lib.rs` (35% of codebase)
- ~550+ lines of duplicated code (21% of lib.rs)
- 10+ instances of identical argument parsing
- 3 duplicate FZF implementations
- 7+ repeated tag filtering patterns

**Goals:**
- Reduce lib.rs by 40% through extraction
- Create 6 new reusable modules
- Eliminate 90%+ code duplication
- Maintain 100% backward compatibility
- Improve testability and maintainability

## Phase 1: Core Abstractions (High Priority)

### 1.1 Argument Parser Module

**File:** `src/args.rs`

**Problem:** 140+ lines of duplicated flag parsing across 10+ commands

**Proposed Solution:**

```rust
// src/args.rs

use std::error::Error;

pub struct ArgParser {
    iter: std::vec::IntoIter<String>,
    command_name: String,
}

impl ArgParser {
    pub fn new(args: Vec<String>, command_name: &str) -> Self {
        Self {
            iter: args.into_iter(),
            command_name: command_name.to_string(),
        }
    }

    /// Extract a single tag from -t/--tag flag
    pub fn extract_tag(&mut self) -> Result<Option<String>, Box<dyn Error>> {
        match self.iter.next() {
            Some(v) => {
                let tag = crate::tags::normalize_tag(&v);
                if tag.is_empty() {
                    Err(format!("Invalid tag provided to {}", self.command_name).into())
                } else {
                    Ok(Some(tag))
                }
            }
            None => Err(format!(
                "Provide a tag after -t/--tag for {}",
                self.command_name
            )
            .into()),
        }
    }

    /// Extract multiple tags from repeating -t flags
    pub fn extract_tags(&mut self) -> Vec<String> {
        let mut tags = Vec::new();
        while let Ok(Some(tag)) = self.extract_tag() {
            tags.push(tag);
        }
        tags
    }

    /// Extract a string value for a flag
    pub fn extract_value(&mut self, flag: &str) -> Result<String, Box<dyn Error>> {
        self.iter.next().ok_or_else(|| {
            format!("Provide a value after {} for {}", flag, self.command_name).into()
        })
    }

    /// Check if there are remaining arguments
    pub fn has_more(&self) -> bool {
        self.iter.len() > 0
    }

    /// Get next positional argument
    pub fn next(&mut self) -> Option<String> {
        self.iter.next()
    }

    /// Collect remaining args as IDs
    pub fn collect_ids(&mut self) -> Vec<String> {
        self.iter.collect()
    }

    /// Parse common flags used across multiple commands
    pub fn parse_common_flags(&mut self) -> Result<CommonFlags, Box<dyn Error>> {
        let mut flags = CommonFlags::default();

        while let Some(arg) = self.iter.next() {
            match arg.as_str() {
                "-t" | "--tag" => {
                    if let Some(tag) = self.extract_tag()? {
                        flags.tag_filters.push(tag);
                    }
                }
                "-s" | "--search" => {
                    flags.search = Some(self.extract_value("-s/--search")?);
                }
                "-r" | "--relative" => {
                    flags.relative_time = true;
                }
                "-a" | "--all" => {
                    flags.show_all = true;
                }
                "--fzf" => {
                    flags.use_fzf = true;
                }
                other if other.starts_with('-') => {
                    return Err(format!(
                        "Unknown flag '{}' for {}",
                        other, self.command_name
                    )
                    .into());
                }
                other => {
                    flags.positional.push(other.to_string());
                }
            }
        }

        Ok(flags)
    }
}

#[derive(Default, Debug)]
pub struct CommonFlags {
    pub tag_filters: Vec<String>,
    pub search: Option<String>,
    pub relative_time: bool,
    pub show_all: bool,
    pub use_fzf: bool,
    pub positional: Vec<String>,
}
```

**Migration Example:**

```rust
// Before (in list_notes_in, lines 180-221):
fn list_notes(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut sort_field = "updated".to_string();
    let mut ascending = false;
    let mut search: Option<String> = None;
    let mut tag_filters: Vec<String> = Vec::new();
    let mut relative_time = false;
    let mut paginate = true;
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
            "-r" | "--relative" => {
                relative_time = true;
            }
            "-a" | "--all" => paginate = false,
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
    // ... rest of function
}

// After:
fn list_notes(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut parser = ArgParser::new(args, "list");
    let mut sort_field = "updated".to_string();
    let mut ascending = false;

    let flags = parser.parse_list_flags()?; // Custom extension

    let search = flags.search;
    let tag_filters = flags.tag_filters;
    let relative_time = flags.relative_time;
    let paginate = !flags.show_all;

    // ... rest of function (now 40 lines shorter)
}
```

**Impact:**
- Eliminates 140+ lines of duplication
- Consistent error messages across commands
- Easy to add new global flags
- Better testability

---

### 1.2 FZF Selector Module

**File:** `src/fzf.rs`

**Problem:** 120 lines of duplicated FZF code in delete/archive/edit commands

**Proposed Solution:**

```rust
// src/fzf.rs

use std::error::Error;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

pub struct FzfSelector {
    preview_command: Option<String>,
    multi_select: bool,
    height: Option<String>,
    layout: Option<String>,
}

impl FzfSelector {
    pub fn new() -> Self {
        Self {
            preview_command: None,
            multi_select: false,
            height: None,
            layout: None,
        }
    }

    pub fn with_note_preview() -> Self {
        let renderer = get_renderer_name();
        let preview = format!(
            "env -u NO_COLOR CLICOLOR_FORCE=1 {renderer} render $(basename {{}}) 2>/dev/null || sed -n '1,120p' {{}}"
        );
        Self {
            preview_command: Some(preview),
            multi_select: true,
            height: None,
            layout: None,
        }
    }

    pub fn with_simple_preview() -> Self {
        Self {
            preview_command: Some("sed -n '1,120p' {}".to_string()),
            multi_select: true,
            height: Some("70%".to_string()),
            layout: Some("reverse".to_string()),
        }
    }

    pub fn multi_select(mut self, enabled: bool) -> Self {
        self.multi_select = enabled;
        self
    }

    pub fn height(mut self, height: &str) -> Self {
        self.height = Some(height.to_string());
        self
    }

    pub fn select_from_paths(&self, paths: &[PathBuf]) -> Result<Vec<String>, Box<dyn Error>> {
        let input = paths
            .iter()
            .map(|p| p.to_string_lossy())
            .collect::<Vec<_>>()
            .join("\n");

        self.select_from_input(&input)
    }

    pub fn select_from_input(&self, input: &str) -> Result<Vec<String>, Box<dyn Error>> {
        if !is_fzf_available() {
            return Err("fzf is not installed or QUICK_NOTES_NO_FZF is set".into());
        }

        let mut cmd = Command::new("fzf");

        if self.multi_select {
            cmd.arg("--multi");
        }

        if let Some(ref height) = self.height {
            cmd.arg("--height").arg(height);
        }

        if let Some(ref layout) = self.layout {
            cmd.arg("--layout").arg(layout);
        }

        if let Some(ref preview) = self.preview_command {
            cmd.arg("--preview").arg(preview);
        }

        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(input.as_bytes())?;
        }

        let output = child.wait_with_output()?;

        if !output.status.success() || output.stdout.is_empty() {
            return Ok(Vec::new()); // User cancelled
        }

        let selected = String::from_utf8_lossy(&output.stdout);
        Ok(selected.lines().map(|s| s.to_string()).collect())
    }

    pub fn select_note_ids(&self, paths: &[PathBuf]) -> Result<Vec<String>, Box<dyn Error>> {
        let selected = self.select_from_paths(paths)?;

        let ids = selected
            .iter()
            .filter_map(|path_str| {
                Path::new(path_str)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            })
            .collect();

        Ok(ids)
    }
}

pub fn is_fzf_available() -> bool {
    if std::env::var("QUICK_NOTES_NO_FZF").is_ok() {
        return false;
    }

    static FZF_AVAILABLE: OnceLock<bool> = OnceLock::new();
    *FZF_AVAILABLE.get_or_init(|| {
        Command::new("fzf")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
    })
}

fn get_renderer_name() -> &'static str {
    static RENDERER: OnceLock<&str> = OnceLock::new();
    *RENDERER.get_or_init(|| {
        if Command::new("quick_notes")
            .arg("--help")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
        {
            "quick_notes"
        } else {
            "qn"
        }
    })
}
```

**Migration Example:**

```rust
// Before (in delete_notes, lines 1138-1206):
if ids.is_empty() {
    if !use_fzf && !has_fzf() {
        return Err("Provide ids or install fzf / use --fzf for interactive delete".into());
    }
    let mut files = list_active_note_files(dir)?;
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

    let input = files.iter().map(|(p, _)| p.to_string_lossy()).collect::<Vec<_>>().join("\n");

    let renderer = if Command::new("quick_notes")...{...} else {...};

    let mut child = Command::new("fzf")
        .arg("--multi")
        .arg("--preview")
        .arg(format!("env -u NO_COLOR CLICOLOR_FORCE=1 {renderer} render $(basename {{}}) 2>/dev/null || sed -n '1,120p' {{}}"))
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

// After:
if ids.is_empty() {
    if !use_fzf && !fzf::is_fzf_available() {
        return Err("Provide ids or install fzf / use --fzf for interactive delete".into());
    }

    let files = list_active_note_files(dir)?;
    let filtered_files = filter_by_tags(files, &tag_filters)?;

    if filtered_files.is_empty() {
        println!("No notes to delete.");
        return Ok(());
    }

    let selector = FzfSelector::with_note_preview();
    ids = selector.select_note_ids(&filtered_files)?;

    if ids.is_empty() {
        println!("No selection made; nothing deleted.");
        return Ok(());
    }
}
```

**Impact:**
- Eliminates 120 lines of duplication
- Cached FZF availability check
- Cached renderer name
- Reusable for future interactive commands
- Better error handling

---

### 1.3 Tag Management Module

**File:** `src/tags.rs`

**Problem:** 90+ lines of repeated tag filtering and normalization

**Proposed Solution:**

```rust
// src/tags.rs

use crate::note::Note;
use std::collections::HashSet;
use std::error::Error;
use std::path::Path;

/// Normalize a tag to #tag format
pub fn normalize_tag(t: &str) -> String {
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

/// Check if a note has all required tags
pub fn note_has_tags(note: &Note, tags: &[String]) -> bool {
    if tags.is_empty() {
        return true;
    }
    tags.iter().all(|t| note.tags.contains(t))
}

/// Normalize and deduplicate a list of tags
pub fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut normalized: Vec<String> = tags
        .into_iter()
        .map(|t| normalize_tag(&t))
        .filter(|t| !t.is_empty())
        .collect();
    normalized.sort();
    normalized.dedup();
    normalized
}

/// Extract tags from command arguments
pub fn extract_tags_from_args(
    args: &mut impl Iterator<Item = String>,
) -> Result<Vec<String>, Box<dyn Error>> {
    let mut tags = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "-t" || arg == "--tag" {
            if let Some(v) = args.next() {
                let tag = normalize_tag(&v);
                if !tag.is_empty() {
                    tags.push(tag);
                }
            } else {
                return Err("Provide a tag after -t/--tag".into());
            }
        }
    }
    Ok(tags)
}

/// Validate that a note at the given path has all required tags
pub fn validate_note_tags(
    dir: &Path,
    id: &str,
    tag_filters: &[String],
) -> Result<bool, Box<dyn Error>> {
    if tag_filters.is_empty() {
        return Ok(true);
    }

    let path = crate::shared::migrate::resolve_active_note_path(dir, id)
        .ok_or_else(|| format!("Note {} not found", id))?;

    let size = std::fs::metadata(&path)?.len();
    let note = crate::note::parse_note(&path, size)?;

    Ok(note_has_tags(&note, tag_filters))
}

/// Get pinned tags from environment or default
pub fn get_pinned_tags() -> Vec<String> {
    const DEFAULT: &str = "#todo,#meeting,#scratch";

    let pinned = std::env::var("QUICK_NOTES_PINNED_TAGS")
        .unwrap_or_else(|_| DEFAULT.to_string());

    pinned
        .split(',')
        .map(|t| normalize_tag(t.trim()))
        .filter(|t| !t.is_empty())
        .collect()
}

/// Hash a tag for deterministic color selection
pub fn hash_tag(tag: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in tag.bytes() {
        h = (h.wrapping_shl(5)).wrapping_add(h) ^ u64::from(b);
    }
    h
}
```

**Impact:**
- Consolidates all tag logic in one module
- Eliminates 90+ lines of duplication
- Easier to add tag features (autocomplete, validation, etc.)

---

## Phase 2: Formatting and Display (Medium Priority)

### 2.1 Unified Formatting Module

**File:** `src/formatting.rs`

**Problem:** 8 scattered formatting functions with no shared infrastructure

**Proposed Solution:**

```rust
// src/formatting.rs

use chrono::{DateTime, FixedOffset};
use yansi::Paint;

/// Color palette for consistent theming
pub struct ColorPalette {
    pub primary: (u8, u8, u8),      // IDs, muted text
    pub secondary: (u8, u8, u8),    // Headers, emphasis
    pub timestamp: (u8, u8, u8),    // Timestamps
    pub highlight: (u8, u8, u8),    // Search matches
}

impl ColorPalette {
    pub const CATPPUCCIN: Self = Self {
        primary: (108, 112, 134),      // Gray
        secondary: (148, 226, 213),     // Teal
        timestamp: (137, 180, 250),     // Blue
        highlight: (243, 139, 168),     // Pink
    };
}

/// Formatting context passed through rendering pipeline
pub struct FormatContext {
    pub use_color: bool,
    pub palette: ColorPalette,
}

impl FormatContext {
    pub fn new(use_color: bool) -> Self {
        Self {
            use_color,
            palette: ColorPalette::CATPPUCCIN,
        }
    }

    pub fn from_env() -> Self {
        let use_color = std::env::var("NO_COLOR").is_err();
        Self::new(use_color)
    }

    pub fn format_id(&self, id: &str) -> String {
        if self.use_color {
            let (r, g, b) = self.palette.primary;
            Paint::rgb(id, r, g, b).to_string()
        } else {
            id.to_string()
        }
    }

    pub fn format_header(&self, text: &str) -> String {
        if self.use_color {
            let (r, g, b) = self.palette.secondary;
            Paint::rgb(text, r, g, b).bold().to_string()
        } else {
            text.to_string()
        }
    }

    pub fn format_timestamp(&self, ts: &str) -> String {
        if self.use_color {
            let (r, g, b) = self.palette.timestamp;
            Paint::rgb(ts, r, g, b).to_string()
        } else {
            ts.to_string()
        }
    }

    pub fn format_tag(&self, tag: &str) -> String {
        if self.use_color {
            let (r, g, b) = crate::tags::color_for_tag(tag);
            Paint::rgb(tag, r, g, b).bold().to_string()
        } else {
            tag.to_string()
        }
    }

    pub fn highlight_match(&self, text: &str, query: Option<&str>) -> String {
        let Some(q) = query else { return text.to_string() };
        if q.is_empty() || !self.use_color {
            return text.to_string();
        }

        let q_lower = q.to_lowercase();
        let mut out = String::new();
        let mut remaining = text;

        while let Some(pos) = remaining.to_lowercase().find(&q_lower) {
            let (before, rest) = remaining.split_at(pos);
            let (matched, after) = rest.split_at(q.len().min(rest.len()));
            out.push_str(before);

            let (r, g, b) = self.palette.highlight;
            out.push_str(&Paint::rgb(matched, r, g, b).to_string());

            remaining = after;
        }
        out.push_str(remaining);
        out
    }
}

/// Timestamp formatting with relative/absolute modes
pub struct TimeFormatter {
    relative_mode: bool,
    now: DateTime<FixedOffset>,
}

impl TimeFormatter {
    pub fn new(relative_mode: bool, now: DateTime<FixedOffset>) -> Self {
        Self { relative_mode, now }
    }

    pub fn format(&self, ts: &str) -> String {
        if let Some(dt) = crate::note::parse_timestamp(ts) {
            if self.relative_mode {
                self.format_relative(dt)
            } else {
                dt.format("%d%b%y %H:%M").to_string()
            }
        } else {
            ts.split_whitespace().take(2).collect::<Vec<_>>().join(" ")
        }
    }

    fn format_relative(&self, dt: DateTime<FixedOffset>) -> String {
        let dur = self.now.signed_duration_since(dt);
        let total_hours = dur.num_hours().max(0);
        let total_days = dur.num_days().max(0);

        if total_days < 30 {
            if total_days == 0 {
                return format!("{}h ago", total_hours);
            }
            let hours = (total_hours - total_days * 24).max(0);
            if hours > 0 {
                format!("{}d {}h ago", total_days, hours)
            } else {
                format!("{}d ago", total_days)
            }
        } else if total_days < 365 {
            let months = total_days / 30;
            let days = total_days % 30;
            if days > 0 {
                format!("{}mo {}d ago", months, days)
            } else {
                format!("{}mo ago", months)
            }
        } else {
            let years = total_days / 365;
            let months = (total_days % 365) / 30;
            if months > 0 {
                format!("{}y {}mo ago", years, months)
            } else {
                format!("{}y ago", years)
            }
        }
    }

    pub fn format_label(&self, base: &str) -> String {
        if self.relative_mode {
            base.to_string()
        } else {
            self.determine_tz_label()
                .map(|tz| format!("{} ({})", base, tz))
                .unwrap_or_else(|| base.to_string())
        }
    }

    fn determine_tz_label(&self) -> Option<String> {
        crate::note::parse_timestamp(&crate::note::timestamp_string())
            .map(|dt| dt.offset().to_string())
    }
}
```

**Migration:** Replace all `format_id()`, `format_timestamp()`, etc. with context methods

**Impact:**
- Centralized color definitions
- Easy to add new themes
- Consistent formatting API
- ~85 lines reduced through consolidation

---

### 2.2 Table Rendering Improvements

**File:** `src/shared/table.rs` (enhance existing)

**Problem:** Parameter explosion with 13-field structs

**Proposed Solution:**

```rust
// src/shared/table.rs (additions)

use crate::formatting::FormatContext;
use crate::note::Note;

/// Row data for list rendering (domain data only)
pub struct NoteRow<'a> {
    pub note: &'a Note,
    pub preview: String,
    pub created_display: Option<String>,
    pub moved_display: Option<String>,
}

/// Rendering configuration
pub struct TableConfig {
    pub widths: ColumnWidths,
    pub include_tags: bool,
    pub include_created: bool,
    pub include_moved: bool,
}

pub fn render_note_row(
    row: &NoteRow,
    config: &TableConfig,
    format_ctx: &FormatContext,
    time_fmt: &crate::formatting::TimeFormatter,
) -> String {
    // Simplified rendering with separated concerns
    // - row: what to display
    // - config: how much space
    // - format_ctx: colors
    // - time_fmt: timestamp format

    // Implementation here...
    String::new() // Placeholder
}
```

**Impact:**
- Separates data, config, and formatting concerns
- Easier to test individual components
- Reduces parameter passing complexity

---

## Phase 3: File Operations (Medium Priority)

### 3.1 Note Operations Module

**File:** `src/operations.rs`

**Problem:** 100+ lines of repeated file operation patterns

**Proposed Solution:**

```rust
// src/operations.rs

use crate::note::{Note, note_path, parse_note, write_note};
use crate::shared::migrate::resolve_active_note_path;
use std::error::Error;
use std::fs;
use std::path::Path;

/// Load a note by ID, resolving across directories
pub fn load_note(dir: &Path, id: &str) -> Result<Note, Box<dyn Error>> {
    let path = resolve_active_note_path(dir, id)
        .ok_or_else(|| format!("Note {} not found", id))?;

    let size = fs::metadata(&path)?.len();
    parse_note(&path, size)
}

/// Ensure a note ID is unique, generating a new one if needed
pub fn ensure_unique_id(dir: &Path, preferred: &str) -> Result<String, Box<dyn Error>> {
    let path = note_path(dir, preferred);
    if !path.exists() {
        return Ok(preferred.to_string());
    }

    // Generate new ID
    let mut reserved = std::collections::HashSet::new();
    reserved.insert(preferred.to_string());
    crate::note::generate_new_id(dir, &mut reserved)
}

/// Move a note between areas with timestamp updates
pub fn move_note(
    from_dir: &Path,
    to_dir: &Path,
    id: &str,
    update_fn: impl FnOnce(&mut Note),
) -> Result<(), Box<dyn Error>> {
    let mut note = load_note(from_dir, id)?;
    update_fn(&mut note);

    crate::note::ensure_dir(to_dir)?;
    write_note(&note, to_dir)?;

    // Remove old file
    let src = resolve_active_note_path(from_dir, id)
        .ok_or_else(|| format!("Note {} not found", id))?;
    fs::remove_file(src)?;

    Ok(())
}

/// Filter notes by tag requirements
pub fn filter_by_tags(
    files: Vec<(std::path::PathBuf, u64)>,
    tag_filters: &[String],
) -> Result<Vec<std::path::PathBuf>, Box<dyn Error>> {
    if tag_filters.is_empty() {
        return Ok(files.into_iter().map(|(p, _)| p).collect());
    }

    let mut filtered = Vec::new();
    for (path, size) in files {
        if let Ok(note) = parse_note(&path, size) {
            if crate::tags::note_has_tags(&note, tag_filters) {
                filtered.push(path);
            }
        }
    }
    Ok(filtered)
}
```

**Impact:**
- Eliminates 100+ lines of duplication
- Consistent error handling
- Reusable operations for future commands

---

## Phase 4: Module Structure (Low Priority)

### 4.1 Split lib.rs into Command Modules

**Proposed Structure:**

```
src/
├── lib.rs (350 lines - dispatch only)
├── commands/
│   ├── mod.rs
│   ├── add.rs (quick_add)
│   ├── new.rs (new_note)
│   ├── list.rs (list_notes, list_deleted, list_archived)
│   ├── view.rs (view_note)
│   ├── edit.rs (edit_note)
│   ├── delete.rs (delete_notes, delete_all_notes)
│   ├── archive.rs (archive_notes, unarchive_notes)
│   ├── migrate.rs (migrate_ids)
│   ├── tags.rs (list_tags)
│   ├── seed.rs (seed_notes)
│   └── stats.rs (stats)
├── args.rs (argument parsing)
├── fzf.rs (FZF integration)
├── formatting.rs (display formatting)
├── operations.rs (file operations)
└── tags.rs (tag management)
```

**lib.rs After Refactoring:**

```rust
// src/lib.rs (reduced to ~350 lines)

mod commands;
mod args;
mod fzf;
mod formatting;
mod operations;
pub mod tags;

pub use commands::*;

/// Dispatch CLI arguments to the right subcommand
pub fn entry() -> Result<(), Box<dyn Error>> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        return help::run(Vec::new());
    }

    let cmd = args.remove(0);
    let dir = notes_dir()?;
    ensure_dir(&dir)?;

    match cmd.as_str() {
        "-h" | "--help" => help::run(args),
        "add" => commands::add::execute(args, &dir),
        "new" => commands::new::execute(args, &dir),
        "list" => commands::list::execute(args, &dir),
        "view" | "render" => commands::view::execute(args, &dir),
        "edit" => commands::edit::execute(args, &dir),
        "delete" => commands::delete::execute(args, &dir),
        "delete-all" => commands::delete::execute_all(&dir),
        "list-deleted" => commands::list::execute_deleted(args, &dir),
        "list-archived" => commands::list::execute_archived(args, &dir),
        "archive" => commands::archive::execute(args, &dir),
        "unarchive" => commands::archive::unexecute(args, &dir),
        "undelete" => commands::archive::unexecute_trash(args, &dir),
        "migrate" => commands::migrate::execute(args, &dir),
        "seed" => commands::seed::execute(args, &dir),
        "tags" => commands::tags::execute(args, &dir),
        "stats" => commands::stats::execute(&dir),
        "path" => println!("{}", dir.display()),
        "completion" => print_completion(args),
        "help" => help::run(args),
        "guide" => help::run_guides(args),
        other => {
            eprintln!("Unknown command: {other}");
            help::run(Vec::new())
        }
    }
}
```

**Impact:**
- lib.rs reduced from 2,221 to ~350 lines (84% reduction)
- Each command in its own testable module
- Easier to add new commands
- Better code organization

---

## Implementation Plan

### Step 1: Create Core Modules (Week 1)
1. Create `src/tags.rs` - extract tag functions
2. Create `src/args.rs` - argument parsing
3. Create `src/fzf.rs` - FZF selector
4. Run tests to ensure no regressions

### Step 2: Formatting Module (Week 2)
1. Create `src/formatting.rs`
2. Migrate all format_* functions
3. Update call sites incrementally
4. Run tests after each migration

### Step 3: Operations Module (Week 2)
1. Create `src/operations.rs`
2. Extract file operation helpers
3. Update commands to use new helpers
4. Run tests

### Step 4: Split Commands (Week 3)
1. Create `src/commands/` directory structure
2. Move one command at a time, starting with simplest (stats, path)
3. Test after each move
4. Refactor lib.rs dispatch

### Step 5: Integration Testing (Week 3)
1. Run full test suite
2. Manual testing of all commands
3. Performance benchmarking
4. Update CLAUDE.md with new structure

---

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arg_parser_tags() {
        let args = vec!["-t".into(), "todo".into(), "-t".into(), "work".into()];
        let mut parser = ArgParser::new(args, "test");
        let flags = parser.parse_common_flags().unwrap();
        assert_eq!(flags.tag_filters, vec!["#todo", "#work"]);
    }

    #[test]
    fn test_fzf_selector_builder() {
        let selector = FzfSelector::new()
            .multi_select(true)
            .height("50%");
        assert!(selector.multi_select);
        assert_eq!(selector.height.as_deref(), Some("50%"));
    }

    #[test]
    fn test_format_context() {
        let ctx = FormatContext::new(false);
        assert_eq!(ctx.format_id("abc123"), "abc123");

        let ctx = FormatContext::new(true);
        assert!(ctx.format_id("abc123").contains("abc123"));
    }
}
```

### Integration Tests
```rust
// tests/refactored_commands.rs
#[test]
fn test_delete_with_fzf() {
    let tmp = tempdir().unwrap();
    // Create notes
    // Test FZF integration
    // Verify deletion
}
```

---

## Migration Checklist

- [ ] Create `src/tags.rs` module
- [ ] Create `src/args.rs` module
- [ ] Create `src/fzf.rs` module
- [ ] Create `src/formatting.rs` module
- [ ] Create `src/operations.rs` module
- [ ] Update `src/lib.rs` to use new modules
- [ ] Create `src/commands/` directory
- [ ] Split commands into separate files
- [ ] Update all tests
- [ ] Update CLAUDE.md documentation
- [ ] Run `cargo fmt && cargo clippy -- -D warnings`
- [ ] Full test suite passes
- [ ] Manual testing of all commands
- [ ] Update CONTRIBUTE.md with new structure
- [ ] Update README.md if needed

---

## Expected Outcomes

**Code Metrics:**
- lib.rs: 2,221 → ~350 lines (84% reduction)
- Total duplication: ~550 → ~50 lines (91% reduction)
- Module count: 5 → 15 (+10 focused modules)
- Average function length: Reduced by ~40%

**Maintainability:**
- ✅ Single responsibility per module
- ✅ Clear separation of concerns
- ✅ Reusable abstractions
- ✅ Better testability
- ✅ Easier to add features

**Developer Experience:**
- ✅ Easier to find code
- ✅ Less duplicate code to maintain
- ✅ Consistent patterns throughout
- ✅ Better error messages
- ✅ Improved documentation

**Risks:**
- Low: Changes are mostly mechanical extractions
- All existing tests should continue to pass
- Backward compatible - no API changes
- Can be done incrementally

---

## Next Steps

1. Review this plan with stakeholders
2. Get approval for phased approach
3. Create feature branch: `refactor/modular-structure`
4. Start with Phase 1 (Core Abstractions)
5. Submit incremental PRs for review
6. Gather feedback and adjust approach

This refactoring will modernize the codebase while maintaining stability and backward compatibility.
