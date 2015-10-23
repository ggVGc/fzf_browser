
_fuzzybrowse_zsh_insert_output() {
  local startDir="$1"
  fuzzybrowse "$startDir" "" /tmp/zsh_fzf_brow_out
  local res="$(cat /tmp/zsh_fzf_brow_out | tr "\\n" " ")"
  zle reset-prompt
  if [[ -n "$res" ]]; then
    res="$res[0,-2]"
    LBUFFER+=" $res"
    if [[ -d "$res" ]]; then
      LBUFFER+="/"
    else
      LBUFFER+=" "
    fi
  fi
}

zle -N _fuzzybrowse_zsh_insert_output
