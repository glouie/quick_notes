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

- Binaries: `src/main.rs` (`quick_notes`) and `src/bin/qn.rs` (`qn` symlink)
  forward to the shared library entrypoint.
- Library modules:
  - `src/lib.rs` — command dispatch and wiring between subcommands.
  - `src/note.rs` — note model, storage paths, ID/time helpers, read/write.
  - `src/render.rs` — markdown rendering (ANSI) and `glow` detection.
  - `src/table.rs` — ANSI-aware width helpers and generic table rendering.
- Storage: one Markdown file per note in the notes directory; each file has a
  short header (`Title`, `Created`, `Updated`, `Tags`, separator line, then
  body).
- Timestamps: generated in US-local format (`%m/%d/%Y %I:%M %p %:z`) via
  `chrono::Local`.
- Rendering: markdown rendered for terminal display (headings, lists, rules,
  inline code); falls back to plain when `NO_COLOR` is set; uses `glow` when
  available for `view -r`.
- Editing: opens the selected file in `$EDITOR` (fallback `vi`), then re-parses
  and rewrites to refresh the `Updated` timestamp.

## Code walkthrough (key functions)

- `entry` (lib) — parses the first CLI argument as a command and routes to
  subcommands.
- `print_help` — prints usage, commands, and environment hints.
- `notes_dir`/`ensure_dir` (note) — resolve and create the storage path
  (`~/.quick_notes` or `QUICK_NOTES_DIR`).
- `create_note` / `write_note` / `note_path` / `unique_id` (note) — handle
  note creation, safe file naming, and header/body serialization.
- `parse_note` / `parse_timestamp` / `timestamp_string` (note) — read UTF-8
  text, extract headers, and handle US-local timestamps.
- `list_notes` — reads `.md` files, parses metadata, sorts, and renders rows
  via shared table helpers.
- `view_note` — prints raw or rendered markdown (respecting `--render`/`-r` and
  `--plain`/`-p`/`NO_COLOR`); uses `glow` when present.
- `edit_note` — opens the file in an editor, then re-parses and rewrites to
  refresh the `Updated` timestamp (with optional tag guard).
- `render_markdown` / `highlight_inline_code` (render) — color headings, lists,
  rules, inline/code fences while preserving line structure for tests.
- `render_table` / `display_len` / `truncate_with_ellipsis` (table) — ANSI-aware
  width and table rendering helpers used across list/tag output.

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
