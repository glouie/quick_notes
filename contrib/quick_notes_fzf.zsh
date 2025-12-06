#compdef qn quick_notes

# FZF-powered completion for qn view/render/edit commands.
# Requirements: fzf installed; notes live in $QUICK_NOTES_DIR or ~/.quick_notes.

_qn() {
  local state
  _arguments \
    '1:command:(add new list view render edit path help)' \
    '2:note id:_qn_note_ids' \
    '*::text:->text'
}

_qn_note_ids() {
  # Only trigger fzf for commands that take a note id.
  if (( CURRENT != 2 )); then
    return 1
  fi
  if [[ ${words[2]} != view && ${words[2]} != render && ${words[2]} != edit ]]; then
    return 1
  fi

  local dir=${QUICK_NOTES_DIR:-$HOME/.quick_notes}
  local -a files
  files=("${(@f)$(find "$dir" -maxdepth 1 -name '*.md' -print 2>/dev/null)}")
  if (( ${#files} == 0 )); then
    return 1
  fi

  local selection
  selection=$(printf '%s\n' "${files[@]}" | FZF_DEFAULT_OPTS="${FZF_DEFAULT_OPTS-} --preview 'sed -n \"1,80p\" {}' --preview-window=down:70%" fzf)
  if [[ -n $selection ]]; then
    compadd -- ${selection:t:r}
  fi
}

_qn "$@"
