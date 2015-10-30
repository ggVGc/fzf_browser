
_fuzzybrowse_zsh_insert_output() {
  local startDir="$1"
  local res
  () {
    fuzzybrowse -o "$1" "$startDir"
    res="$(<$1 tr "\\n" " ")"
  } =(:)
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
