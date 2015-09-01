#!/usr/bin/env sh

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
  local exclude fzf_opts
  exclude="$1"
  fzf_opts="$2"
  if [[ "$fzf_opts" == "" ]]; then
    echo $(find -L -maxdepth 1 -type f -not -path '*/\.*' | grep -v -i "$exclude"| fzf)
  else
    echo $(find -L -maxdepth 1 -type f -not -path '*/\.*' | grep -v -i "$exclude"| fzf "$fzf_opts")
  fi
}


fuzzyedit(){
  local sel fzf_opts
  fzf_opts="$@"
  sel=$(fuzzyfile "\(.\(docx\|xz\|webm\|msi\|crdownload\|exe\|aria2\|iso\|torrent\|torrent\.added\|part\|jpg\|png\|jpeg\|zip\|flv\|m4a\|mpg\|mpeg\|avi\|pdf\|tiff\|mp3\|m4a\|mp4\|mkv\|apk\|torrent\|tgz\|bz2\|odt\|gz\|7z\|bz\|tar\)$\)" "$fzf_opts")
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
