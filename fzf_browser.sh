#!/usr/bin/env bash


#TODO:
# Toggle for showing hidden files
# Hash of query strings mapped to paths
# toggle max-depth. Turn off file listing for dirs when maxdepth is disabled.
# Combined mode, file+dir
# Ctrl-o for going back to previous dir with same query string(implemented, but not with query string)
# Command for opening shell in current dir. Useful for jumping to new root etc. Optionally open with current selection appended.
# Rewrite with ShellMonad
# Toggle for selecting visible files from dir mode. (Only first 10 in dir are listed)
# support --multi. Maintain selection of files during all session
# Mode for removing or jumping to location of all selected files
# toggle options, like multi, while browsing
# Recursive mode for dir listing, toggled
# Finish type extension retrieval, with negation
# Add filtering selection to ui 


#Done
# Add option to start in edit mode instead
# Ctrl-c for aborting, i.e don't print anything
# Command for listing files in current dir. Right now, tab jumps into selection and lists files


#Potential user mappings:
#Tab - Switch mode
#Return -  If in Text, call $EDITOR, otherwise call xdg-open
#Alt-E - Force $EDITOR
#Alt-Return - Force xdg-open


#__fuzzybrow_file_ignore="wma|au|mid|midi|mka|mpc|ra|axa|oga|spx|xspf|flac|ogg|mp3|m4a|aac|wav|avi|mov|m2v|ogm|mp4v|vob|qt|nuv|asd|rm|rmvb|flc|fli|gl|m2ts|divx|axv|anx|ogv|ogx|mkv|webm|flv|mp4|m4v|mpg|mpeg|gif|bmp|pbm|pgm|ppm|tga|xbm|xpm|tif|tiff|svg|svgz|mng|pcx|dl|xcf|xwd|yuv|cgm|emf|eps|cr2|ico|jpg|jpeg|png|msi|exe|fla|iso|xz|zip|tar|7z|gz|bz|bz2|apk|tgz|lzma|arj|taz|lzh|tlz|txz|z|dz|lz|tbz|tbz2|tz|deb|rpm|jar|ace|rar|zoo|cpio|rz|gem|docx|pdf|odt|sqlite|log|bak|aux|lof|lol|lot|toc|bbl|blg|tmp|temp|swp|incomplete|o|class|cache|pyc|aria2|torrent|torrent.added|part|crdownload|ttf"
__fuzzybrow_file_ignore="log bak aux lof lol lot toc bbl blg tmp temp swp incomplete o class cache pyc aria2 torrent torrent.added part crdownload"

__fuzzybrow_populate_dir_list(){
  local line
  local ignore_pat="$(typext)"
  
  while read line ; do
    echo -e "\e[36m$line\t\e[0m$(cd "$line" && find -L . -maxdepth 1 -type f |head -9 | grep -v -i "$ignore_pat" |cut -c3- | tr "\\n" "|" | sed 's/|/\\\e[36m | \\\e[0m/g')"
  done
}


__fuzzydir_inner(){
  #-not -path '*/\.*' 
  find -L  . -maxdepth 1 -type d | tail -n +2 | cut -c3- | __fuzzybrow_populate_dir_list  | \
  #fzf --extended --ansi -d'\t' -n 1 --expect=, "$@" \
  fzf --extended --ansi --expect=, "$@" \
  | cut -f1 -d$'\t'
}

__fuzzydir(){
  local res key sel stored_res
  local init_q
  local query
  init_q="$1"
  local cwd=$(pwd)
  while true ; do
    res=$(__fuzzydir_inner -q "$init_q"  --prompt="[dir] $(pwd): " --print-query --expect=return,\;,:,\,,ctrl-o,ctrl-p,"${@:2}")
    { read -r query; read -r key; read -r sel; } <<< "$res"
    if [[ -z "$key" ]] ; then
      dirs -c
      return
    else
      stored_res="$res"
      case "$key" in
        ",")
          init_q=""
          pushd .. > /dev/null 2>&1
        ;;
        "return")
          init_q=""
          pushd "$sel" > /dev/null 2>&1
        ;;
        "ctrl-o")
          last_dir="$(pwd)"
          popd > /dev/null 2>&1
        ;;
        "ctrl-p")
          if [[ -n "$last_dir" ]]; then
            pushd "$last_dir" > /dev/null 2>&1
            last_dir=""
          fi
        ;;
        ";")
          cd "$sel"
          break
        ;;
        ":")
          break
        ;;
        *)
          break
        ;;
      esac
    fi
  done

  local ret
  ret=$(cat <(echo "$stored_res") <(pwd))
  dirs -c
  echo "$ret"
}






#declare -A __type_ext_map=( \
  #["font"]="ttf" \
  #["audio"]="wma au mid midi mka mpc ra axa oga spx xspf flac ogg mp3 m4a aac wav" \
  #["video"]="avi mov m2v ogm mp4v vob qt nuv asd rm rmvb flc fli gl m2ts divx axv anx ogv ogx mkv webm flv mp4 m4v mpg mpeg" \
  #["image"]="gif bmp pbm pgm ppm tga xbm xpm tif tiff svg svgz mng pcx dl xcf xwd yuv cgm emf eps cr2 ico jpg jpeg png" \
  #["binary"]="msi exe fla" \
  #["archives"]="iso xz zip tar 7z gz bz bz2 apk tgz lzma arj taz lzh tlz txz z dz lz tbz tbz2 tz deb rpm jar ace rar zoo cpio rz gem" \
  #["document"]="docx pdf odt" \
  #["text"]="nfo markdown asciidoc cfg conf" \
  #["markup"]="xml html tex haml css" \
  #["code"]="ada cs d nim c cpp hs rb lua moon fs go py sh js sql tex" \
  #["dbase"]="sqlite" \
  #["junk"]="log bak aux lof lol lot toc bbl blg tmp temp swp incomplete o class cache pyc aria2 torrent torrent.added part crdownload" \
  #["CodeMarkup"]="|code markup|" \
  #["Text"]="|text CodeMarkup|" \
#)

#declare -A __type_name_map=( \
  #["code"]="Makefile Rakefile" \
#)

function __join { local IFS="$1"; shift; echo "$*"; }

typext(){
  #local ret=()
  #for k in "$@"; do
    #ret+=($(__join \| "${__type_ext_map["$k"]}"))
  #done
  #ret+=($(__join \| "au mid midi mka mpc ra axa oga spx xspf flac ogg mp3 m4a aac wav avi mov m2v ogm mp4v vob qt nuv asd rm rmvb flc fli gl m2ts divx axv anx ogv ogx mkv webm flv mp4 m4v mpg mpeg gif bmp pbm pgm ppm tga xbm xpm tif tiff svg svgz mng pcx dl xcf xwd yuv cgm emf eps cr2 ico jpg jpeg png msi exe fla iso xz zip tar 7z gz bz bz2 apk tgz lzma arj taz lzh tlz txz z dz lz tbz tbz2 tz deb rpm jar ace rar zoo cpio rz gem docx pdf odt sqlite log bak aux lof lol lot toc bbl blg tmp temp swp incomplete o class cache pyc aria2 torrent torrent.added part crdownload"))
  #local ret2=$(__join \| "${ret[@]}")
  #printf "%q$" "($ret2)"
  printf ".*\(%q\)$"  "$__fuzzybrow_file_ignore"
}

#extype(){
#}

fuzzyfile() {
#  -not -path '*/\.*' \
  find -L . -maxdepth 1 -type f ! -iregex "$1" \
  | cut -c3- |fzf --extended --prompt "[file] $(pwd): " "${@:2}"
}

fuzzyedit(){
  fuzzyfile "$(typext image video audio document archives dbase junk binary)" "$@"
}

#full_path(){
  #printf "%q" "$(cd "$(dirname "$1")"; pwd)/$(basename "$1")"
#}


__fuzzybrowse_file_source(){
  find -L . -maxdepth 1 -type f ! -iregex "$1" | cut -c3-
}

__fuzzybrowse_file_handler(){
  echo "sdsa"
}

__fuzzybrowse_dir_handler(){
  echo "sdsa"
}
__fuzzybrowse_dir_source(){
  echo "sdsa"
}


__fuzzybrowse_source_from_mode(){
  case $1 in
    0)"__fuzzybrowse_dir_source" ;;
    1)"__fuzzybrowse_file_source" ;;
  esac
}

# Opens fuzzy dir browser. Tab to switch between file mode and directory mode. Esc to quit.
fuzzybrowse(){
  local fzf_cmd='fzf --extended --print-query --expect=tab,ctrl-c,\`,ctrl-x,ctrl-s'
  local res key sel dir_q file_q new_dir last_dir
  local mode=0
  local initial_dir="$(pwd)"
  local start_dir="$1"
  if [[ "$2" == "f" ]]; then
    mode=1
  fi
  if [[ -n "$start_dir" ]]; then
    cd "$start_dir" 
  else
    start_dir="$initial_dir"
  fi
  while true ; do
    case $mode in
      0)
        last_dir="$(pwd)"
        res=$(__fuzzydir "$dir_q" "tab,ctrl-c,\`")
        dir_q=$(echo "$res" | head -1)
        new_dir=$(echo "$res"|tail -1)
        if [[ "$last_dir" != "$new_dir" ]]; then
          file_q=""
        fi
        cd "$new_dir"
      ;;
      1)
        res=$(__fuzzybrowse_source_from_mode "$mode" | eval "$fzf_cmd" -q "\"$file_q\"")
        file_q=$(echo "$res" | head -1)
      ;;
    esac
    if [[ -z "$res" ]]; then
      cd "$initial_dir"
      return
    fi
    key=$(echo "$res" | head -2 | tail -1)
    case "$key" in
      ctrl-c)
        cd "$initial_dir"
        return
      ;;
      ctrl-s|ctrl-x)
        export f=$(full_path "$(echo "$res" | tail -1)")
        clear
        echo "\$f = $f"
        bash
      ;;
      tab)
        mode=$((mode==0))
      ;;
      \`)
        mode=$((mode==0))
        dir_q=""
        file_q=""
        if [[ "$key" == "\`" ]]; then
          if [[ "$mode" == 1 ]]; then
            if [[ -n "$res" ]]; then
              cd "$(echo "$res" | tail -2 | head -1)"
            fi
          else
            cd - > /dev/null
           fi
         fi
      ;;
      *)
        break
      ;;
    esac
  done
  local ret="$(echo "$res" | tail -1)"
  printf "%q" "$(realpath --relative-base="$initial_dir" "$ret")"
  cd "$initial_dir"
}


fuzzydir(){
  __fuzzydir "" "" | tail -1
}
