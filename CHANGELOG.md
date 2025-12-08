Changelog
=========

All notable changes are documented in this file. Version numbers follow semantic versioning.

## [0.1.0] - 2025-12-06
- Initial release of the Quick Notes CLI.
- Add `add`, `new`, `list`, `view`, `render`, `edit`, `path`, and `help` commands.
- Store notes as UTF-8 Markdown with creation/updated timestamps in US-local format.
- Markdown render mode for terminal reading with colored headings/lists when supported.
- Render mode honors `--plain` flag and `NO_COLOR` for non-colored output.
- Document quick keyboard binding example for clipboard capture.
- Note ids now default to a timestamp (`YYYYMMDDHHMMSS`) to keep them sortable and unique; suffixes are added only if necessary.
- Added zsh + fzf completion helper in `contrib/quick_notes_fzf.zsh` with preview support for note selection on `view`/`render`/`edit`.
- Introduced `qn completion zsh` so `.zshrc` can source completions via `source <(qn completion zsh)`.
- Added `delete` command with optional fzf multi-select and preview for batch deletions; list output now shows compact previews.
- Added `seed` command to generate bulk test notes; improved id uniqueness using microsecond timestamps.
- `list` supports `--sort` (created|updated|size) with `--asc|--desc`.
- `edit` now uses an fzf popup (70% height) with preview when fzf is available, otherwise falls back to the default editor.
- Added `delete-all` to remove every note in the current notes directory.
- Tags: notes now carry `Tags:` metadata; add via `-t/--tag`, filter list/view/edit/delete by tag, and list tag stats via `qn tags` (pinned tags default to `#todo,#meeting,#scratch`, overridable with `QUICK_NOTES_PINNED_TAGS`).
- Search: `list -s/--search` filters by substring in title/body (case-insensitive).
- `seed` supports the same tagging flags as `add` (`-t/--tag`).
