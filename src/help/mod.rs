use crate::{paginate_and_print, terminal_columns};
use std::error::Error;

mod content;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    All,
    Guides,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Section {
    Command,
    Environment,
    Guide,
}

impl Section {
    fn label(self) -> &'static str {
        match self {
            Section::Command => "Commands",
            Section::Environment => "Environment",
            Section::Guide => "Guides",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct HelpFlag<'a> {
    pub name: &'a str,
    pub desc: &'a str,
}

#[derive(Clone, Copy)]
pub(crate) struct HelpTopic<'a> {
    pub name: &'a str,
    pub summary: &'a str,
    pub usage: &'a str,
    pub details: &'a [&'a str],
    pub flags: &'a [HelpFlag<'a>],
    pub aliases: &'a [&'a str],
    pub section: Section,
    pub examples: &'a [&'a str],
}

#[derive(Clone, Copy)]
pub(crate) struct HelpBook<'a> {
    pub title: &'a str,
    pub usage: &'a str,
    pub topics: &'a [HelpTopic<'a>],
    pub footer: &'a [&'a str],
}

impl<'a> HelpBook<'a> {
    fn find(&self, name: &str) -> Option<&HelpTopic<'a>> {
        let needle = name.to_ascii_lowercase();
        self.topics.iter().find(|topic| {
            topic.name.eq_ignore_ascii_case(&needle)
                || topic.aliases.iter().any(|a| a.eq_ignore_ascii_case(&needle))
        })
    }

    fn in_section(
        &self,
        section: Section,
    ) -> impl Iterator<Item = &HelpTopic<'a>> {
        self.topics.iter().filter(move |t| t.section == section)
    }
}

pub(crate) fn run(args: Vec<String>) -> Result<(), Box<dyn Error>> {
    run_with_mode(args, Mode::All)
}

pub(crate) fn run_guides(args: Vec<String>) -> Result<(), Box<dyn Error>> {
    run_with_mode(args, Mode::Guides)
}

fn run_with_mode(args: Vec<String>, mode: Mode) -> Result<(), Box<dyn Error>> {
    let book = content::book();
    let width = terminal_columns().unwrap_or(96).clamp(64, 120);
    let printer = HelpPrinter::new(width);

    let lines = if args.is_empty() {
        match mode {
            Mode::All => printer.render_overview(&book),
            Mode::Guides => printer.render_guides(&book),
        }
    } else {
        let topic = args[0].as_str();
        match book.find(topic) {
            Some(entry)
                if mode == Mode::Guides && entry.section != Section::Guide =>
            {
                eprintln!("Unknown guide: {topic}");
                printer.render_guides(&book)
            }
            Some(entry) => printer.render_topic(&book, entry),
            None => {
                eprintln!("Unknown help topic: {topic}");
                match mode {
                    Mode::All => printer.render_overview(&book),
                    Mode::Guides => printer.render_guides(&book),
                }
            }
        }
    };

    paginate_and_print(&lines)?;
    Ok(())
}

struct HelpPrinter {
    width: usize,
}

impl HelpPrinter {
    fn new(width: usize) -> Self {
        Self { width }
    }

    fn render_overview(&self, book: &HelpBook<'_>) -> Vec<String> {
        let mut out = Vec::new();
        out.push(book.title.to_string());
        out.push(format!("usage: {}", book.usage));
        out.push(String::new());

        let commands: Vec<(String, String)> = book
            .in_section(Section::Command)
            .map(|t| (t.usage.to_string(), t.summary.to_string()))
            .collect();
        out.extend(self.render_block(Section::Command.label(), &commands));

        let env: Vec<(String, String)> = book
            .in_section(Section::Environment)
            .map(|t| (t.usage.to_string(), t.summary.to_string()))
            .collect();
        if !env.is_empty() {
            out.extend(self.render_block(Section::Environment.label(), &env));
        }

        let guides: Vec<(String, String)> = book
            .in_section(Section::Guide)
            .map(|t| (t.name.to_string(), t.summary.to_string()))
            .collect();
        if !guides.is_empty() {
            out.extend(self.render_block(Section::Guide.label(), &guides));
        }

        for line in book.footer {
            for l in self.wrap(line, self.width) {
                out.push(l);
            }
        }
        out
    }

    fn render_guides(&self, book: &HelpBook<'_>) -> Vec<String> {
        let mut out = Vec::new();
        out.push(format!("{} (guides)", book.title));
        out.push("usage: qn guide [topic]".to_string());
        out.push(String::new());

        let guides: Vec<(String, String)> = book
            .in_section(Section::Guide)
            .map(|t| (t.name.to_string(), t.summary.to_string()))
            .collect();
        out.extend(self.render_block(Section::Guide.label(), &guides));

        for line in book.footer {
            for l in self.wrap(line, self.width) {
                out.push(l);
            }
        }
        out
    }

    fn render_topic(
        &self,
        book: &HelpBook<'_>,
        topic: &HelpTopic<'_>,
    ) -> Vec<String> {
        let mut out = Vec::new();
        out.push(format!("{} — {}", topic.name, topic.summary));
        out.push(format!("usage: {}", topic.usage));
        if !topic.aliases.is_empty() {
            out.push(format!("aliases: {}", topic.aliases.join(", ")));
        }
        out.push(String::new());

        for line in topic.details {
            for l in self.wrap(line, self.width) {
                out.push(l);
            }
        }
        if !topic.details.is_empty() {
            out.push(String::new());
        }

        if !topic.flags.is_empty() {
            let flags: Vec<(String, String)> = topic
                .flags
                .iter()
                .map(|f| (f.name.to_string(), f.desc.to_string()))
                .collect();
            out.extend(self.render_block("Options", &flags));
        }

        if !topic.examples.is_empty() {
            out.push("Examples:".to_string());
            for ex in topic.examples {
                for l in self.wrap(ex, self.width.saturating_sub(2)) {
                    out.push(format!("  {l}"));
                }
            }
            out.push(String::new());
        }

        for line in book.footer {
            for l in self.wrap(line, self.width) {
                out.push(l);
            }
        }
        out
    }

    fn render_block(
        &self,
        title: &str,
        rows: &[(String, String)],
    ) -> Vec<String> {
        if rows.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let min_desc = self.width / 2;
        let mut label_width =
            rows.iter().map(|r| r.0.len()).max().unwrap_or(0).min(38);
        if label_width + 4 + min_desc > self.width {
            label_width = self.width.saturating_sub(min_desc + 4);
        }
        let desc_width =
            self.width.saturating_sub(2 + label_width + 2).max(min_desc);

        out.push(format!("{title}:"));
        for (label, desc) in rows {
            let label_lines = self.wrap(label, label_width);
            let desc_lines = self.wrap(desc, desc_width);
            let rows = label_lines.len().max(desc_lines.len());
            for idx in 0..rows {
                let l = label_lines.get(idx).map(String::as_str).unwrap_or("");
                let d = desc_lines.get(idx).map(String::as_str).unwrap_or("");
                out.push(format!(
                    "  {:label_width$}  {}",
                    l,
                    d,
                    label_width = label_width
                ));
            }
        }
        out.push(String::new());
        out
    }

    fn wrap(&self, text: &str, width: usize) -> Vec<String> {
        let mut out = Vec::new();
        let mut line = String::new();
        for word in text.split_whitespace() {
            if line.is_empty() {
                line.push_str(word);
                continue;
            }
            if line.len() + 1 + word.len() <= width {
                line.push(' ');
                line.push_str(word);
            } else {
                out.push(line);
                line = word.to_string();
            }
        }
        if !line.is_empty() {
            out.push(line);
        }
        if out.is_empty() {
            out.push(String::new());
        }
        out
    }
}
