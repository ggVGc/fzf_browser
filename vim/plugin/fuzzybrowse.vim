fun! FuzzyBrowse(rootDir)
  exec "!fuzzybrowse ".fnameescape(a:rootDir)." > /tmp/vim_fbrowse_out"
  let l:res = readfile("/tmp/vim_fbrowse_out")
  if len(l:res) > 0
    let l:e = l:res[0]
    if isdirectory(l:e)
      exe "lcd ".fnameescape(l:e)
    else
      exe "e ".l:e
    endif
  endif
endfun

command! -nargs=? FuzzyBrowse silent call FuzzyBrowse(<q-args>)|redraw!|echo "cwd: ".getcwd()
command! FuzzyBrowseHere silent call FuzzyBrowse(fnamemodify(expand("%"), ":p:h"))|redraw!|echo "cwd: ".getcwd()
