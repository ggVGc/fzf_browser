#!/usr/bin/env bash


# Finish type extension retrieval, with negation
# Add filtering selection to ui 
# Add option to start in edit mode instead
# support --multi
# toggle options, like multi, while browsing




#Potential user mappings:
#Tab - Switch mode
#Return -  If in Text, call $EDITOR, otherwise call xdg-open
#Alt-E - Force $EDITOR
#Alt-Return - Force xdg-open


__fuzzybrow_file_ignore="au|mid|midi|mka|mpc|ra|axa|oga|spx|xspf|flac|ogg|mp3|m4a|aac|wav|avi|mov|m2v|ogm|mp4v|vob|qt|nuv|asd|rm|rmvb|flc|fli|gl|m2ts|divx|axv|anx|ogv|ogx|mkv|webm|flv|mp4|m4v|mpg|mpeg|gif|bmp|pbm|pgm|ppm|tga|xbm|xpm|tif|tiff|svg|svgz|mng|pcx|dl|xcf|xwd|yuv|cgm|emf|eps|cr2|ico|jpg|jpeg|png|msi|exe|fla|iso|xz|zip|tar|7z|gz|bz|bz2|apk|tgz|lzma|arj|taz|lzh|tlz|txz|z|dz|lz|tbz|tbz2|tz|deb|rpm|jar|ace|rar|zoo|cpio|rz|gem|docx|pdf|odt|sqlite|log|bak|aux|lof|lol|lot|toc|bbl|blg|tmp|temp|swp|incomplete|o|class|cache|pyc|aria2|torrent|torrent.added|part|crdownload"

__fuzzybrow_populate_dir_list(){
  local line
  local ignore_pat
  ignore_pat=$(typext)
  
  while read line ; do
    echo "\e[36m$line\t\e[0m$(cd "$line" && find -L . -maxdepth 1 -type f | grep -v -i "$ignore_pat" |cut -c3- | tr "\\n" " " | sed 's/,/, /g')"
  done
}


__fuzzydir_inner(){
  cat <(find -L  . -maxdepth 1 -type d -not -path '*/\.*' | tail -n +2 | cut -c3- | __fuzzybrow_populate_dir_list ) <(echo "..") |  fzf --ansi -d'\t' -n 1 --expect=, "$@" | cut -f1 -d$'\t'
}

__fuzzydir(){
  local res key sel stored_res
  local init_q
  init_q="$1"
  local cwd=$(pwd)
  while true ; do
    lastres="$res"
    res=$(__fuzzydir_inner -q "$init_q"  --print-query $(printf "%q " "${@:2}"))
    init_q=""
    key=$(echo "$res" | tail -2 | head -1)
    sel=$(echo "$res" | tail -1)
    if [[ -z "$key" ]]; then
      if [[ "$sel" == ".." ]]; then
        cd ..
        stored_res="$res"
      elif [[ -n "$sel" ]]; then
        cd "$sel" 
        stored_res="$res"
      else
        break
      fi
    else
      stored_res="$res"
      break
    fi
  done

  local ret
  ret=$(cat <(echo "$stored_res") <(pwd))
  cd "$cwd"
  echo "$ret"
}






#declare -A __type_ext_map=( \
  #["audio"]="au mid midi mka mpc ra axa oga spx xspf flac ogg mp3 m4a aac wav" \
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
  printf "\(%q\)$"  "$__fuzzybrow_file_ignore"
}

#extype(){
#}

fuzzyfile() {
  find -L . -maxdepth 1 -type f -not -path '*/\.*' | grep -v -i "$1"| fzf "${@:2}"
}

fuzzyedit(){
  fuzzyfile "$(typext image video audio document archives dbase junk binary)" "$@"
}

full_path(){
  echo "$(cd "$(dirname "$1")"; pwd)/$(basename "$1")"
}

# Opens fuzzy dir browser. Tab to switch between file mode and directory mode. Esc to quit.
fuzzybrowse(){
  local res key sel dir_q file_q new_dir
  file_q=""
  local mode=0
  local cwd=$(pwd)
  local last_dir
  while true ; do
    case $mode in
      0)
        last_dir="$(pwd)"
        res=$(__fuzzydir "$dir_q" --expect=tab --print-query)
        dir_q=$(echo "$res" | head -1)
        new_dir=$(echo "$res"|tail -1)
        if [[ "$last_dir" != "$new_dir" ]]; then
          file_q=""
        fi
        cd "$new_dir"
      ;;
      1)
        res=$(fuzzyedit --expect=tab --print-query -q "$file_q")
        file_q=$(echo "$res" | head -1)
      ;;
    esac
    key=$(echo "$res" | head -2 | tail -1)
    case "$key" in
      tab)
        mode=$((mode==0))
        if [[ "$mode" == 1 ]]; then
          if [[ -n "$res" ]]; then
            cd "$(echo "$res" | tail -2 | head -1)"
          fi
        else
           cd -
        fi
      ;;
      *)
        break
      ;;
    esac
  done
  full_path "$(echo "$res" | tail -1)"
  cd "$cwd"
}


fuzzydir(){
  __fuzzydir "" "$@" | head -n 2
}
