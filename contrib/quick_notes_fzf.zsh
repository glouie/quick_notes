#compdef qn quick_notes

# FZF-powered completion for qn view/render/edit commands.
# Requirements: fzf installed; notes live in $QUICK_NOTES_DIR or ~/.quick_notes.

_qn() {
  local state
  _arguments -C \
    '1:command:(add new list view render edit delete seed path help completion)' \
    '2:note id:_qn_note_ids' \
    '*::text:->text'
}

_qn_note_ids() {
  if [[ ${words[2]} != view && ${words[2]} != render && ${words[2]} != edit && ${words[2]} != delete ]]; then
    return 1
  fi

  local dir=${QUICK_NOTES_DIR:-$HOME/.quick_notes}
  if [[ ! -d $dir ]]; then
    return 1
  fi
  local -a files
  files=("${(@f)$(find "$dir" -maxdepth 1 -name '*.md' -print 2>/dev/null)}")
  if (( ${#files} == 0 )); then
    return 1
  fi

  # If fzf is missing, fall back to plain completion.
  if ! command -v fzf >/dev/null 2>&1; then
    compadd -- ${files:t:r}
    return 0
  fi

  local fzf_opts="--preview 'sed -n \"1,80p\" {}' --preview-window=down:70%"
  if [[ ${words[2]} == delete ]]; then
    fzf_opts="$fzf_opts --multi"
  fi

  local selection
  selection=$(printf '%s\n' "${files[@]}" | FZF_DEFAULT_OPTS="${FZF_DEFAULT_OPTS-} ${fzf_opts}" fzf 2>/dev/null || true)
  if [[ -z $selection ]]; then
    return 1
  fi
  compadd -- ${(f)${selection:t:r}}
}

compdef _qn qn quick_notes
