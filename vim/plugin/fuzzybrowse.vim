fun! LaunchFuzzyBrowse(...)
  "exec "!fuzzybrowse -q \"".a:initialQuery."\""." -o /tmp/vim_fbrowse_out ".join(options, ' ').fnameescape(a:rootDir)
  exec "!fuzzybrowse -o /tmp/vim_fbrowse_out ".join(a:000, ' ')
  let l:res = readfile("/tmp/vim_fbrowse_out")
  if len(l:res) > 0
    return l:res[0]
  else
    return ""
  endif
endfun

fun! FuzzyBrowse(...)
  let entry=call('LaunchFuzzyBrowse', a:000)
  if entry == ""
    return
  endif
  if isdirectory(entry)
    exe "lcd ".fnameescape(entry)
  else
    exe "e ".entry
  endif
endfun

fun! s:findPath(content)
  let specialCharPat = "[\"'<( =/]"
  let ind = len(a:content)-1
  while ind>0
    if a:content[ind] == '/'
      if a:content[ind-1]=='.'
        break
      elseif match(a:content[ind-1], specialCharPat)==0
        let ind+=1
        break
      endif
    endif
    let ind-=1
  endwhile
  if ind>0
    return a:content[ind-1:-1]
  else
    return ''
  endif
endf



fun! FuzzyPathFromHere()
  "execute "normal! dv?[^-[:alnum:]_/~.+]\\zs\\\|^\<cr>"
  "let str=@"
  "if match(str[0], "") == 0
    "exec 'normal! i'.str[0]
    "let str = str[1:]
  "endif
  let str = s:findPath(getline(line('.'))[:getpos('.')[2]-1])
  "exec "normal! ".(len(str)-1)."dhcl "
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
    if str[-1:] == '/'
      let l:dir=join(spl, '/')
      let extra=''
    else
      let l:dir=join(spl[:-2], '/')
      let extra=spl[-1]
    endif
  endif
  if str[0]=='/'
    let l:dir='/'.l:dir
  endif

  if isdirectory(l:dir)
    let res = LaunchFuzzyBrowse(extra==''?'': '-q '.extra, l:dir)
    if res != ""
      let oldReg=@x
      let @x=res
      let l=len(str)
      if l>0
        exec "normal! v".(l-1).'h"xp'
      else
        exec 'normal! "xp'
      endif
      let @x=oldReg
    endif
  endif
endf


command! -nargs=? -complete=file FuzzyBrowse silent call call('FuzzyBrowse', split(<q-args>))|redraw!|echo "cwd: ".getcwd()
command! -nargs=? FuzzyBrowseHere silent call call('FuzzyBrowse', split(<q-args>)+[expand("%:p:h")])|redraw!|echo "cwd: ".getcwd()
command! -nargs=? FuzzyInsertPath silent exec "normal! a".call('LaunchFuzzyBrowse', split(<q-args>))|redraw!|echo "cwd: ".getcwd()
inoremap <plug>FuzzyPath <esc>:call FuzzyPathFromHere()<cr>a



