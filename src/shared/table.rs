//! Table and text layout helpers used by the CLI.
//! Keeps ANSI-aware width calculations and simple table rendering in one place.

/// Render a simple text table. Column widths are auto-computed from the widest
/// cell (header or row) using display lengths that ignore ANSI color codes.
pub fn render_table(headers: &[String], rows: &[Vec<String>]) -> String {
    if headers.is_empty() {
        return String::new();
    }
    let cols = headers.len();
    let mut widths: Vec<usize> =
        headers.iter().map(|h| display_len(h)).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate().take(cols) {
            widths[i] = widths[i].max(display_len(cell));
        }
    }

    let mut out = String::new();
    out.push_str(&format_row(headers, &widths));
    out.push('\n');
    out.push_str(&"=".repeat(display_len(&format_row(headers, &widths))));
    for row in rows {
        out.push('\n');
        out.push_str(&format_row(row, &widths));
    }
    out
}

fn format_row(row: &[String], widths: &[usize]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (cell, width) in row.iter().zip(widths.iter()) {
        let plain_len = display_len(cell);
        parts.push(pad_field(cell, *width, plain_len));
    }
    parts.join(" | ")
}

/// Right-pad a field based on visible length (ignoring ANSI codes).
pub fn pad_field(display: &str, target: usize, plain_len: usize) -> String {
    let mut out = display.to_string();
    let padding = target.saturating_sub(plain_len);
    out.push_str(&" ".repeat(padding));
    out
}

/// Truncate text to a width, appending an ellipsis when needed.
pub fn truncate_with_ellipsis(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let len = text.chars().count();
    if len <= max_width {
        return text.to_string();
    }
    if max_width == 1 {
        return "…".to_string();
    }
    let mut out =
        text.chars().take(max_width.saturating_sub(1)).collect::<String>();
    out.push('…');
    out
}

/// Compute visible length of a string, ignoring ANSI escape sequences.
pub fn display_len(s: &str) -> usize {
    let mut len = 0;
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            while let Some(next) = chars.next() {
                if next == 'm' {
                    break;
                }
            }
            continue;
        }
        len += 1;
    }
    len
}
