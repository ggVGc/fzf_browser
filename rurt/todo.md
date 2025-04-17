* Feat: Populate initial view from input file, instead of filesystem listing
* Feat: Sort by modification time
* Feat: Some kind of image preview
* Toggle action and CLI option for git information
* Feat: Stable tree file listing
  - Similar to running tree | fzf --no-sort
  - Filter input list based on full file paths, but don't change ordering
  - Keep directories in list, if any contained files are in the list
* Bug: Everything is easily scrambled in "git show" preview mode
* Bug: Colors don't work in bat in preview when opened inside neovim (but only the second time...)
* Bug: If you hit return during startup, colours don't work in bat in preview
* Bug: Keep cursor position when expanding a directory (ctrl-e)
* Multi selection
* Separate toggle for showing preview window, and preview window "mode"
* Feat: Alternative listing mode which doesn't reduce the list, but disables and grays out entries instead
* Feat: Move cursor to preview window in git log preview mode, fuzzy filter, selecting commit puts it into clipboard
* Feat: Preview-detail window:
  - Opens as horizontal split of preview windows
  - Allow focus shift to preview window, with fuzzy filtering
  - Show 'details' of current entry in preview-details window
  - Example: files changed in commit, when in 'git lg' view
* Git status view: Shows modified files in left pane and diffs in right pane:
  - Optionally show untracked
* Non-fuzzy filtering mode:
    - Narrows down list, keeping order
    - When Default action is pressed, exits back to fuzzy mode with narrowed list
    - Some action for undoing the filter
    - Display active filter under input line
* Action launching an external program, which should return a path, and cd to it:
  - Example usage: Jump to recent directories using fasd, z.sh etc.
* Jump to last visited directory in listing, when using DirBack action.
* Preview window flickers slightly on slow loads. There's some racy mitigations.
  - fzf does async loading in preview window, with a little progress bar.
* Indicator for current listing mode
* LRU the cache for the preview window?
* cursor stability as the list changes underneath it
* Support for indexed colours in rotting, once we work out what rotting should look like
* Support for terminal colours in rotting (impossible?)
* Config, themes
* Search term aware highlighting?
* Prevent grabbage of ctrl+c??
