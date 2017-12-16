#!/usr/bin/env bash

################## CONFIGURATION ##############

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
__fuzzybrow_file_ignore="log|bak|aux|lof|lol|lot|toc|bbl|blg|tmp|temp|swp|incomplete|o|class|pdb|cache|pyc|aria2|torrent|torrent.added|part|crdownload"
# List of folders to ignore, separated by |
__fuzzybrow_dir_ignore="_build|elm-stuff|node_modules|.git|.svn|.hg|deps"

#################### END CONFIGURATION #########


# Opens fuzzy dir browser. Tab to switch between file mode and directory mode. Esc to quit.
fzf_browser() {
  local res key sel prev_dir query stored_query tmp_prompt tmp_file tmp_dir
  local initial_dir
  initial_dir="$(pwd)"

  local start_query
  local out_file
  local custom_prompt
  local early_exit="--ansi"
  local fzf_opts="--ansi"
  local extraIgnoreDirs=""
  local extraIgnoreFiles=""
  while getopts "hrep:q:o:f:s:i:d:" opt; do
      case "$opt" in
      h)
        __fuzzybrowse_show_hidden=1
      ;;
      r)
        __fuzzybrowse_recursive=1
      ;;
      s)
        __fuzzybrowse_sort=1
      ;;
      e)
        early_exit="-1"
      ;;
      p)
        custom_prompt="$OPTARG"
      ;;
      q)
        start_query="$OPTARG"
      ;;
      o)
        out_file="$OPTARG"
      ;;
      f)
        fzf_opts="$OPTARG"
      ;;
      i)
        extraIgnoreFiles="|$OPTARG"
      ;;
      d)
        extraIgnoreDirs="|$OPTARG"
      ;;
      *)
        # invalid flag
      ;;
      esac
  done
  shift $((OPTIND-1))


  __fuzzybrow_file_ignore_pat="$(printf '.*\.\(%q\)$'  "$__fuzzybrow_file_ignore""$extraIgnoreFiles")"
  __fuzzybrow_dir_ignore_pat="$(printf '.*/\(%q\)\(/.*\|$\)'  "$__fuzzybrow_dir_ignore""$extraIgnoreDirs")"

  local start_dir
  start_dir="$1"

  if [[ "${start_query:0:1}" == "." ]]; then
    __fuzzybrowse_show_hidden=1
  fi
  stored_query="$start_query"

  if [[ -n "$out_file" ]]; then
    echo -n "" > "$out_file"
  fi

  if [[ -n "$start_dir" ]]; then
    if [[ "$start_dir" == "~" ]]; then
      start_dir="$HOME"
    fi
    if [[ -d "$start_dir" ]]; then
      cd "$start_dir" 
    else
      >&2 echo "Invalid start directory"
      return
    fi
  else
    start_dir="$initial_dir"
  fi
  start_query=""
  while true ; do
    if [[ -n "$custom_prompt" ]]; then
      tmp_prompt="--prompt=$custom_prompt""$(pwd)/"
    else
      tmp_prompt="--ansi"
    fi
    if [[ "$__fuzzybrowse_recursive" == 1 ]]; then
      res="$(__fuzzybrowse_all_source | __fuzzybrowse_fzf_cmd "$tmp_prompt" "$early_exit" "-q" "$stored_query")" 
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
    sel=$(echo "$res"|tail -n +3)
    key=$(echo "$res" | head -2 | tail -1)
    if [[ "$early_exit" == "-1" && -z "$key" ]]; then
      break
    fi
    early_exit="--ansi"

    case "$query" in
      ".")
        sel="$(pwd)/"
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
      ctrl-q)
        break;
        #if [[ -d "$sel" ]]; then
          #pushd "$sel" > /dev/null 2>&1
        #fi
      ;;
      ctrl-o)
        prev_dir="$(pwd)"
        popd > /dev/null 2>&1 || exit
      ;;
      ctrl-u)
        if [[ -n "$prev_dir" ]]; then
          pushd "$prev_dir" > /dev/null 2>&1 || exit
          prev_dir=""
        fi
      ;;
      \>)
        sel="$(pwd)/"
        break
      ;;
      return)
        tmp_file=$(__fuzzybrowse_get_entry "$sel")
        if [[ -f "$tmp_file" ]]; then
          sel="$tmp_file"
          break
        fi
        if [[ -d "$tmp_file" ]]; then
            pushd "$tmp_file" > /dev/null 2>&1 || exit
        else
          break
        fi
      ;;
      right)
        stored_query="$query"
        tmp_dir=$(__fuzzybrowse_get_dir "$sel")
        if [[ -d "$tmp_dir" ]]; then
          pushd "$tmp_dir" > /dev/null 2>&1 || exit
        else
          __fuzzybrowse_runFile "$sel"
        fi
      ;;
      ctrl-c)
        dirs -c
        cd "$initial_dir" || exit
        return
      ;;
      ctrl-a)
        stored_query="$query"
        __fuzzybrowse_show_hidden=$((__fuzzybrowse_show_hidden==0))
      ;;
      ctrl-y)
        stored_query="$query"
        __fuzzybrowse_sort=$((__fuzzybrowse_sort==0))
      ;;
      ctrl-h)
        pushd "$HOME" > /dev/null 2>&1  || exit
      ;;
      ctrl-z)
        tmp_dir="$(fasd -ld 2>&1 | sed -n 's/^[ 0-9.,]*//p' | fzf --tac +s)"
        if [[ -n "$tmp_dir" ]]; then
          pushd "$tmp_dir" > /dev/null 2>&1 || exit
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
    # ctrl-t)
    #   stored_query="$query"
    #     export e
    #     e="$(__fuzzybrowse_full_path "$(__fuzzybrowse_get_entry "$sel")")"
    #   __fuzzybrowse_runInTerminal "$SHELL"
    #   ;;
    \\)
      len=${#query}
      stored_query="${query:0:$len-1}"
      __fuzzybrowse_recursive=$((__fuzzybrowse_recursive==0))
    ;;
    ctrl-r)
      stored_query="$query"
      __fuzzybrowse_recursive=$((__fuzzybrowse_recursive==0))
    ;;
    ctrl-g)
      sel="$(echo "$sel"| rev | cut -f2- -d'/' | rev)"
      if [[ -d "$sel" ]]; then
        pushd "$sel" > /dev/null 2>&1 || exit
      fi
    ;;
    esac
  done
  dirs -c
  local x rel_path
  echo "$sel" | while read -r x; do
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
  cd "$initial_dir" || return
  export __fuzzybrowse_show_hidden=0
  export __fuzzybrowse_recursive=0


}


__fuzzybrowse_show_hidden=0
__fuzzybrowse_recursive=0

__fuzzybrow_populate_dir_list(){
  local line
  
  if [[ "$__fuzzybrowse_recursive" == 1 ]]; then
    while read -r line ; do
      if [[ -d "$line" ]]; then
        printf '\e[36m%s/\n' "$line"
      else
        printf '\e[0m%s\n' "$line"
      fi
    done
  else
    while read -r line ; do
      if [[ -d "$line" ]]; then
        printf "\e[36m$line/\e[0m $(cd "$line" && find . -maxdepth 1 -type f |head -9 | grep -v -i "$__fuzzybrow_file_ignore_pat" |cut -c3- | tr "\\n" "|" | sed 's/|/\\\e[36m | \\\e[0m/g')\n"
      fi
    done
  fi
}


__fuzzybrowse_file_source(){
  local max_dep=1
  if [[ -n "$1" ]]; then
    max_dep="$1"
  fi
  # TODO: -xtype temporarily disabled, because of OSX. Fix.
  # if [[ "$__fuzzybrowse_show_hidden" == 1 ]]; then
  #   find  . "$@" -maxdepth "$max_dep" -type f -o -xtype f ! -iregex "$__fuzzybrow_file_ignore_pat" | cut -c3-
  # else
  #   find . "$@" -maxdepth "$max_dep" \( -type f -o -xtype f \) -not -path '*/\.*' ! -iregex "$__fuzzybrow_file_ignore_pat" | cut -c3-
  # fi
  if [[ "$__fuzzybrowse_show_hidden" == 1 ]]; then
    find  . "$@" -maxdepth "$max_dep" -type f  -! -iregex "$__fuzzybrow_file_ignore_pat" | cut -c3-
  else
    find . "$@" -maxdepth "$max_dep" \( -type f \) -not -path '*/\.*' ! -iregex "$__fuzzybrow_file_ignore_pat" | cut -c3-
  fi
}

__fuzzybrowse_dir_source(){
  local max_dep=1
  if [[ -n "$1" ]]; then
    max_dep="$1"
  fi
  if [[ "$__fuzzybrowse_show_hidden" == 1 ]]; then
    find . -maxdepth "$max_dep" \( -type d -o -type l \) ! -iregex "$__fuzzybrow_dir_ignore_pat" | tail -n +2 | cut -c3- | __fuzzybrow_populate_dir_list
  else
    find . -maxdepth "$max_dep" \( -type d -o -type l \) -not -path '*/\.*' ! -iregex "$__fuzzybrow_dir_ignore_pat" | tail -n +2 | cut -c3- | __fuzzybrow_populate_dir_list
  fi
}

__fuzzybrowse_all_source(){
  if [[ "$__fuzzybrowse_show_hidden" == 1 ]]; then
    find . ! -iregex "$__fuzzybrow_dir_ignore_pat" ! -iregex "$__fuzzybrow_file_ignore_pat" | tail -n +2 | cut -c3- | __fuzzybrow_populate_dir_list
  else
    find .  -not -path '*/\.*' ! -iregex "$__fuzzybrow_dir_ignore_pat" ! -iregex "$__fuzzybrow_file_ignore_pat" | tail -n +2 | cut -c3- | __fuzzybrow_populate_dir_list
  fi
}


__fuzzybrowse_get_dir(){
  echo "$@" | rev |cut -f2- -d'/' | rev
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

  if [[ "$__fuzzybrowse_sort" == 1 ]]; then
    cat <(__fuzzybrowse_dir_source "$max_dep" | sort) <(__fuzzybrowse_file_source "$max_dep" | sort) 
  else
    cat <(__fuzzybrowse_dir_source "$max_dep") <(__fuzzybrowse_file_source "$max_dep" ) 
  fi


}

__fuzzybrowse_fzf_cmd(){
  local prePrompt=""
  if [[ "$__fuzzybrowse_recursive" == 1 ]]; then
    prePrompt="REC"
  fi
  if [[ "$__fuzzybrowse_show_hidden" == 1 ]]; then
    if [[ -n "$prePrompt" ]]; then
      prePrompt+="|"
    fi
    prePrompt+="HID"
  fi
  if [[ -n "$prePrompt" ]]; then
    prePrompt="{$prePrompt}"
  fi
  fzf "$fzf_opts" --reverse --multi --prompt="$prePrompt ""$(pwd): " --ansi --extended --print-query "$@"  --tiebreak=begin --expect=ctrl-c,ctrl-x,ctrl-s,\#,return,ctrl-o,ctrl-u,\`,\\,ctrl-h,ctrl-z,ctrl-r,ctrl-e,ctrl-l,/,ctrl-v,left,right,ctrl-g,\>,ctrl-a,ctrl-t,ctrl-y,ctrl-q

  #`# Hack to fix syntax highlight in vim..
}



__fuzzybrowse_full_path(){
  local base
  base="/$(basename "$1")"
  if [[ "$base" == "/." ]]; then
      base=""
  fi
  printf "%q" "$(cd "$(dirname "$1")"; pwd)$base"
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

  common_part="$source" # for now
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


  local no_common
  no_common=0
  if [[ $common_part == "/" ]]; then
    # special case for root (no common path)
    #result="$result/"
    result="/"
    no_common=1
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

  if [[ "${result:0:2}" == "//" ]]; then
    result="${result:1}"
  fi

  if [[ $no_common == 0 && "${result:0:1}" != "." ]]; then
    result="./$result"
  fi
  echo "$result"
}
