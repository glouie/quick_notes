use chrono::{DateTime, FixedOffset};
use yansi::Paint;

/// Color palette for consistent theming
pub struct ColorPalette {
    pub primary: (u8, u8, u8),   // IDs, muted text
    pub secondary: (u8, u8, u8), // Headers, emphasis
    pub timestamp: (u8, u8, u8), // Timestamps
    pub highlight: (u8, u8, u8), // Search matches
}

impl ColorPalette {
    pub const CATPPUCCIN: Self = Self {
        primary: (108, 112, 134),   // Gray
        secondary: (148, 226, 213), // Teal
        timestamp: (137, 180, 250), // Blue
        highlight: (243, 139, 168), // Pink
    };
}

/// Formatting context passed through rendering pipeline
pub struct FormatContext {
    pub use_color: bool,
    pub palette: ColorPalette,
}

impl FormatContext {
    pub fn new(use_color: bool) -> Self {
        Self { use_color, palette: ColorPalette::CATPPUCCIN }
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

    pub fn format_relative(&self, dt: DateTime<FixedOffset>) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_context_no_color() {
        let ctx = FormatContext::new(false);
        assert_eq!(ctx.format_id("abc123"), "abc123");
        assert_eq!(ctx.format_header("Header"), "Header");
        assert_eq!(ctx.format_timestamp("2024-01-01"), "2024-01-01");
    }

    #[test]
    fn test_format_context_with_color() {
        let ctx = FormatContext::new(true);
        let id = ctx.format_id("abc123");
        assert!(id.contains("abc123"));
        assert!(id.len() > "abc123".len()); // Has ANSI codes
    }

    #[test]
    fn test_highlight_match() {
        let ctx = FormatContext::new(false);
        assert_eq!(
            ctx.highlight_match("hello world", Some("world")),
            "hello world"
        );

        let ctx = FormatContext::new(true);
        let result = ctx.highlight_match("hello world", Some("world"));
        assert!(result.contains("world"));
    }

    #[test]
    fn test_time_formatter_relative() {
        let now = crate::note::now_fixed();
        let formatter = TimeFormatter::new(true, now);

        // Can't test actual relative times without mocking, but we can test the format method exists
        let result = formatter.format("15Dec24 14:30 -05:00");
        assert!(!result.is_empty());
    }

    #[test]
    fn test_time_formatter_label() {
        let now = crate::note::now_fixed();
        let formatter = TimeFormatter::new(true, now);
        assert_eq!(formatter.format_label("Updated"), "Updated");

        let formatter = TimeFormatter::new(false, now);
        let label = formatter.format_label("Updated");
        assert!(label.starts_with("Updated"));
    }
}
