TROUBLESHOOTING
===============

FZF Popup Missing For `qn` View/Edit/Delete
-------------------------------------------
- Ensure fzf is available on `PATH`: `command -v fzf` should succeed.
- Make sure `QUICK_NOTES_NO_FZF` is **not** set; if it is, `unset QUICK_NOTES_NO_FZF`
  and remove it from your shell init.
- Load completion after `compinit` in your shell rc:
  ```zsh
  autoload -Uz compinit && compinit
  command -v fzf >/dev/null && source <(qn completion zsh)
  ```
- Confirm you’re running the intended binary: `which qn`/`which quick_notes`
  should point to your installed CLI.
