# Contribute & Develop

Quick Notes is a small Rust CLI that stores UTF-8 Markdown files under
`~/.quick_notes` by default (overridable with `QUICK_NOTES_DIR`). This guide
explains how it is built and how to extend it.

## Getting started

- Prereqs: Rust toolchain, `cargo fmt`, `cargo check`.
- Build/install: `cargo install --path .` (or run locally with
  `cargo run -- <cmd>`).
- Preferred alias: `alias qn=quick_notes` for fast terminal use.

## Architecture

- Single binary (`src/main.rs`) with a thin command dispatcher and a few focused
  helpers.
- Storage: one Markdown file per note in the notes directory; each file has a
  short header (`Title`, `Created`, `Updated`, separator line, then body).
- Timestamps: generated in US-local format (`%m/%d/%Y %I:%M %p %:z`) via
  `chrono::Local`.
- Rendering: Markdown is parsed with `pulldown-cmark` and reflowed into a
  terminal-friendly text view (headings, lists, horizontal rules) for the
  `render`/`view --render` mode.
- Editing: Opens the selected file in `$EDITOR` (fallback `vi`), then refreshes
  the `Updated` header and rewrites the file.

## Code walkthrough (key functions)

- `main` — parses the first CLI argument as a command and routes to subcommands.
- `print_help` — prints usage, commands, and environment hints.
- `notes_dir` — resolves the storage path from `QUICK_NOTES_DIR` or defaults to
  `~/.quick_notes`.
- `ensure_dir` — creates the notes directory on demand.
- `quick_add` / `new_note` — build a `Note`, stamp timestamps, and persist.
- `list_notes` — reads `.md` files, parses metadata, sorts by updated time, and
  prints a summary line per note.
- `view_note` — loads a note and prints either the raw body or the rendered
  Markdown view (when forced or when `--render`/`-r` is passed).
- `edit_note` — opens the file in an editor, then re-parses and rewrites to
  refresh the `Updated` timestamp.
- `create_note` / `write_note` / `note_path` / `unique_id` — handle note
  creation, safe file naming, and header/body serialization.
- `parse_note` — reads UTF-8 text, extracts the header, and returns a `Note`.
- `timestamp_string` / `parse_timestamp` — generate and parse US-local
  timestamps to sort notes correctly.
- `render_markdown` — walks the pulldown-cmark event stream to produce a
  readable terminal rendering of headings, lists, rules, and text.

## Style and encoding

- UTF-8 only for inputs/outputs; keep editors and tests in UTF-8.
- Keep code formatted with `cargo fmt`; keep dependencies lean and fast.
- Add short comments only where logic is non-obvious.

## Versioning and release process

- Follow semantic versioning in `Cargo.toml`.
- Record every user-visible change in `CHANGELOG.md` before tagging or
  publishing.
- Bump the crate version and changelog together when you ship a new release.

## Developing new features

- Keep the CLI surface minimal and fast; prefer additive flags over breaking
  changes.
- For new commands, add usage hints to `print_help` and update `README.md` and
  `CHANGELOG.md`.
- Preserve the storage format (header + `---` + body) for backward
  compatibility.
- When touching timestamps, keep the US-local format to match existing notes.

## Testing

- Run `cargo fmt` and `cargo test` before sending or committing changes.
- For behavior changes, add lightweight integration checks (e.g., create a temp
  notes dir and run the commands via `cargo run -- ...`) to confirm end-to-end
  flows.
