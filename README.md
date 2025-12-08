Quick Notes CLI
================

A tiny Rust CLI to capture UTF-8 Markdown notes quickly from the terminal. Notes are saved as individual `.md` files with creation and updated timestamps in a human-friendly US-local format.

Install
-------

- Build locally: `cargo install --path .`
- Optional convenience alias after install: `alias qn=quick_notes`
- Default storage path: `~/.quick_notes` (override with `QUICK_NOTES_DIR=/path/to/notes`). The directory is created on first use.

Usage
-----

- `qn add "note text"` — fast path; title is generated automatically.
- `qn new "Title" [body...]` — create with explicit title and optional body.
- `qn list [--sort created|updated|size] [--asc|--desc]` — show ids with updated timestamp and a preview (default sort: updated desc).
- `qn view <id>` — print the note plus header.
- `qn view <id> --render` or `qn render <id>` — render Markdown in the terminal (headings, lists, rules) for quick reading. Add `--plain` or set `NO_COLOR=1` to disable color.
- `qn edit <id>` — opens in `$EDITOR` (falls back to `vi`); if `fzf` is installed, it uses a 70% height popup with a preview before editing, then refreshes the Updated timestamp.
- `qn delete <id> [more ids...]` — delete one or more notes; use `--fzf` or call with no ids (and fzf installed) to pick multiple notes in an interactive preview list.
- `qn delete-all` — delete every note in the notes directory.
- `qn seed <count> [--chars N]` — generate test notes (for load/perf checks) with random content of N characters (default 400).
- `qn path` — print the notes directory.
- `qn completion zsh` — print the zsh completion script (fzf-powered note id selection with preview).
- `qn help` — usage overview.

Notes are written with a small header:

```
Title: My note
Created: 05/20/2024 12:00 PM -04:00
Updated: 05/20/2024 12:00 PM -04:00
---
markdown body...
```

Versioning
----------

Releases are tracked in `CHANGELOG.md`. Update the changelog with every user-visible change before tagging a new version.

Tips
----

- Create a keyboard shortcut that runs `qn add "$(pbpaste)"` or `qn add "text"` in your shell/launcher for super-fast capture.
- Sync `~/.quick_notes` with cloud storage by pointing `QUICK_NOTES_DIR` to a synced folder.
- Notes are UTF-8; keep your editor configured for UTF-8 to avoid encoding surprises.
- Example zsh key binding for instant capture from the clipboard:
  - Add to `~/.zshrc`:
    ```
    function qnclip() { qn add "$(pbpaste)"; }
    zle -N qnclip
    bindkey '^q' qnclip
    ```
  - Reload your shell, copy text, press `Ctrl+Q` to capture a note.
- FZF completion for note selection (zsh):
  - Add this one-liner to `~/.zshrc` (after `compinit`): `source <(qn completion zsh)`
  - On `qn view`/`qn render`/`qn edit`, press Tab to open an fzf list of note ids with a preview of each file; the selected id is inserted automatically.
  - On `qn delete`, press Tab to open fzf with multi-select and preview, then hit Enter to insert selected ids.
