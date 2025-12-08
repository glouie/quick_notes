AGENTS
======

Purpose
-------
Quick Notes is a fast, UTF-8 Rust CLI for capturing Markdown notes with creation and updated timestamps. This guide summarizes how the CLI behaves so an agent can quickly assist users.

Key Commands
------------
- Binary name is `quick_notes`; users often add `alias qn=quick_notes`. Completions support both `qn` and `quick_notes`.
- `qn add "text" [-t tag...]` — quick add with generated title and timestamp-based id (tags normalized to `#tag` form).
- `qn new <title> [body...] [-t tag...]` — add with explicit title and optional tags.
- `qn list [--sort created|updated|size] [--asc|--desc] [-s text] [-t tag]` — list notes with compact previews; search across title/body; filter by tag.
- `qn view <id> [--render|-r] [--plain] [-t tag]` — show raw or rendered Markdown; optional tag guard.
- `qn render <id>` — shortcut to rendered view.
- `qn edit <id> [-t tag]` — edit in `$EDITOR`; if `fzf` exists, uses a ~70% popup with preview; optional tag guard.
- `qn delete <ids...> [--fzf] [-t tag]` — delete notes; interactive multi-select if `fzf` is present and no ids provided; optional tag guard.
- `qn delete-all` — remove every note.
- `qn tags` — list tags with counts and first/last usage; pinned tags stay visible even if unused.
- `qn seed <count> [--chars N]` — generate bulk test notes (microsecond ids, random bodies).
- `qn completion zsh` — emits zsh/fzf completion script (includes delete multi-select).
- `qn path` — show the notes directory (`~/.quick_notes` by default).

Storage & Format
----------------
- Notes live in `~/.quick_notes` unless `QUICK_NOTES_DIR` is set.
- Each note is a Markdown file with a header:
  ```
  Title: ...
  Created: 12/07/2025 11:53 AM -08:00
  Updated: 12/07/2025 11:53 AM -08:00
  ---
  body...
  ```

Important Behaviors
-------------------
- IDs: microsecond-based timestamps, suffixed if a collision occurs.
- Sorting: `list` defaults to updated desc; can sort by created/updated/size asc/desc.
- Render: uses pulldown-cmark; `--plain` or `NO_COLOR` disables color.
- Completion: zsh script provides fzf previews for view/render/edit/delete; delete allows multi-select via Tab in fzf.
- Popup edit: fzf-based preview/70% height; falls back to `$EDITOR` normally.
- Tags: normalized to `#tag`; stored in `Tags:` header. Tag filters work on list/view/edit/delete. Pinned tags (default `#todo,#meeting,#scratch`, override via `QUICK_NOTES_PINNED_TAGS`) remain visible in `qn tags` even if unused.
- Search: `list -s/--search` matches substring in title/body (case-insensitive).

Common Fix/Assist Tips
----------------------
- Completion not working: ensure `compinit` ran; source via `source <(qn completion zsh)`; confirm `fzf` is installed for previews.
- Notes not found: check `QUICK_NOTES_DIR`; run `qn path`.
- Encoding issues: ensure UTF-8 editor; files are plain UTF-8 Markdown.
- Large datasets: use `qn seed <N>` to create load; `qn list --sort size --desc` for inspection.
