
Fuzzy directory and file browser for the shell, built around fzf.

![](doc/fzfbrowcast1.gif)

Zsh integration

![](doc/zsh_example.gif)


Installation
------------

#### Using git (recommended)

Clone this repository (recommended location ~/.fzf_browser)
```sh
git clone https://github.com/ggVGc/fzf_browser ~/.fzf_browser
```

#### Install
Make sure this repository is added to your $PATH, i.e
```sh
PATH=${PATH}:~/.fzf_browser
```

#### Zsh integration

Add the following line to your .zshrc
This will bind ctrl-b to open fuzzybrowse, and insert the result in the current command line.
```sh
bindkey "^b" _fuzzybrowse_zsh_insert_output
```


#### Vim plugin

Add the following line to your .vimrc.
```vim
set rtp+=~/.fzf_browser/vim
```

Usage
-----
#### Special queries
If any of these are entered as the input string, they will trigger an action instead of selecting the current entry.

| Input string                | Description                                                      |
| -------------------------- | ---------------------------------------------------------------- |
| `.`                   | Select current dir and exit. |
| `..`| Go to parent dir. |


#### Shell usage
| Command                | Description                                                      |
| -------------------------- | ---------------------------------------------------------------- |
| `fuzzybrowse`                   | Opens browser, and prints selected entries to stdout. |
| (Zsh only) `_fuzzybrowse_zsh_insert_output`| Should be mapped to something with bindkey(see installation example). Inserts output from `fuzzybrowse` into current command line. |

#### Vim usage

For these commands, if a directory is selected, `cd` to it. If one of more files are selected, opens them for editing.

| Command                                | Description                                                      |
| -------------------------------------- | ---------------------------------------------------------------- |
| `:FuzzyBrowse``<start_dir>`           | Opens fuzzybrowse in `<start_dir>`, or current working dir if no argument is given. |
| `:FuzzyBrowseHere`                     | Same as `FuzzyBrowse`, but starts in the directory of the current buffer(regardless of current working dir) |


| Map                                | Description                                                      |
| -------------------------------------- | ---------------------------------------------------------------- |
| `<plug>FuzzyPath`           | In insert mode, triggers path completion using fzf_browser  |

Example mapping:
```vim
imap <c-f> <plug>FuzzyPath
```



Configuration
-------------
See top of [fzf_browser.sh](https://github.com/ggVGc/fzf_browser/blob/master/fzf_browser.sh) for options/functions.

Key mappings:
-------------


|  &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;Key&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp; | Description                                                      |
| -------------------------------------- | ---------------------------------------------------------------- |
| `Enter`                                | If selection is directory, change to it. If it is a file, select it and exit. If multiple entries are selected, just exits(with the selections).|
| `/`                                    | Select entry, regardless if it's a directory or folder |
| `Tab`                                  | Select multiple files/directories.|
| `Right arrow`                          | `cd` into selected directory, or run file(same as ctrl-v for files) |
| `Left arrow`/ `#`/`` ` ``              | Go to parent directory|
| `Ctrl-l`                               | Preview selection. By default launches a `less` in a new terminal.|
| `Ctrl-q`                               | Toggle showing hidden files.|
| `Ctrl-r`                               | Toggle recursive mode. Lists all dirs/files recursively from current dir.|
| `Ctrl-g`                               | In file-recursive mode, go to directory of selected file. |
| `Ctrl-v`                               | View current selection using xdg-open |
| `Ctrl-z`                               | (Currently only works if application `fasd` is available) Select recent directory and switch to it. |
| `Ctrl-h`                               | Go to `$HOME`|
| `Ctrl-x`                               | Open `$SHELL`, with `$e` containing current selection.|
| `Ctrl-e`                               | Opens `$EDITOR` with selection.|
| `Ctrl-c`                               | Abort|
| `Ctrl-o`                               | Go backwards in directory stack.|
| `Ctrl-u`                               | Go forward in directory stack.(Currently only supports one jump)|

