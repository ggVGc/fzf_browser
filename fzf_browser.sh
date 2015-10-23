#!/usr/bin/env bash

# CONFIGURATION

__fuzzybrowse_openTerminal(){
  urxvt "$@"
}

__fuzzybrowse_previewFile(){
  __fuzzybrowse_openTerminal -e less "$@"
}

# List of extensions to ignore, separated by |
__fuzzybrow_file_ignore="log|bak|aux|lof|lol|lot|toc|bbl|blg|tmp|temp|swp|incomplete|o|class|cache|pyc|aria2|torrent|torrent.added|part|crdownload"

# END CONFIGURATION

# Opens fuzzy dir browser. Tab to switch between file mode and directory mode. Esc to quit.
fuzzybrowse() {
  local res key sel prev_dir query stored_query tmp_prompt
  local initial_dir
  initial_dir="$(pwd)"
  local start_dir="$1"
  local start_query="$2"
  local out_file="$3"
  local custom_prompt="$4"
  local tmp_dir

  stored_query="$start_query"

  if [[ -n "$out_file" ]]; then
    echo -n "" > "$out_file"
  fi

  if [[ -n "$start_dir" ]]; then
    cd "$start_dir" 
  else
    start_dir="$initial_dir"
  fi
  local early_exit
  if [[ -n "$start_query" ]]; then
    early_exit="-1"
  else
    early_exit="--ansi" # just dummy
  fi
  while true ; do
    if [[ -n "$custom_prompt" ]]; then
      tmp_prompt="--prompt=$custom_prompt""$(pwd)/"
    else
      tmp_prompt="--ansi"
    fi
    if [[ "$__fuzzybrowse_recursive" == 1 ]]; then
      res="$(__fuzzybrowse_file_source "" 2>/dev/null | sort | __fuzzybrowse_fzf_cmd "$tmp_prompt" "$early_exit" "-q" "$stored_query" )"
    else
      res="$(__fuzzybrowse_combined_source 2>/dev/null | __fuzzybrowse_fzf_cmd "$tmp_prompt" "$early_exit" "-q" "$stored_query")" 
    fi
    stored_query=""
    if [[ -z "$res" ]]; then
      dirs -c
      cd "$initial_dir"
      return
    fi
    query=$(echo "$res" | head -1)
    sel=$(echo "$res"|tail -n +3 | cut -f1 -d$'\t')
    key=$(echo "$res" | head -2 | tail -1)
    if [[ -n "$start_query" && -z "$key" ]]; then
      break
    fi
    start_query=""
    case "$key" in
      \#|\`)
        pushd ".." > /dev/null 2>&1
      ;;
      /)
        if [[ -d "$sel" ]]; then
          pushd "$sel" > /dev/null 2>&1
        fi
      ;;
      "ctrl-o")
        prev_dir="$(pwd)"
        popd > /dev/null 2>&1
      ;;
      "ctrl-u")
        if [[ -n "$prev_dir" ]]; then
          pushd "$prev_dir" > /dev/null 2>&1
          prev_dir=""
        fi
      ;;
      #";")
        #break
      #;;
      #":")
        #break
      #;;
      return)
        if [[ "$sel" == "." ]]; then
          break
        fi
        if [[ -d "$sel" ]]; then
          pushd "$sel" > /dev/null 2>&1
        else
          break
        fi
      ;;
      ctrl-c)
        dirs -c
        cd "$initial_dir"
        return
      ;;
      ctrl-q)
        stored_query="$query"
        __fuzzybrowse_show_hidden=$((__fuzzybrowse_show_hidden==0))
      ;;
      ctrl-h)
        pushd "$HOME" > /dev/null 2>&1
      ;;
      ctrl-z)
        tmp_dir="$(fasd -ld 2>&1 | sed -n 's/^[ 0-9.,]*//p' | fzf --tac +s)"
        if [[ -n "$tmp_dir" ]]; then
          pushd "$tmp_dir" > /dev/null 2>&1
        fi
      ;;
      ctrl-s|ctrl-x)
        export e
        e="$(full_path "$sel")"
        clear
        echo "\$e = $e"
        $SHELL
      ;;
    ctrl-l)
      stored_query="$query"
      __fuzzybrowse_previewFile "$sel"
      ;;
    ctrl-e)
      stored_query="$query"
      "$EDITOR" "$sel"
      ;;
    ctrl-f)
      stored_query="$query"
      __fuzzybrowse_recursive=$((__fuzzybrowse_recursive==0))
    ;;
    esac
  done
  dirs -c
  local x rel_path
  echo "$sel" | while read x; do
    rel_path="$(printf "%q\n" "$(realpath --relative-base="$initial_dir" "$x")")"
    fasd -A "$rel_path" > /dev/null 2>&1
    if [[ -n "$out_file" ]]; then
      echo "$rel_path" >> "$out_file"
    else
      echo "$rel_path"
    fi
  done
  cd "$initial_dir"
}





__fuzzybrowse_show_hidden=0
__fuzzybrowse_recursive=0

__fuzzybrow_file_ignore_pat="$(printf ".*\(%q\)$"  "$__fuzzybrow_file_ignore")"

__fuzzybrow_populate_dir_list(){
  local line
  
  while read line ; do
    if [[ -d "$line" ]]; then
      echo -e "\e[36m$line\t\e[0m$(cd "$line" && find . -maxdepth 1 -type f |head -9 | grep -v -i "$__fuzzybrow_file_ignore_pat" |cut -c3- | tr "\\n" "|" | sed 's/|/\\\e[36m | \\\e[0m/g')"
    fi
  done
}

full_path(){
  printf "%q" "$(cd "$(dirname "$1")"; pwd)/$(basename "$1")"
}

__fuzzybrowse_file_source(){
  if [[ "$__fuzzybrowse_show_hidden" == 1 ]]; then
    find  . "$@" -type f -o -xtype f ! -iregex "$__fuzzybrow_file_ignore_pat" | cut -c3-
  else
    find . "$@" \( -type f -o -xtype f \) -not -path '*/\.*' ! -iregex "$__fuzzybrow_file_ignore_pat" | cut -c3-
  fi
}

__fuzzybrowse_combined_source(){
  cat <(__fuzzybrowse_dir_source|sort) <(__fuzzybrowse_file_source -maxdepth 1 |sort) 
}

__fuzzybrowse_dir_source(){
  if [[ "$__fuzzybrowse_show_hidden" == 1 ]]; then
    cat <(echo ".") <(find . -maxdepth 1 -type d -o -type l | tail -n +2 | cut -c3- | __fuzzybrow_populate_dir_list)
  else
    cat <(echo ".") <(find . -maxdepth 1 \( -type d -o -type l \) -not -path '*/\.*' | tail -n +2 | cut -c3- | __fuzzybrow_populate_dir_list)
  fi
}

__fuzzybrowse_fzf_cmd(){
  local prePrompt=""
  if [[ "$__fuzzybrowse_recursive" == 1 ]]; then
    prePrompt="{REC}"
  fi
  fzf --reverse --multi --prompt="$prePrompt""$(pwd): " --ansi --extended --print-query "$@"  --expect=ctrl-c,ctrl-x,ctrl-s,\#,return,ctrl-o,ctrl-u,\`,ctrl-q,ctrl-h,ctrl-z,ctrl-f,ctrl-e,ctrl-l,/ 

  #`# Hack to fix syntax highlight in vim..
}

