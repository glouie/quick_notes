use crate::note::{
    Note, ensure_dir, generate_new_id, note_path, parse_note, write_note,
};
use crate::shared::migrate::resolve_active_note_path;
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// Load a note by ID, resolving across directories
pub fn load_note(dir: &Path, id: &str) -> Result<Note, Box<dyn Error>> {
    let path = resolve_active_note_path(dir, id)
        .ok_or_else(|| format!("Note {} not found", id))?;

    let size = fs::metadata(&path)?.len();
    Ok(parse_note(&path, size)?)
}

/// Ensure a note ID is unique, generating a new one if needed
pub fn ensure_unique_id(
    dir: &Path,
    preferred: &str,
) -> Result<String, Box<dyn Error>> {
    let path = note_path(dir, preferred);
    if !path.exists() {
        return Ok(preferred.to_string());
    }

    // Generate new ID
    let mut reserved = HashSet::new();
    reserved.insert(preferred.to_string());
    Ok(generate_new_id(dir, &mut reserved)?)
}

/// Move a note between directories with custom update function
pub fn move_note(
    from_dir: &Path,
    to_dir: &Path,
    id: &str,
    update_fn: impl FnOnce(&mut Note),
) -> Result<(), Box<dyn Error>> {
    let mut note = load_note(from_dir, id)?;
    update_fn(&mut note);

    ensure_dir(to_dir)?;
    write_note(&note, to_dir)?;

    // Remove old file
    let src = resolve_active_note_path(from_dir, id)
        .ok_or_else(|| format!("Note {} not found", id))?;
    fs::remove_file(src)?;

    Ok(())
}

/// Filter notes by tag requirements
pub fn filter_by_tags(
    files: Vec<(PathBuf, u64)>,
    tag_filters: &[String],
) -> Result<Vec<PathBuf>, Box<dyn Error>> {
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

/// Validate note exists and matches tag filters
pub fn validate_note(
    dir: &Path,
    id: &str,
    tag_filters: &[String],
) -> Result<bool, Box<dyn Error>> {
    if tag_filters.is_empty() {
        // Just check existence
        return Ok(resolve_active_note_path(dir, id).is_some());
    }

    let note = load_note(dir, id)?;
    Ok(crate::tags::note_has_tags(&note, tag_filters))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_ensure_unique_id_no_conflict() {
        let tmp = tempdir().unwrap();
        let result = ensure_unique_id(tmp.path(), "test123").unwrap();
        assert_eq!(result, "test123");
    }

    #[test]
    fn test_filter_by_tags_empty() {
        let files = vec![];
        let result = filter_by_tags(files, &[]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_validate_note_no_filters() {
        let tmp = tempdir().unwrap();
        // Note doesn't exist
        let result = validate_note(tmp.path(), "nonexistent", &[]);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}
