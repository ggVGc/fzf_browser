#!/usr/bin/env bash

##### CONFIGURATION #####

__fuzzybrowse_runInTerminal(){
  urxvt -e "$@"
}

__fuzzybrowse_previewFile(){
  __fuzzybrowse_runInTerminal less "$@"
}

__fuzzybrowse_runFile(){
  xdg-open "$@"
}

# List of extensions to ignore, separated by |
__fuzzybrow_file_ignore="log|bak|aux|lof|lol|lot|toc|bbl|blg|tmp|temp|swp|incomplete|o|class|cache|pyc|aria2|torrent|torrent.added|part|crdownload"

##### END CONFIGURATION ##### 
# Opens fuzzy dir browser. Tab to switch between file mode and directory mode. Esc to quit.
fuzzybrowse() {
  local res key sel prev_dir query stored_query tmp_prompt tmp_file
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
  start_query=""
  while true ; do
    if [[ -n "$custom_prompt" ]]; then
      tmp_prompt="--prompt=$custom_prompt""$(pwd)/"
    else
      tmp_prompt="--ansi"
    fi
    #if [[ "$__fuzzybrowse_recursive" == 1 ]]; then
      #res="$(__fuzzybrowse_file_source "" 2>/dev/null | __fuzzybrowse_fzf_cmd "$tmp_prompt" "$early_exit" "-q" "$stored_query" )"
    #else
      res="$(__fuzzybrowse_combined_source 2>/dev/null | __fuzzybrowse_fzf_cmd "$tmp_prompt" "$early_exit" "-q" "$stored_query")" 
    #fi
    stored_query=""
    if [[ -z "$res" ]]; then
      dirs -c
      cd "$initial_dir"
      return
    fi
    query=$(echo "$res" | head -1)
    sel=$(echo "$res"|tail -n +3)
    key=$(echo "$res" | head -2 | tail -1)
    if [[ "$early_exit" == "-1" && -z "$key" ]]; then
      break
    fi
    early_exit="--ansi"

    case "$query" in
      ".")
        sel="$(pwd)"
        break
      ;;
      "..")
        pushd ".." > /dev/null 2>&1
      ;;
    esac
    case "$key" in
      left|\#|\`)
        pushd ".." > /dev/null 2>&1
      ;;
      /)
        break;
        #if [[ -d "$sel" ]]; then
          #pushd "$sel" > /dev/null 2>&1
        #fi
      ;;
      ctrl-o)
        prev_dir="$(pwd)"
        popd > /dev/null 2>&1
      ;;
      ctrl-u)
        if [[ -n "$prev_dir" ]]; then
          pushd "$prev_dir" > /dev/null 2>&1
          prev_dir=""
        fi
      ;;
      return)
        tmp_file=$(__fuzzybrowse_get_entry "$sel")
        if [[ -f "$tmp_file" ]]; then
          sel="$tmp_file"
          break
        fi
        if [[ -d "$tmp_file" ]]; then
            pushd "$tmp_file" > /dev/null 2>&1
        else
          break
        fi
      ;;
      right)
        stored_query="$query"
        tmp_dir=$(__fuzzybrowse_get_dir "$sel")
        if [[ -d "$tmp_dir" ]]; then
          pushd "$tmp_dir" > /dev/null 2>&1
        else
          __fuzzybrowse_runFile "$sel"
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
      ctrl-x)
        export e
        e="$(__fuzzybrowse_full_path "$(__fuzzybrowse_get_entry "$sel")")"
        clear
        echo "\$e = $e"
        $SHELL
      ;;
    ctrl-v)
      stored_query="$query"
      __fuzzybrowse_runFile "$(__fuzzybrowse_get_entry "$sel")" > /dev/null 2>&1
    ;;
    ctrl-l)
      stored_query="$query"
      __fuzzybrowse_previewFile "$sel"
      ;;
    ctrl-e)
      stored_query="$query"
      "$EDITOR" "$sel"
      ;;
    ctrl-r)
      stored_query="$query"
      __fuzzybrowse_recursive=$((__fuzzybrowse_recursive==0))
    ;;
    ctrl-g)
      sel="$(echo "$sel"| rev | cut -f2- -d'/' | rev)"
      if [[ -d "$sel" ]]; then
        pushd "$sel" > /dev/null 2>&1
      fi
    ;;
    esac
  done
  dirs -c
  local x rel_path
  echo "$sel" | while read x; do
    if [[ ! -f "$x" ]]; then
      x=$(__fuzzybrowse_get_dir "$x")
    fi
    #rel_path="$(printf "%q\n" "$(__fuzzybrowse_relpath "$initial_dir" "$x")")"
    rel_path="$(__fuzzybrowse_relpath "$initial_dir" "$x")"
    if [[ "$rel_path" == "" ]]; then
      rel_path="$(pwd)"
    fi
    if [[ "${rel_path: -1}" == "/" ]]; then
      rel_path="${rel_path:0:-1}"
    fi
    fasd -A "$rel_path" > /dev/null 2>&1
    if [[ -n "$out_file" ]]; then
      echo "$rel_path" >> "$out_file"
    else
      echo "$rel_path"
    fi
  done
  cd "$initial_dir"
  export __fuzzybrowse_show_hidden=0
  export __fuzzybrowse_recursive=0
}


__fuzzybrowse_show_hidden=0
__fuzzybrowse_recursive=0

__fuzzybrow_file_ignore_pat="$(printf ".*\(%q\)$"  "$__fuzzybrow_file_ignore")"

__fuzzybrow_populate_dir_list(){
  local line
  
  if [[ "$__fuzzybrowse_recursive" == 1 ]]; then
    while read line ; do
        echo -e "\e[36m$line/"
    done
  else
    while read line ; do
      if [[ -d "$line" ]]; then
        echo -e "\e[36m$line/\e[0m $(cd "$line" && find . -maxdepth 1 -type f |head -9 | grep -v -i "$__fuzzybrow_file_ignore_pat" |cut -c3- | tr "\\n" "|" | sed 's/|/\\\e[36m | \\\e[0m/g')"
      fi
    done
  fi
}


__fuzzybrowse_file_source(){
  local max_dep=1
  if [[ -n "$1" ]]; then
    max_dep="$1"
  fi
  if [[ "$__fuzzybrowse_show_hidden" == 1 ]]; then
    find  . "$@" -maxdepth "$max_dep" -type f -o -xtype f ! -iregex "$__fuzzybrow_file_ignore_pat" | cut -c3-
  else
    find . "$@" -maxdepth "$max_dep" \( -type f -o -xtype f \) -not -path '*/\.*' ! -iregex "$__fuzzybrow_file_ignore_pat" | cut -c3-
  fi
}

__fuzzybrowse_dir_source(){
  local max_dep=1
  if [[ -n "$1" ]]; then
    max_dep="$1"
  fi
  if [[ "$__fuzzybrowse_show_hidden" == 1 ]]; then
    find . -maxdepth "$max_dep" -type d -o -type l | tail -n +2 | cut -c3- | __fuzzybrow_populate_dir_list
  else
    find . -maxdepth "$max_dep" \( -type d -o -type l \) -not -path '*/\.*' | tail -n +2 | cut -c3- | __fuzzybrow_populate_dir_list
  fi
}

__fuzzybrowse_get_dir(){
  echo "$@" | cut -f1 -d'/'
}

__fuzzybrowse_get_entry(){
  local tmp_dir
  if [[ -f "$@" ]]; then
    echo "$@"
  else
    tmp_dir=$(__fuzzybrowse_get_dir "$sel")
    if [[ -d "$tmp_dir" ]]; then
      echo "$tmp_dir"
    else
      echo "$@"
    fi
  fi
}

__fuzzybrowse_combined_source(){
  local max_dep=1
  if [[ "$__fuzzybrowse_recursive" == 1 ]]; then
    max_dep=99
  fi
  cat <(__fuzzybrowse_dir_source "$max_dep") <(__fuzzybrowse_file_source "$max_dep" ) 
}

__fuzzybrowse_fzf_cmd(){
  local prePrompt=""
  if [[ "$__fuzzybrowse_recursive" == 1 ]]; then
    prePrompt="{REC}"
  fi
  fzf --reverse --multi --prompt="$prePrompt""$(pwd): " --ansi --extended --print-query "$@"  --expect=ctrl-c,ctrl-x,ctrl-s,\#,return,ctrl-o,ctrl-u,\`,ctrl-q,ctrl-h,ctrl-z,ctrl-r,ctrl-e,ctrl-l,/,ctrl-v,left,right,ctrl-g

  #`# Hack to fix syntax highlight in vim..
}



__fuzzybrowse_full_path(){
  printf "%q" "$(cd "$(dirname "$1")"; pwd)/$(basename "$1")"
}

__fuzzybrowse_relpath(){
  # both $1 and $2 are absolute paths beginning with /
  # returns relative path to $2/$target from $1/$source
  source=$(__fuzzybrowse_full_path "$1")
  target=$(__fuzzybrowse_full_path "$2")

  if [[ "$source" == "$target" ]]; then
    echo "$source"
    return
  fi

  common_part=$source # for now
  result="" # for now

  while [[ "${target#$common_part}" == "${target}" ]]; do
    # no match, means that candidate common part is not correct
    # go up one level (reduce common part)
    common_part="$(dirname "$common_part")"
    # and record that we went back, with correct / handling
    if [[ -z $result ]]; then
      result=".."
    else
      result="../$result"
    fi
  done


  if [[ $common_part == "/" ]]; then
    # special case for root (no common path)
    #result="$result/"
    result="/"
  fi

  # since we now have identified the common part,
  # compute the non-common part
  forward_part="${target#$common_part}"

  # and now stick all parts together
  if [[ -n $result ]] && [[ -n $forward_part ]]; then
    result="$result$forward_part"
  elif [[ -n $forward_part ]]; then
    # extra slash removal
    result="${forward_part:1}"
  fi

  echo "$result"
}
