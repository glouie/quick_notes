use crate::note::{
    ensure_dir, generate_new_id, note_path, parse_note, short_timestamp,
    timestamp_string, write_note,
};
use crate::{Area, area_dir, list_note_files};
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) fn migrated_dir(dir: &Path) -> PathBuf {
    dir.join("migrated")
}

pub(crate) fn list_active_note_files(
    dir: &Path,
) -> io::Result<Vec<(PathBuf, u64)>> {
    let mut files = list_note_files(dir)?;
    let migrated_root = migrated_dir(dir);
    if migrated_root.exists() {
        for entry in fs::read_dir(migrated_root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                files.extend(list_note_files(&entry.path())?);
            }
        }
    }
    Ok(files)
}

pub(crate) fn resolve_active_note_path(
    dir: &Path,
    id: &str,
) -> Option<PathBuf> {
    let direct = note_path(dir, id);
    if direct.exists() {
        return Some(direct);
    }
    let migrated_root = migrated_dir(dir);
    if let Ok(entries) = fs::read_dir(migrated_root) {
        for entry in entries.flatten() {
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                let candidate = entry.path().join(format!("{id}.md"));
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn list_note_files_if_exists(dir: &Path) -> io::Result<Vec<(PathBuf, u64)>> {
    if dir.exists() { list_note_files(dir) } else { Ok(Vec::new()) }
}

pub(crate) fn collect_ids_across_areas(
    dir: &Path,
) -> io::Result<HashSet<String>> {
    let mut ids: HashSet<String> = HashSet::new();
    for (path, _) in list_active_note_files(dir)? {
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            ids.insert(stem.to_string());
        }
    }
    for area in [Area::Trash, Area::Archive] {
        let area_dir = area_dir(dir, area);
        for (path, _) in list_note_files_if_exists(&area_dir)? {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                ids.insert(stem.to_string());
            }
        }
    }
    Ok(ids)
}

/// Import notes from another directory into a new migrated batch, keeping timestamps.
pub(crate) fn migrate_notes(
    args: Vec<String>,
    dir: &Path,
) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        return Err("Usage: qn migrate <path>".into());
    }
    let src = PathBuf::from(&args[0]);
    if !src.exists() {
        return Err(format!("Source path not found: {}", src.display()).into());
    }
    if !src.is_dir() {
        return Err(format!(
            "Source path is not a directory: {}",
            src.display()
        )
        .into());
    }
    let files = list_note_files(&src)?;
    if files.is_empty() {
        println!("No notes to migrate from {}", src.display());
        return Ok(());
    }

    let migrated_root = migrated_dir(dir);
    ensure_dir(&migrated_root)?;
    let mut batch_id = format!("migration-{}", short_timestamp());
    let mut batch_dir = migrated_root.join(&batch_id);
    while batch_dir.exists() {
        batch_id = format!("migration-{}", short_timestamp());
        batch_dir = migrated_root.join(&batch_id);
    }
    ensure_dir(&batch_dir)?;

    let mut reserved = collect_ids_across_areas(dir)?;
    let mut migrated = 0;
    for (path, size) in files {
        let mut note = match parse_note(&path, size) {
            Ok(note) => note,
            Err(e) => {
                eprintln!(
                    "Skipping {}: {e}",
                    path.file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or_default()
                );
                continue;
            }
        };
        let original_id = note.id.clone();
        if note.created.trim().is_empty() {
            note.created = timestamp_string();
        }
        if note.updated.trim().is_empty() {
            note.updated = note.created.clone();
        }
        let mut final_id = original_id.clone();
        if reserved.contains(&final_id) {
            final_id = generate_new_id(dir, &mut reserved)?;
            note.id = final_id.clone();
        }
        reserved.insert(final_id.clone());
        write_note(&note, &batch_dir)?;
        migrated += 1;
        if final_id == original_id {
            println!("Migrated {original_id} into migrated/{batch_id}");
        } else {
            println!(
                "Migrated {original_id} -> {final_id} into migrated/{batch_id}"
            );
        }
    }

    if migrated == 0 {
        println!("No notes migrated.");
    } else {
        println!("Imported {migrated} note(s) into {}", batch_dir.display());
    }
    Ok(())
}
