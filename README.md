# Quick Notes CLI

A tiny Rust CLI to capture UTF-8 Markdown notes quickly from the terminal. Notes
are saved as individual `.md` files with creation and updated timestamps in a
human-friendly US-local format.

## Install

- Build locally: `cargo install --path .` (installs both `quick_notes` and `qn`
  binaries).
- Install from GitHub in one step: `cargo install --git \
  https://github.com/glouie/quick_notes.git --locked --bins`.
- Optional convenience alias after install: `alias qn=quick_notes` (not required
  if you use the `qn` binary).
- Default storage path: `~/.quick_notes` (override with
  `QUICK_NOTES_DIR=/path/to/notes`). The directory is created on first use.

## Usage

- `qn add <id> "note text"` | `qn add "note text"` — append to an existing note
  (id completion via fzf) or create a new note from body-only input with an
  auto-generated title.
- `qn new "Title" [body...]` — create with explicit title and optional body.
- `qn list [--sort created|updated|size] [--asc|--desc] [-s|--search text]
  [-t|--tag tag] [--all|-a]` — show ids with updated timestamp and a preview (default sort:
  updated desc).
- `qn list-deleted` / `qn list-archived` — list trashed or archived notes with
  the same flags as `list` (sorting, search, tags, relative time).
- `qn view <id>` — rendered view by default (headings, lists, rules). Add
  `--plain`/`-p` or set `NO_COLOR=1` to disable color.
- `qn edit <id> [-t tag]` — opens in `$EDITOR` (falls back to `vi`); if `fzf`
  is installed, it uses a popup with preview and multi-select (70% height by
  default; override with `QUICK_NOTES_FZF_HEIGHT` or go full-screen with
  `QUICK_NOTES_FZF_FULLSCREEN`), then opens all chosen notes together and
  refreshes the Updated timestamp. Optional tag guard.
- `qn delete <id> [more ids...] [-t tag]` — soft-delete to `trash`; use `--fzf`
  or call with no ids (and fzf installed) to pick multiple notes in an
  interactive preview list; optional tag guard for safety.
- `qn delete-all` — soft-delete every note to `trash`.
- `qn archive <id>...` — move notes to `archive` (kept indefinitely).
- `qn undelete <id>...` / `qn unarchive <id>...` — restore from `trash` or
  `archive` (renames on conflict).
- `qn migrate <path>` — import Markdown notes from another folder into a
  `migrated/<batch>` directory; keeps Created/Updated headers when present and
  renames on id conflicts.
- `qn seed <count> [--chars N] [-t tag] [--markdown]` — generate test notes
  (for load/perf checks) with random content of N characters (default 400) and
  optional tags; `--markdown` seeds rich Markdown samples. Argument order is
  flexible (e.g., `qn seed --markdown 3`).
- `qn tags` — list tags with counts plus first/last usage (pinned tags stay
  visible even if unused).
- `qn path` — print the notes directory.
- `qn completion zsh` — print the zsh completion script (fzf-powered note id
  selection with preview).
- `qn help` — usage overview.

Notes are written with a small header:

```text
Title: My note
Created: 20May24 12:00 -04:00
Updated: 20May24 12:00 -04:00
Tags: #todo, #meeting
---
markdown body...
```

## Versioning

Releases are tracked in `CHANGELOG.md`. Update the changelog with every
user-visible change before tagging a new version.

## Architecture

- Binaries: `src/main.rs` (`quick_notes`) and `src/bin/qn.rs` (`qn` symlink)
  delegate to the shared library entrypoint.
- Library modules:
  - `src/lib.rs` — CLI dispatch and top-level command wiring.
  - `src/note.rs` — note model, storage paths, ID/time helpers, read/write.
  - `src/render.rs` — markdown rendering (ANSI) and `glow` detection.
  - `src/shared/table.rs` — ANSI-aware width helpers and generic table
    rendering.
  - `src/shared/migrate.rs` — migration helpers and active-note resolution for
    imported batches.
- Shell completion: `contrib/quick_notes_fzf.zsh` (fzf-powered zsh completion).

## Tests

- Run `cargo fmt` and `cargo test` before committing changes.

## Tips

- Create a keyboard shortcut that runs `qn add <id> "$(pbpaste)"` in your
  shell/launcher for super-fast append (fzf helps pick ids).
- Sync `~/.quick_notes` with cloud storage by pointing `QUICK_NOTES_DIR` to a
  synced folder.
- Notes are UTF-8; keep your editor configured for UTF-8 to avoid encoding
  surprises.
- Example zsh key binding for instant capture from the clipboard:
  - Add to `~/.zshrc`:
    ```zsh
    function qnclip() { qn add <id> "$(pbpaste)"; }
    zle -N qnclip
    bindkey '^q' qnclip
    ```
  - Reload your shell, copy text, press `Ctrl+Q` to capture a note.
- FZF completion for note selection (zsh):
  - Add this one-liner to `~/.zshrc` (after `compinit` is run):
    `source <(qn completion zsh)`
  - On `qn view`/`qn render`/`qn edit`, press Tab to open an fzf list of note
    ids with a preview of each file; the selected id is inserted automatically.
  - On `qn delete`, press Tab to open fzf with multi-select and preview, then
    hit Enter to insert selected ids.
  - If completion fails with prompt/keymap errors, double-check that `compinit`
    runs before sourcing the script.
- Resize the fzf picker used by `edit` via `QUICK_NOTES_FZF_HEIGHT=90%`; set
  `QUICK_NOTES_FZF_FULLSCREEN=1` to use the full terminal height.
- Tags:
  - Append quickly with `qn add <id> "text"` or capture a new note with
    `qn add "text"` when you do not have an id handy; use completion/fzf for
    the id.
  - Filter `list`/`view`/`edit`/`delete` by tag using `-t/--tag`.
  - List all tags with `qn tags`; pinned tags default to
    `#todo,#meeting,#scratch` (override with
    `QUICK_NOTES_PINNED_TAGS=tag1,tag2`).
  - Unused tags disappear unless pinned.
