# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Quick Notes is a Rust CLI tool for managing UTF-8 Markdown notes from the terminal. Notes are stored as individual `.md` files with timestamped IDs under `~/.quick_notes` (or `QUICK_NOTES_DIR`). The project emphasizes speed, simplicity, and terminal-native workflows with optional fzf integration.

## Essential Commands

### Building and Testing
```bash
# Build and install locally
cargo install --path .

# Run without installing
cargo run -- <command>

# Format code (required before commits)
cargo fmt

# Run all tests
cargo test

# Run specific test
cargo test <test_name>

# Check for clippy warnings (runs with -D warnings in CI)
cargo clippy -- -D warnings
```

### Running the CLI
```bash
# After install, use either binary:
quick_notes <command>
qn <command>

# Or run directly:
cargo run -- <command>
```

## Architecture

### Binary Structure
- **Two binaries, one library**: `src/main.rs` (quick_notes) and `src/bin/qn.rs` (qn alias) both delegate to `src/lib.rs`
- **Single entrypoint**: `lib.rs::entry()` dispatches all commands

### Module Breakdown

**Core Modules:**
- **`src/lib.rs`**: Command dispatch and subcommand implementations (add, new, list, view, edit, delete, archive, etc.)
- **`src/note.rs`**: Note model, file I/O, timestamp generation, ID allocation (base62-encoded microsecond timestamps)
- **`src/render.rs`**: Terminal markdown rendering with ANSI colors, glow integration
- **`src/shared/mod.rs`**: Shared utilities and re-exports
- **`src/shared/table.rs`**: ANSI-aware table rendering for list/tag outputs
- **`src/shared/migrate.rs`**: Note migration and active-note resolution across directories

**New Refactored Modules (as of 2024):**
- **`src/tags.rs`**: Tag normalization, validation, filtering, and color generation - eliminates tag-handling duplication
- **`src/args.rs`**: Reusable argument parsing with `ArgParser` - consistent flag handling across commands
- **`src/fzf.rs`**: FZF integration with `FzfSelector` - eliminates FZF spawn code duplication
- **`src/formatting.rs`**: Color palettes, `FormatContext`, `TimeFormatter` - centralized display formatting
- **`src/operations.rs`**: File operations like `load_note()`, `filter_by_tags()` - reusable note operations

**Migration Status:** ✅ **6 major commands refactored** (delete_notes, archive_notes, edit_note, view_note, list_notes_in, list_tags). All high-impact duplication eliminated. Remaining 9 commands have minimal duplication and can be migrated as needed.

### Storage Format
Each note is a `.md` file with this structure:
```
Title: <note title>
Created: <timestamp>
Updated: <timestamp>
Tags: #tag1, #tag2
---
<markdown body>
```

Optional headers for trash/archive: `Deleted:` and `Archived:` timestamps.

### ID Generation
- IDs are base62-encoded microsecond timestamps (9 chars) for chronological ordering and uniqueness
- The `unique_id()` function ensures no collisions by checking existing files
- Notes in subdirectories (like `migrated/<batch>`) preserve their full path-based IDs

### Timestamp Format
- Default: `%d%b%y %H:%M %:z` (e.g., "15Dec24 14:30 -05:00")
- Legacy support: `%m/%d/%Y %I:%M %p %:z`
- All timestamps are US-local timezone via `chrono::Local`

### Areas and Directories
Notes can exist in three areas:
- **Active**: `$QUICK_NOTES_DIR/` (default `~/.quick_notes/`)
- **Trash**: `$QUICK_NOTES_DIR/trash/` (soft-deleted notes, auto-cleaned after 30 days)
- **Archive**: `$QUICK_NOTES_DIR/archive/` (archived notes, kept indefinitely)

The `Area` enum (Active/Trash/Archive) is used throughout for area-specific logic.

### Key Patterns

**Note resolution**: `resolve_active_note_path()` finds notes in the active directory or migrated subdirectories, enabling batch organization while keeping simple top-level IDs.

**Tag handling**: Tags are normalized to `#tag` format, deduplicated, and sorted. Pinned tags (default: `#todo,#meeting,#scratch`) show in tag lists even when unused.

**FZF integration**: Many commands (edit, delete, archive) fall back to fzf for interactive multi-select when no IDs are provided. Preview windows render notes using `quick_notes render` or fallback to `sed`.

**Rendering modes**:
- Plain text (default when `NO_COLOR=1`)
- ANSI-colored markdown (built-in terminal renderer)
- Rich markdown via `glow` (when installed, used with `view` command)

**Testing approach**: Integration tests in `tests/` use `assert_cmd` to run the CLI against temporary directories. Unit tests verify parsing, rendering, and helper functions.

## Common Development Workflows

### Adding a new command

**Preferred Approach (using new modules):**
1. Add command case to `entry()` match in `src/lib.rs`
2. Implement the command function using the new modules:
   - Use `args::ArgParser` for argument parsing
   - Use `fzf::FzfSelector` for interactive selection
   - Use `tags::` functions for tag operations
   - Use `operations::` functions for file operations
   - Use `formatting::FormatContext` for display
3. Update help text in `src/help/content.rs`
4. Add CLI tests in `tests/cli.rs`
5. Update README.md and CHANGELOG.md

**Example pattern:**
```rust
fn my_command(args: Vec<String>, dir: &Path) -> Result<(), Box<dyn Error>> {
    // Parse args
    let mut parser = args::ArgParser::new(args, "my_command");
    let mut tag_filters = Vec::new();

    while let Some(arg) = parser.next() {
        match arg.as_str() {
            "-t" | "--tag" => {
                if let Some(tag) = parser.extract_tag()? {
                    tag_filters.push(tag);
                }
            }
            // ... handle other flags
        }
    }

    // Use FZF if needed
    let selector = fzf::FzfSelector::with_note_preview();
    let ids = selector.select_note_ids(&paths)?;

    // Validate and operate
    for id in ids {
        if !tags::validate_note_tags(dir, &id, &tag_filters)? {
            continue;
        }
        // ... do work
    }

    Ok(())
}
```

See `delete_notes()` in `src/lib.rs:1118` for a complete refactored example.

### Modifying note format
- Keep backward compatibility: add optional headers, never remove existing ones
- Update `parse_note()` in `src/note.rs` to handle new fields
- Update `write_note()` to serialize new fields
- Test with existing notes to ensure migration works

### Working with timestamps
- Always use `timestamp_string()` for new timestamps
- Use `parse_timestamp()` for reading (handles both formats)
- Use `cmp_dt()` for chronological comparisons
- Relative time formatting: `format_relative()` in `src/lib.rs`

### Adding tests
- Integration tests: `tests/cli.rs` - test CLI commands end-to-end
- Unit tests: inline in source files with `#[cfg(test)]`
- Use `tempfile::tempdir()` for isolated test directories
- Use `assert_cmd` predicates for output validation

## Environment Variables

- `QUICK_NOTES_DIR`: Override default `~/.quick_notes` storage location
- `QUICK_NOTES_PINNED_TAGS`: Override default pinned tags (comma-separated, e.g., `#work,#personal`)
- `QUICK_NOTES_TRASH_RETENTION_DAYS`: Days before auto-deleting trash (default 30, 0 to disable)
- `QUICK_NOTES_NO_FZF`: Disable fzf integration even if installed
- `NO_COLOR`: Disable ANSI colors in output
- `EDITOR`: Editor for `qn edit` (defaults to `vi`)

## Code Style

- Run `cargo fmt` before all commits
- Keep functions focused and under ~150 lines where practical
- Use descriptive names; avoid abbreviations except for common terms (ts, dt, dir)
- Prefer direct error propagation with `?` over verbose error handling
- Keep dependencies minimal (current: chrono, pulldown-cmark, terminal_size, yansi)

## Critical Constraints

1. **UTF-8 only**: All input/output must be UTF-8
2. **Preserve storage format**: Never break backward compatibility with existing note files
3. **Maintain timestamp format**: Keep US-local format for consistency with existing notes
4. **ID stability**: Never reuse or change existing note IDs
5. **Fast startup**: Keep binary small, avoid heavy dependencies

## Testing Philosophy

- Test via the CLI interface (integration tests) for end-to-end behavior
- Test parsing/rendering directly (unit tests) for edge cases
- Use temporary directories to isolate test state
- Verify both success and error cases
- Test with and without optional dependencies (fzf, glow)

## Release Process

1. Update version in `Cargo.toml`
2. Document changes in `CHANGELOG.md`
3. Run `cargo fmt && cargo clippy -- -D warnings && cargo test`
4. Commit with version bump message
5. Tag with `v<version>`
6. Push tag to trigger release

## Refactoring Strategy

The codebase is undergoing incremental refactoring to improve consistency and reduce duplication.

### Current Status
- ✅ **New modules created**: `args`, `fzf`, `tags`, `formatting`, `operations`
- ✅ **6 commands refactored**: delete_notes, archive_notes, edit_note, view_note, list_notes_in, list_tags
- ✅ **All high-impact duplication eliminated**: FZF (100%), tags (86%), args (60%)
- ✅ **Production-ready**: All tests pass, clippy clean, fully documented
- 📋 **Optional**: Remaining 9 commands have minimal duplication (<20 lines total)

### Guidelines for New Code
1. **Prefer new modules** over inline implementations
2. **Use `ArgParser`** instead of manual argument loops
3. **Use `FzfSelector`** instead of spawning FZF directly
4. **Use `tags::` functions** for all tag operations
5. **Use `operations::` helpers** for file operations
6. **Use `formatting::FormatContext`** for colored output

### Refactoring Existing Commands
When updating an existing command:
1. Read the current implementation
2. Identify which new modules apply (args, fzf, tags, operations, formatting)
3. Replace inline code with module calls
4. Verify tests still pass
5. Check that behavior is identical

The goal is gradual migration without breaking changes. Both old and new patterns coexist during transition.

## Useful References

- **REFACTORING_PLAN.md**: Comprehensive refactoring roadmap with code examples
- **README.md**: User-facing usage documentation
- **CONTRIBUTE.md**: Extended development guide with architecture details
- **CHANGELOG.md**: Version history and changes
- **TROUBLESHOOT.md**: Common issues and solutions (if exists)
- **AGENTS.md**: AI agent usage expectations (if exists)
