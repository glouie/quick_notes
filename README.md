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
- `qn list` — show ids, titles, and timestamps (sorted by most recently updated).
- `qn view <id>` — print the note plus header.
- `qn view <id> --render` or `qn render <id>` — render Markdown in the terminal (headings, lists, rules) for quick reading. Add `--plain` or set `NO_COLOR=1` to disable color.
- `qn edit <id>` — open in `$EDITOR` (falls back to `vi`), then refreshes the Updated timestamp.
- `qn path` — print the notes directory.
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
  - Source `contrib/quick_notes_fzf.zsh` in your shell (`source /path/to/quick_notes/contrib/quick_notes_fzf.zsh`).
  - On `qn view`/`qn render`/`qn edit`, press Tab to open an fzf list of note ids with a preview of each file; the selected id is inserted automatically.
