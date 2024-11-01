(fn quote-str [content]
  (.. "\"" content "\""))

(fn launch-fzf [query] 
  (io.popen (.. "fzf --query " (quote-str query)) "w"))

(fn populate-dir-list [path max-count]
    (io.popen 
      (.. "cd " path " && find . -maxdepth 1 -type f |head -" max-count)
      "r"))


(local fzf (launch-fzf "a thing"))

(local files (populate-dir-list "." 10))
(each [path (files:lines)]
  (fzf:write (.. path "\n")))
