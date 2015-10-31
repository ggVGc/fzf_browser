fun! LaunchFuzzyBrowse(...)
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
  let specialCharPat = "[^\.a-zA-Z0-9/\\\\\]"
  let rev = join(reverse(split(a:content, '.\zs')), '')
  let lastWasSpecial=0
  let stepInd=0
  while stepInd<len(rev)
    let cur=rev[stepInd]
    let specialMatch = match(cur, specialCharPat)==0
    if lastWasSpecial && cur != '\'
      break
    endif
    let stepInd+=1
    let lastWasSpecial = specialMatch
  endwhile
  let stepInd-=1
  let ind=len(a:content)-stepInd+1
  if ind>0
    return [a:content[ind-1:-1], stepInd]
  else
    return ''
  endif
endf




fun! FuzzyPathFromHere()
  let [str, origLen] = s:findPath(getline(line('.'))[:getpos('.')[2]-1])
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

  let res = LaunchFuzzyBrowse(extra==''?'': '-q '.extra, l:dir)
  if res != ""
    let oldReg=@x
    let @x=res
    if origLen > 0
      exec "normal! v".(origLen-1).'h"xp'
    else
      exec 'normal! "xp'
    endif
    let @x=oldReg
  endif
endf


command! -nargs=? -complete=file FuzzyBrowse silent call call('FuzzyBrowse', split(<q-args>))|redraw!|echo "cwd: ".getcwd()
command! -nargs=? FuzzyBrowseHere silent call call('FuzzyBrowse', split(<q-args>)+[expand("%:p:h")])|redraw!|echo "cwd: ".getcwd()
command! -nargs=? FuzzyInsertPath silent exec "normal! a".call('LaunchFuzzyBrowse', split(<q-args>))|redraw!|echo "cwd: ".getcwd()
inoremap <plug>FuzzyPath <esc>:call FuzzyPathFromHere()<cr>a



