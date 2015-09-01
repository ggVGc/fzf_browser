#!/usr/bin/env sh


# Use --expect with fzf, instead of hack with kill -9
# Print selections instead of using $EDITOR and cd automatically
# Finish type extension retrieval, with negation
# Add filtering selection to ui 
# Add option to start in edit mode instead




#Potential user mappings:
#Tab - Switch mode
#Return -  If in Text, call $EDITOR, otherwise call xdg-open
#Alt-E - Force $EDITOR
#Alt-Return - Force xdg-open


fuzzydir(){
#declare -a arr
  #local query sel
  #readarray -t arr < <(cat <(echo "..") <(find -L -maxdepth 1 -type d -not -path '*/\.*' | tail -n +2) | fzf --print-query "$@")
  local sel
  sel=$(cat <(echo "..") <(find -L -maxdepth 1 -type d -not -path '*/\.*' | tail -n +2) | fzf "$@")
  if [[ "$sel" == ".." ]]; then
    cd .. && fuzzydir "$@"
  else
    if [[ -z "$sel" ]]; then
      return
    else
      cd "$sel" && fuzzydir "$@"
    fi
  fi
}


fuzzyfile() {
  # Hack - Adding an extra empty line.
  # If there are no entries, fzf listens for input forever, and kill -9 doesn't work
  cat <(find -L -maxdepth 1 -type f -not -path '*/\.*' | grep -v -i "$1") <(echo "") | fzf ${2:+"$2"} 
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
  local ret2=$(__join \| "xml html tex haml css nfo markdown asciidoc cfg conf ada cs d nim c cpp hs rb lua moon fs go py sh js sql tex")
  printf "%q$" "($ret2)"
}

#extype(){
#}

fuzzyedit(){
  local fzf_opts="$@"
  local sel=$(fuzzyfile "$(typext image video audio document archives dbase junk binary)" "$fzf_opts")
  if [[ -n "$sel" ]]; then
    $EDITOR "$sel"
  fi
}


# Opens fuzzy dir browser. Tab to switch between file mode and directory mode. Esc to quit.
fuzzybrowse(){
  local mode
  echo "dir" > /tmp/fuzzybrow_switch_to
  while [[ -e /tmp/fuzzybrow_switch_to ]]; do
    mode=$(cat /tmp/fuzzybrow_switch_to)
    rm /tmp/fuzzybrow_switch_to
    if [[ "$mode" == "file" ]]; then
      bash -i -c 'fuzzyedit --bind=tab:execute@"echo "dir" > /tmp/fuzzybrow_switch_to && kill -9 $$"@'
    else
      bash -i -c 'fuzzydir --bind=tab:execute@"pwd > /tmp/fuzzybrow_new_dir && echo "file" > /tmp/fuzzybrow_switch_to && kill -9 $$"@ && pwd >/tmp/fuzzybrow_new_dir'
    fi
    if [[ -e /tmp/fuzzybrow_new_dir ]]; then
      cd "$(cat /tmp/fuzzybrow_new_dir)"
      rm /tmp/fuzzybrow_new_dir
    fi
    #bash -i -c 'fuzzydir --bind=tab:execute@"pwd > /tmp/fuzzybrow_new_dir && kill -9 $$"@ && pwd >/tmp/fuzzybrow_new_dir'
  done
}
