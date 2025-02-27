* Behaves badly when used in pipe, which also breaks usage in, for example, fish shell:
  - Reproduce: `rurt | grep "yeo"`, or anything really
  - Nothing renders. If enter is pressed, the "selection" is sent over the pipe.
* Toggle option for colors in preview window:
  - Bit too much for some files
* Action launching an external program, which should return a path, and cd to it:
  - Example usage: Jump to recent directories using fasd, z.sh etc.
* Preview window flickers slightly on slow loads. There's some racy mitigations.
  - fzf does async loading in preview window, with a little progress bar.
