* CLI option for saving result to a target file instead of printing
* Bug: Only last path separator is colored.
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
* Scroll preview window somehow?
* LRU the cache for the preview window?
* Show/hide preview window (and log?) with a keybinding
* cursor stability as the list changes underneath it
* Support for indexed colours in rotting, once we work out what rotting should look like
* Support for terminal colours in rotting (impossible?)
* Flicker prevention for directory navigation
* Input clearing behaviour for return/arrow key navigation
* Config, themes
* Search term aware highlighting?
* Prevent grabbage of ctrl+c??
