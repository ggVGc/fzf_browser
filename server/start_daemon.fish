#!/usr/bin/env fish

eval (dircolors -c $HOME/.dir_colors)
fish_add_path $HOME/.local/bin
mix run --no-halt
