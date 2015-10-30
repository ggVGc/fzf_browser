fun! LaunchFuzzyBrowse(rootDir, initialQuery)
  exec "!fuzzybrowse ".fnameescape(a:rootDir)." ".a:initialQuery." > /tmp/vim_fbrowse_out"
  let l:res = readfile("/tmp/vim_fbrowse_out")
  if len(l:res) > 0
    return l:res[0]
  else
    return ""
  endif
endfun

fun! FuzzyBrowse(rootDir, initialQuery)
  let entry=LaunchFuzzyBrowse(a:rootDir, a:initialQuery)
  if isdirectory(entry)
    exe "lcd ".fnameescape(entry)
  else
    exe "e ".entry
  endif
endfun

fun! FuzzyPathFromHere()
  execute "normal! dv?[^-[:alnum:]_/~.+]\\zs\\\|^\<cr>"
  let str=@"
  if match(str[0], "[\"'<(]") == 0
    exec 'normal! i'.str[0]
    let str = str[1:]
  endif
  let spl = split(str, '/')
  if len(spl)==0
    let l:dir='.'
    let l:extra = ''
  elseif len(spl)==1
    if str[0]=='/'
      let l:extra=spl[0]
      let l:dir = ''
    else
      let l:dir=spl[0]
      let l:extra = ''
    endif
  else
    let l:dir=join(spl[:-2], '/')
    let extra=spl[-1]
  endif
  if str[0]=='/'
    let l:dir='/'.l:dir
  endif

  if isdirectory(l:dir)
    let res = LaunchFuzzyBrowse(l:dir, l:extra)
    if res != ""
      let str = res
    endif
  endif
  exec "normal! a".str
endf

command! -nargs=? FuzzyBrowse silent call FuzzyBrowse(<q-args>, "")|redraw!|echo "cwd: ".getcwd()
command! FuzzyBrowseHere silent call FuzzyBrowse(fnamemodify(expand("%"), ":p:h"), "")|redraw!|echo "cwd: ".getcwd()
inoremap <plug>FuzzyPath <esc>:call FuzzyPathFromHere()<cr>a
