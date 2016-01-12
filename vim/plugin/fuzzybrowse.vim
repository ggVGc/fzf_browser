fun! LaunchFuzzyBrowse(...)
  "let outFile="/tmp/vim_fbrowse_out"
  let outFile = tempname()
  let oldAutowrite = &autowrite
  set noautowrite
  exec "!fuzzybrowse ".join(a:000, ' ').' > '.outFile
  let &autowrite = oldAutowrite
  redraw!
  let l:res = filereadable(outFile) ? readfile(outFile) : ''
  if len(l:res) > 0
    return l:res[0]
  else
    return ''
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

fun! FuzFindPath()
  echo s:findPath(getline(line('.'))[:getpos('.')[2]-1])
endf


fun! s:findPath(content)
  let specialCharPat = "[^$~\.a-zA-Z0-9/\\\\\]"
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
  let [l:str, l:origLen] = s:findPath(getline(line('.'))[:getpos('.')[2]-1])
  let l:spl = split(l:str, '/')
  if len(l:spl)==0
    let l:dir='.'
    let l:extra = ''
  elseif len(l:spl)==1
    if l:str[0]=='/'
      let l:extra=l:spl[0]
      let l:dir = ''
    else
      let l:dir=l:spl[0]
      let l:extra = ''
    endif
  else
    if l:str[-1:] == '/'
      let l:dir=join(l:spl, '/')
      let l:extra=''
    else
      let l:dir=join(l:spl[:-2], '/')
      let l:extra=l:spl[-1]
    endif
  endif
  if l:str[0]=='/'
    let l:dir='/'.l:dir
  endif

  let l:res = LaunchFuzzyBrowse(l:extra==''?'': '-q '.l:extra, l:dir)

  if res != ''
    let oldReg=@x
    let @x=l:res
    if l:origLen > 0
      exec "normal! v".(l:origLen-1).'h"xp'
    else
      exec 'normal! "xp'
    endif
    let @x=oldReg
  endif
endf


command! -nargs=* -complete=file FuzzyBrowse silent call call('FuzzyBrowse', split(<q-args>))|echo "cwd: ".getcwd()
command! -nargs=? FuzzyBrowseHere silent call call('FuzzyBrowse', split(<q-args>)+[expand("%:p:h")])|echo "cwd: ".getcwd()
command! -nargs=? FuzzyInsertPath silent exec "normal! a".call('LaunchFuzzyBrowse', split(<q-args>))|echo "cwd: ".getcwd()
inoremap <plug>FuzzyPath <esc>:call FuzzyPathFromHere()<cr>a
"inoremap <expr> <plug>FuzzyPath        FuzzyPathFromHere()



