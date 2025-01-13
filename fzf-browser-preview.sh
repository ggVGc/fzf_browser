#!/usr/bin/env bash
#
### Based on example from fzf (https://github.com/junegunn/fzf)
#
# The purpose of this script is to demonstrate how to preview a file or an
# image in the preview window of fzf.
#
# Dependencies:
# - https://github.com/sharkdp/bat
# - https://github.com/hpjansson/chafa
# - https://iterm2.com/utilities/imgcat

if [[ $# -ne 1 ]]; then
  >&2 echo "usage: $0 FILENAME"
  exit 1
fi

file=${1/#\~\//$HOME/}
type=$(file --dereference --mime -- "$file")

if [[ ! $type =~ image/ ]]; then
  if [[ $type =~ directory ]]; then
    ls --color=always "$(realpath $file)"
    exit
  fi

  if [[ $type =~ =binary ]]; then
    file "$1"
    exit
  fi

  # Sometimes bat is installed as batcat.
  if command -v batcat > /dev/null; then
    batname="batcat"
  elif command -v bat > /dev/null; then
    batname="bat"
  else
    cat "$1"
    exit
  fi

  ${batname} --style="${BAT_STYLE:-numbers}" --color=always --pager=never -- "$file"
  exit
fi


dim=${FZF_PREVIEW_COLUMNS}x${FZF_PREVIEW_LINES}
if [[ $dim = x ]]; then
  dim=$(stty size < /dev/tty | awk '{print $2 "x" $1}')
elif ! [[ $KITTY_WINDOW_ID ]] && (( FZF_PREVIEW_TOP + FZF_PREVIEW_LINES == $(stty size < /dev/tty | awk '{print $1}') )); then
  # Avoid scrolling issue when the Sixel image touches the bottom of the screen
  # * https://github.com/junegunn/fzf/issues/2544
  dim=${FZF_PREVIEW_COLUMNS}x$((FZF_PREVIEW_LINES - 1))
fi

# 1. Use kitty icat on kitty terminal
if [[ $KITTY_WINDOW_ID ]]; then
  # 1. 'memory' is the fastest option but if you want the image to be scrollable,
  #    you have to use 'stream'.
  #
  # 2. The last line of the output is the ANSI reset code without newline.
  #    This confuses fzf and makes it render scroll offset indicator.
  #    So we remove the last line and append the reset code to its previous line.
  kitty icat --clear --transfer-mode=memory --unicode-placeholder --stdin=no --place="$dim@0x0" "$file" | sed '$d' | sed $'$s/$/\e[m/'

elif command -v w3m > /dev/null; then
  # view_image.sh "$file"
  W3MIMGDISPLAY="/usr/lib/w3m/w3mimgdisplay"
  FILENAME=$1
  FONTH=14 # Size of one terminal row
  FONTW=8  # Size of one terminal column
  # COLUMNS=`tput cols`
  # LINES=`tput lines`

  read width height <<< `echo -e "5;$FILENAME" | $W3MIMGDISPLAY`

  max_width=$(($FONTW * $FZF_PREVIEW_COLUMNS))
  max_height=$(($FONTH * $(($FZF_PREVIEW_LINES - 2)))) # substract one line for prompt

  if test $width -gt $max_width; then
  height=$(($height * $max_width / $width))
  width=$max_width
  fi
  if test $height -gt $max_height; then
  width=$(($width * $max_height / $height))
  height=$max_height
  fi

  x=$(($FZF_PREVIEW_LEFT * $FONTW))
  y=$(($FZF_PREVIEW_TOP * $FONTH))
  w3m_command="0;1;$x;$y;$width;$height;;;;;$FILENAME\n4;\n3;"

  # tput cup $(($height/$FONTH)) 0
  echo -e $w3m_command|$W3MIMGDISPLAY

elif command -v chafa > /dev/null; then
  chafa -s "$dim" "$file"
  # Add a new line character so that fzf can display multiple images in the preview window
  echo

# 3. If chafa is not found but imgcat is available, use it on iTerm2
elif command -v imgcat > /dev/null; then
  # NOTE: We should use https://iterm2.com/utilities/it2check to check if the
  # user is running iTerm2. But for the sake of simplicity, we just assume
  # that's the case here.
  imgcat -W "${dim%%x*}" -H "${dim##*x}" "$file"

# 4. Cannot find any suitable method to preview the image
else
  file "$file"
fi
