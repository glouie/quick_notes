use std::error::Error;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

pub struct FzfSelector {
    preview_command: Option<String>,
    multi_select: bool,
    height: Option<String>,
    layout: Option<String>,
    preview_window: Option<String>,
}

impl FzfSelector {
    pub fn new() -> Self {
        Self {
            preview_command: None,
            multi_select: false,
            height: None,
            layout: None,
            preview_window: None,
        }
    }

    /// Create selector with note preview using the renderer
    pub fn with_note_preview() -> Self {
        let renderer = get_renderer_name();
        let preview = format!(
            "env -u NO_COLOR CLICOLOR_FORCE=1 {} render {{}} 2>/dev/null",
            renderer
        );
        Self {
            preview_command: Some(preview),
            multi_select: true,
            height: None,
            layout: None,
            preview_window: None,
        }
    }

    /// Create selector with simple sed preview
    pub fn with_simple_preview() -> Self {
        Self {
            preview_command: Some("sed -n '1,120p' {}".to_string()),
            multi_select: true,
            height: Some("70%".to_string()),
            layout: Some("reverse".to_string()),
            preview_window: Some("down:wrap".to_string()),
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

    pub fn layout(mut self, layout: &str) -> Self {
        self.layout = Some(layout.to_string());
        self
    }

    /// Select from a list of file paths
    pub fn select_from_paths(
        &self,
        paths: &[PathBuf],
    ) -> Result<Vec<String>, Box<dyn Error>> {
        let input = paths
            .iter()
            .map(|p| p.to_string_lossy())
            .collect::<Vec<_>>()
            .join("\n");

        self.select_from_input(&input)
    }

    /// Select from raw input string
    pub fn select_from_input(
        &self,
        input: &str,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        if !is_fzf_available() {
            return Err(
                "fzf is not installed or QUICK_NOTES_NO_FZF is set".into()
            );
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

        if let Some(ref preview_window) = self.preview_window {
            cmd.arg("--preview-window").arg(preview_window);
        }

        let mut child =
            cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;

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

    /// Select note IDs from file paths
    pub fn select_note_ids(
        &self,
        paths: &[PathBuf],
    ) -> Result<Vec<String>, Box<dyn Error>> {
        // Extract IDs first, then pass them to FZF
        let ids: Vec<String> = paths
            .iter()
            .filter_map(|p| {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            })
            .collect();

        let input = ids.join("\n");
        let selected = self.select_from_input(&input)?;

        Ok(selected)
    }
}

impl Default for FzfSelector {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if fzf is available
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

/// Get the renderer binary name (cached)
fn get_renderer_name() -> &'static str {
    static RENDERER: OnceLock<&str> = OnceLock::new();
    RENDERER.get_or_init(|| {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fzf_selector_builder() {
        let selector = FzfSelector::new()
            .multi_select(true)
            .height("50%")
            .layout("reverse");

        assert!(selector.multi_select);
        assert_eq!(selector.height.as_deref(), Some("50%"));
        assert_eq!(selector.layout.as_deref(), Some("reverse"));
    }

    #[test]
    fn test_with_note_preview() {
        let selector = FzfSelector::with_note_preview();
        assert!(selector.multi_select);
        assert!(selector.preview_command.is_some());
        assert!(selector.preview_command.unwrap().contains("render"));
    }

    #[test]
    fn test_with_simple_preview() {
        let selector = FzfSelector::with_simple_preview();
        assert!(selector.multi_select);
        assert_eq!(selector.height.as_deref(), Some("70%"));
        assert_eq!(selector.layout.as_deref(), Some("reverse"));
    }

    #[test]
    fn test_get_renderer_name_cached() {
        let name1 = get_renderer_name();
        let name2 = get_renderer_name();
        assert_eq!(name1, name2);
        assert!(name1 == "quick_notes" || name1 == "qn");
    }
}
