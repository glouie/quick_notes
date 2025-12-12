use std::process::{Command, Stdio};

use yansi::Paint;

/// Minimal styling categories used when coloring markdown output.
#[derive(Clone, Copy)]
enum Style {
    Heading,
    Bullet,
    Rule,
    Code,
}

/// Render markdown with lightweight styling. When `use_color` is false the
/// original text is returned unchanged so whitespace and line counts stay
/// stable for tests.
pub fn render_markdown(input: &str, use_color: bool) -> String {
    if !use_color {
        return input.to_string();
    }

    let mut rendered = String::new();
    let mut in_code_block = false;

    for segment in input.split_inclusive('\n') {
        let (line, newline) = if let Some(stripped) = segment.strip_suffix('\n')
        {
            (stripped, "\n")
        } else {
            (segment, "")
        };
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") {
            rendered.push_str(&push_painted(line, Style::Code, true));
            rendered.push_str(newline);
            in_code_block = !in_code_block;
            continue;
        }

        if in_code_block {
            rendered.push_str(&push_painted(line, Style::Code, true));
            rendered.push_str(newline);
            continue;
        }

        let styled_line = if trimmed.starts_with('#') {
            push_painted(line, Style::Heading, true)
        } else if trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.starts_with("+ ")
            || trimmed
                .split_once('.')
                .map(|(a, _)| a.chars().all(|c| c.is_ascii_digit()))
                .unwrap_or(false)
        {
            push_painted(line, Style::Bullet, true)
        } else if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            push_painted(line, Style::Rule, true)
        } else {
            highlight_inline_code(line)
        };

        rendered.push_str(&styled_line);
        rendered.push_str(newline);
    }

    rendered
}

pub fn highlight_inline_code(line: &str) -> String {
    if !line.contains('`') {
        return line.to_string();
    }
    let mut out = String::new();
    let mut rest = line;

    while let Some(start) = rest.find('`') {
        let (before, after_tick) = rest.split_at(start);
        out.push_str(before);
        let after_tick = &after_tick[1..];
        if let Some(end) = after_tick.find('`') {
            let (code, after) = after_tick.split_at(end);
            out.push('`');
            out.push_str(&Paint::blue(code).to_string());
            out.push('`');
            rest = &after[1..];
        } else {
            out.push('`');
            out.push_str(after_tick);
            return out;
        }
    }
    out.push_str(rest);
    out
}

fn push_painted(text: &str, style: Style, use_color: bool) -> String {
    if !use_color {
        return text.to_string();
    }
    match style {
        Style::Heading => Paint::cyan(text).bold().to_string(),
        Style::Bullet => Paint::yellow(text).bold().to_string(),
        Style::Rule => Paint::new(text).dim().to_string(),
        Style::Code => Paint::blue(text).to_string(),
    }
}

/// Prefer `glow` for rich markdown rendering if available.
pub fn detect_glow() -> Option<&'static str> {
    if Command::new("glow")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .map_or(false, |s| s.success())
    {
        return Some("glow");
    }
    None
}
