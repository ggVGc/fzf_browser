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
  let oldReg=@x
  let pos=getpos('.')
  exec 'normal! a '
  exec 'normal! "xyT '
  let str=@x
  let @x=oldReg
  call setpos('.', pos)
  let spl = split(str, '/')
  let l:dir=join(spl[:-2], '/')
  if str[0]=='/'
    let l:dir='/'.l:dir
  endif
  let extra=spl[-1]
  if isdirectory(l:dir)
    let res = LaunchFuzzyBrowse(dir, extra)
    if res != ""
      let @x=res." "
      normal! vT "xp
      let @x=oldReg
    endif
  endif
  return ""
endf

command! -nargs=? FuzzyBrowse silent call FuzzyBrowse(<q-args>, "")|redraw!|echo "cwd: ".getcwd()
command! FuzzyBrowseHere silent call FuzzyBrowse(fnamemodify(expand("%"), ":p:h"), "")|redraw!|echo "cwd: ".getcwd()
inoremap <plug>FuzzyPath <esc>:call FuzzyPathFromHere()<cr>a
