#compdef qn quick_notes quick_notes

# FZF-powered completion for quick_notes/qn.
# Requirements: fzf installed; notes live in $QUICK_NOTES_DIR or ~/.quick_notes.
# Safe to source multiple times; expects compinit to have been run.

if ! typeset -f _arguments >/dev/null 2>&1; then
  if [[ -o interactive ]]; then
    print -u2 \
      "quick_notes completion: run 'compinit' before sourcing this script."
  fi
  return 0 2>/dev/null || exit 0
fi

_qn() {
  local cmd=${words[2]}

  # First arg: subcommand selection.
  if (( CURRENT == 1 || CURRENT == 2 )); then
    compadd add new list view render edit delete delete-all seed tags path help guide completion list-deleted list-archived archive undelete unarchive stats
    return
  fi

  case $cmd in
    list|list-deleted|list-archived) _qn_list_opts ;;
    view|render) _qn_view_opts ;;
    edit) _qn_edit_opts ;;
    delete|archive|add|undelete|unarchive) _qn_delete_opts ;;
    seed) _qn_seed_opts ;;
    add|new) _qn_add_new_opts ;;
    tags) _qn_tags_opts ;;
    help) _qn_help_topics ;;
    guide) _qn_guide_topics ;;
    path|completion|delete-all) return 0 ;;
    *) return 0 ;;
  esac
}

_qn_list_opts() {
  _arguments -C \
    '--sort[sort field]:field:(created updated size)' \
    '--asc[ascending]' \
    '--desc[descending]' \
    '(-s --search)'{-s,--search}'[search text]:search:' \
    '(-r --relative)'{-r,--relative}'[show relative times]' \
    '(-t --tag)'{-t,--tag}'[tag filter]:tag:'
}

_qn_view_opts() {
  _arguments -C \
    '(-p --plain)'{-p,--plain}'[plain output]' \
    '(-r --render)'{-r,--render}'[render markdown]' \
    '(-t --tag)'{-t,--tag}'[tag filter]:tag:' \
    '*:note id:_qn_note_ids'
}

_qn_edit_opts() {
  _arguments -C \
    '(-t --tag)'{-t,--tag}'[tag filter]:tag:' \
    '*:note id:_qn_note_ids'
}

_qn_delete_opts() {
  _arguments -C \
    '--fzf[force fzf selection]' \
    '(-t --tag)'{-t,--tag}'[tag filter]:tag:' \
    '*:note id:_qn_note_ids'
}

_qn_seed_opts() {
  _arguments -C \
    '--markdown[seed markdown samples]' \
    '--chars[body length]:chars:(200 400 800 1200)' \
    '(-t --tag)'{-t,--tag}'[tag to apply]:tag:' \
    '1:count' \
    '2:body (optional when not using --markdown)'
}

_qn_add_new_opts() {
  _arguments -C \
    '(-t --tag)'{-t,--tag}'[tag to apply]:tag:' \
    '1:title or body'
}

_qn_tags_opts() {
  _arguments -C \
    '(-s --search)'{-s,--search}'[search tags]:search:' \
    '(-r --relative)'{-r,--relative}'[show relative times]'
}

_qn_help_topics() {
  _arguments '*:topic:->topic'
  case $state in
    topic)
      local -a topics
      topics=(
        add new list list-deleted list-archived view render edit delete delete-all archive undelete unarchive migrate-ids tags seed stats path completion help
        getting-started searching bulk-ops
        QUICK_NOTES_DIR QUICK_NOTES_TRASH_RETENTION_DAYS QUICK_NOTES_PINNED_TAGS QUICK_NOTES_NO_FZF NO_COLOR
      )
      compadd -- $topics
    ;;
  esac
}

_qn_guide_topics() {
  _arguments '*:guide:->guide'
  case $state in
    guide)
      compadd -- getting-started searching bulk-ops
    ;;
  esac
}

_qn_note_ids() {
  local cmd=${words[2]}
  if [[ $cmd != view && $cmd != render && $cmd != edit \
    && $cmd != delete && $cmd != list ]]; then
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
  if ! command -v quick_notes >/dev/null 2>&1 \
    && command -v qn >/dev/null 2>&1; then
    renderer="qn"
  fi

  local preview_cmd
  if command -v bat >/dev/null 2>&1; then
    preview_cmd="env -u NO_COLOR CLICOLOR_FORCE=1 bat --color=always \
--style=plain --language=markdown {}"
  elif command -v batcat >/dev/null 2>&1; then
    preview_cmd="env -u NO_COLOR CLICOLOR_FORCE=1 batcat --color=always \
--style=plain --language=markdown {}"
  else
    preview_cmd="env -u NO_COLOR CLICOLOR_FORCE=1 ${renderer} render \
\$(basename {}) 2>/dev/null || sed -n '1,120p' {}"
  fi

  local fzf_opts="--preview '${preview_cmd}' --preview-window=down:70% --ansi"
  if [[ ${words[2]} == delete ]]; then
    fzf_opts="$fzf_opts --multi"
  fi

  local selection
  selection=$(
    printf '%s\n' "${stems[@]}" |
      FZF_DEFAULT_OPTS="${FZF_DEFAULT_OPTS-} ${fzf_opts}" \
      fzf --query "${cur_prefix}" 2>/dev/null || true
  )
  if [[ -z $selection ]]; then
    if [[ -n $cur_prefix ]]; then
      local -a matches
      matches=(${(M)stems:#${~cur_prefix}*})
      if (( ${#matches} == 1 )); then
        compadd -- $matches
        return 0
      fi
    fi
    return 1
  fi
  compadd -- ${(f)selection}
}

compdef _qn qn quick_notes
