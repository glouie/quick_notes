# AGENTS

## Purpose

Quick Notes is a fast, UTF-8 Rust CLI for capturing Markdown notes with creation
and updated timestamps. This guide summarizes how the CLI behaves so an agent
can quickly assist users.

## Key Commands

- Binary name is `quick_notes`; users often add `alias qn=quick_notes`.
  Completions support both `qn` and `quick_notes`.
- `qn add <id> "text"` — append text to an existing note (fzf completion for
  ids).
- `qn new <title> [body...] [-t tag...]` — add with explicit title and optional
  tags.
- `qn list [--sort created|updated|size] [--asc|--desc] [-s text] [-t tag] [-a|--all]`
  — list notes with compact previews; search across title/body; filter by tag; `-a` disables pagination (pager also accepts `q` to quit).
- `qn list-deleted` / `qn list-archived` — list trashed or archived notes with
  the same flags as `list` (including `-a/--all`).
- `qn view <id> [--plain] [-t tag]` — show rendered Markdown by default;
  optional tag guard; `--plain` disables color.
- `qn render <id>` — shortcut to rendered view.
- `qn edit <id> [-t tag]` — edit in `$EDITOR`; if `fzf` exists, uses a ~70%
  popup with preview and multi-select; optional tag guard.
- `qn delete <ids...> [--fzf] [-t tag]` — soft delete to `trash`; interactive
  multi-select with preview if `fzf` is present and no ids provided; optional tag guard.
- `qn delete-all` — move every note to `trash`.
- `qn archive <ids...> [--fzf]` — move notes to `archive` (kept indefinitely); interactive preview picker when `fzf` is available and no ids are given.
- `qn undelete <ids...>` / `qn unarchive <ids...>` — restore from `trash` or
  `archive` (renames on conflict).
- `qn migrate <path>` — import Markdown notes from another directory into a new
  `migrated/<batch>` folder; keeps Created/Updated headers when present and
  resolves id collisions.
- `qn tags` — list tags with counts and first/last usage; pinned tags stay
  visible even if unused.
- `qn seed <count> [--chars N] [--markdown] [-t tag]` — generate bulk test
  notes (microsecond ids, random bodies or Markdown samples) with optional tags.
- `qn completion zsh` — emits zsh/fzf completion script (includes delete
  multi-select).
- `qn path` — show the notes directory (`~/.quick_notes` by default).

## Storage & Format

- Notes live in `~/.quick_notes` unless `QUICK_NOTES_DIR` is set.
- Each note is a Markdown file with a header:
  ```text
  Title: ...
  Created: 07Dec25 11:53 -08:00
  Updated: 07Dec25 11:53 -08:00
  Tags: #todo, #meeting
  ---
  body...
  ```

## Implementation Map

- Binaries: `src/main.rs` (`quick_notes`) and `src/bin/qn.rs` (`qn`) call the
  shared library entrypoint.
- Modules:
  - `src/lib.rs` — CLI wiring/dispatch for commands.
  - `src/note.rs` — note model, paths, ids, timestamps, read/write.
  - `src/render.rs` — markdown rendering; prefers `glow` for `view -r` when
    installed; otherwise uses internal ANSI styling.
  - `src/shared/table.rs` — ANSI-aware width helpers and generic table
    rendering used in listings.
  - `src/shared/migrate.rs` — migration helpers for imported note batches and
    active note resolution.
- Completion script: `contrib/quick_notes_fzf.zsh` (fzf-powered zsh).
- Linting/tests: run `cargo fmt` and `cargo test` before committing; `NO_COLOR`
  and `QUICK_NOTES_NO_FZF` are set in tests to keep output deterministic.

## Important Behaviors

- IDs: microsecond-based timestamps, suffixed if a collision occurs. `list`
  shows the id (dark gray) and timestamp (blue) followed by the preview; note
  content stays uncolored; tags are colored per tag hash.
- Sorting: `list` defaults to updated desc; can sort by created/updated/size
  asc/desc.
- Render: uses pulldown-cmark; `--plain` or `NO_COLOR` disables color.
- Completion: zsh script provides fzf previews for view/render/edit/delete;
  delete allows multi-select via Tab in fzf.
- Popup edit: fzf-based preview/70% height; falls back to `$EDITOR` normally.
- Tags: normalized to `#tag`; stored in `Tags:` header. Tag filters work on
  list/view/edit/delete. Pinned tags (default `#todo,#meeting,#scratch`,
  override via `QUICK_NOTES_PINNED_TAGS`) remain visible in `qn tags` even if
  unused.
- Search: `list -s/--search` matches substring in title/body
  (case-insensitive).
- Migration: imported notes are stored under `~/.quick_notes/migrated/<batch>`
  and surfaced in list/view/edit/delete alongside active notes; ids are
  preserved unless they conflict, in which case a new id is generated.

## Common Fix/Assist Tips

- Completion not working: ensure `compinit` ran; source via
  `source <(qn completion zsh)`; confirm `fzf` is installed for previews.
- Notes not found: check `QUICK_NOTES_DIR`; run `qn path`.
- Encoding issues: ensure UTF-8 editor; files are plain UTF-8 Markdown.
- Large datasets: use `qn seed <N>` to create load; `qn list --sort size --desc`
  for inspection.
- Always run `cargo fmt` and `cargo test` before committing changes.
