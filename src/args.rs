use std::error::Error;

pub struct ArgParser {
    iter: std::vec::IntoIter<String>,
    command_name: String,
}

impl ArgParser {
    pub fn new(args: Vec<String>, command_name: &str) -> Self {
        Self { iter: args.into_iter(), command_name: command_name.to_string() }
    }

    /// Extract a single tag from -t/--tag flag
    pub fn extract_tag(&mut self) -> Result<Option<String>, Box<dyn Error>> {
        match self.iter.next() {
            Some(v) => {
                let tag = crate::tags::normalize_tag(&v);
                if tag.is_empty() {
                    Err(format!(
                        "Invalid tag provided to {}",
                        self.command_name
                    )
                    .into())
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

    /// Extract a string value for a flag
    pub fn extract_value(
        &mut self,
        flag: &str,
    ) -> Result<String, Box<dyn Error>> {
        self.iter.next().ok_or_else(|| {
            format!("Provide a value after {} for {}", flag, self.command_name)
                .into()
        })
    }

    /// Check if there are remaining arguments
    pub fn has_more(&self) -> bool {
        self.iter.len() > 0
    }

    /// Get next positional argument
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<String> {
        self.iter.next()
    }

    /// Collect remaining args
    pub fn collect_remaining(self) -> Vec<String> {
        self.iter.collect()
    }
}

/// Common flags used across multiple commands
#[derive(Default, Debug)]
pub struct CommonFlags {
    pub tag_filters: Vec<String>,
    pub search: Option<String>,
    pub relative_time: bool,
    pub show_all: bool,
    pub use_fzf: bool,
    pub sort_field: String,
    pub ascending: bool,
    pub plain: bool,
    pub render: bool,
    pub positional: Vec<String>,
}

impl CommonFlags {
    pub fn new() -> Self {
        Self { sort_field: "updated".to_string(), ..Default::default() }
    }
}

/// Parse tags from argument iterator (for simple tag extraction)
pub fn parse_tags_from_iter(
    iter: &mut std::vec::IntoIter<String>,
    command_name: &str,
) -> Result<Vec<String>, Box<dyn Error>> {
    let mut tags = Vec::new();
    loop {
        match iter.next() {
            Some(arg) if arg == "-t" || arg == "--tag" => {
                if let Some(v) = iter.next() {
                    let tag = crate::tags::normalize_tag(&v);
                    if !tag.is_empty() {
                        tags.push(tag);
                    }
                } else {
                    return Err(format!(
                        "Provide a tag after -t/--tag for {}",
                        command_name
                    )
                    .into());
                }
            }
            Some(other) => {
                // Put it back conceptually - caller should handle
                return Err(format!("Unexpected argument: {}", other).into());
            }
            None => break,
        }
    }
    Ok(tags)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arg_parser_extract_tag() {
        let args = vec!["-t".to_string(), "todo".to_string()];
        let mut parser = ArgParser::new(args, "test");
        let flag = parser.next().unwrap();
        assert_eq!(flag, "-t");
        let tag = parser.extract_tag().unwrap();
        assert_eq!(tag, Some("#todo".to_string()));
    }

    #[test]
    fn test_arg_parser_extract_value() {
        let args = vec!["--sort".to_string(), "created".to_string()];
        let mut parser = ArgParser::new(args, "test");
        let flag = parser.next().unwrap();
        assert_eq!(flag, "--sort");
        let value = parser.extract_value("--sort").unwrap();
        assert_eq!(value, "created");
    }

    #[test]
    fn test_arg_parser_collect_remaining() {
        let args =
            vec!["id1".to_string(), "id2".to_string(), "id3".to_string()];
        let mut parser = ArgParser::new(args, "test");
        let remaining = parser.collect_remaining();
        assert_eq!(remaining, vec!["id1", "id2", "id3"]);
    }

    #[test]
    fn test_common_flags_default() {
        let flags = CommonFlags::new();
        assert_eq!(flags.sort_field, "updated");
        assert!(!flags.ascending);
        assert!(flags.tag_filters.is_empty());
    }
}
