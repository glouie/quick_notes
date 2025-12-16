use crate::note::Note;
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

/// Get color for a tag based on hash
pub fn color_for_tag(tag: &str) -> (u8, u8, u8) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_tag() {
        assert_eq!(normalize_tag("todo"), "#todo");
        assert_eq!(normalize_tag("#todo"), "#todo");
        assert_eq!(normalize_tag("  work  "), "#work");
        assert_eq!(normalize_tag(""), "");
    }

    #[test]
    fn test_normalize_tags() {
        let tags =
            vec!["todo".to_string(), "#work".to_string(), "todo".to_string()];
        let result = normalize_tags(tags);
        assert_eq!(result, vec!["#todo", "#work"]);
    }

    #[test]
    fn test_note_has_tags() {
        let note = Note {
            id: "test".to_string(),
            title: "Test".to_string(),
            created: "now".to_string(),
            updated: "now".to_string(),
            deleted_at: None,
            archived_at: None,
            body: "body".to_string(),
            tags: vec!["#todo".to_string(), "#work".to_string()],
            size_bytes: 0,
        };

        assert!(note_has_tags(&note, &[]));
        assert!(note_has_tags(&note, &["#todo".to_string()]));
        assert!(note_has_tags(
            &note,
            &["#todo".to_string(), "#work".to_string()]
        ));
        assert!(!note_has_tags(&note, &["#missing".to_string()]));
    }

    #[test]
    fn test_hash_tag_deterministic() {
        let h1 = hash_tag("todo");
        let h2 = hash_tag("todo");
        assert_eq!(h1, h2);

        let h3 = hash_tag("work");
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_color_for_tag_consistent() {
        let c1 = color_for_tag("todo");
        let c2 = color_for_tag("todo");
        assert_eq!(c1, c2);
    }
}
