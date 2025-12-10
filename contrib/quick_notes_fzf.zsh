#compdef qn quick_notes

# FZF-powered completion for qn view/render/edit commands.
# Requirements: fzf installed; notes live in $QUICK_NOTES_DIR or ~/.quick_notes.

_qn() {
  local state
  _arguments -C \
    '1:command:(add new list view render edit delete delete-all seed tags path help completion)' \
    '*:note id:_qn_note_ids'
}

_qn_note_ids() {
  local cmd=${words[2]}
  if [[ $cmd != view && $cmd != render && $cmd != edit && $cmd != delete ]]; then
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

  local cur_prefix=$PREFIX
  local -a stems
  stems=(${files:t:r})

  if [[ -n $cur_prefix ]]; then
    local -a matches
    matches=(${(M)stems:#${~cur_prefix}*})
    if (( ${#matches} == 1 )); then
      compadd -- $matches
      return 0
    fi
  fi

  # If fzf is missing, fall back to plain completion.
  if ! command -v fzf >/dev/null 2>&1; then
    if [[ -n $cur_prefix ]]; then
      local -a matches
      matches=(${(M)stems:#${~cur_prefix}*})
      compadd -- $matches
    else
      compadd -- $stems
    fi
    return 0
  fi

  local renderer="quick_notes"
  if ! command -v quick_notes >/dev/null 2>&1 && command -v qn >/dev/null 2>&1; then
    renderer="qn"
  fi

  local fzf_opts="--preview 'env -u NO_COLOR CLICOLOR_FORCE=1 ${renderer} render \$(basename {}) 2>/dev/null || sed -n \"1,120p\" {}' --preview-window=down:70% --ansi"
  if [[ ${words[2]} == delete ]]; then
    fzf_opts="$fzf_opts --multi"
  fi

  local selection
  selection=$(printf '%s\n' "${files[@]}" | FZF_DEFAULT_OPTS="${FZF_DEFAULT_OPTS-} ${fzf_opts}" fzf --query "${cur_prefix}" 2>/dev/null || true)
  if [[ -z $selection ]]; then
    if [[ -n $cur_prefix ]]; then
      local -a matches
      matches=(${(M)stems:#${~cur_prefix}*})
      if (( ${#matches} == 1 )); then
        compadd -- $matches
        return 0
      fi
    }
    return 1
  fi
  compadd -- ${(f)${selection:t:r}}
}

compdef _qn qn quick_notes
