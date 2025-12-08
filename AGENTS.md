AGENTS
======

Purpose
-------
Quick Notes is a fast, UTF-8 Rust CLI for capturing Markdown notes with creation and updated timestamps. This guide summarizes how the CLI behaves so an agent can quickly assist users.

Key Commands
------------
- Binary name is `quick_notes`; users often add `alias qn=quick_notes`. Completions support both `qn` and `quick_notes`.
- `qn add "text"` — quick add with generated title and timestamp-based id.
- `qn new <title> [body...]` — add with explicit title.
- `qn list [--sort created|updated|size] [--asc|--desc]` — list notes with compact previews.
- `qn view <id> [--render|-r] [--plain]` — show raw or rendered Markdown.
- `qn render <id>` — shortcut to rendered view.
- `qn edit <id>` — edit in `$EDITOR`; if `fzf` exists, uses a ~70% popup with preview.
- `qn delete <ids...> [--fzf]` — delete notes; interactive multi-select if `fzf` is present and no ids provided.
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

Common Fix/Assist Tips
----------------------
- Completion not working: ensure `compinit` ran; source via `source <(qn completion zsh)`; confirm `fzf` is installed for previews.
- Notes not found: check `QUICK_NOTES_DIR`; run `qn path`.
- Encoding issues: ensure UTF-8 editor; files are plain UTF-8 Markdown.
- Large datasets: use `qn seed <N>` to create load; `qn list --sort size --desc` for inspection.
