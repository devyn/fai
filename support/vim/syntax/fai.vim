" Vim syntax file
" Language: Fai assembler
" Maintainer: Devyn Cairns
" Latest Revision: 8 April 2017

if exists("b:current_syntax")
  finish
endif

syn iskeyword .,_,A-Z,a-z

" Keywords
syn keyword faiFunction
      \ bad nop set load store cmp branch branchl branchg
      \ branche branchne getsp setsp push pop call ret add
      \ sub mul div divmod not and or xor lsh rsh halt
      \ intsw inthw intpause intcont inthget inthset intexit
      \ trace

syn keyword faiDirective
      \ .words .len_words .bytes .len_bytes

syn keyword faiRegister a b c d

syn match faiNumber '-\?\d\+'
syn match faiNumber '-\?0x[A-Fa-f0-9]\+'
syn match faiNumber '-\?0b[01]\+'
syn match faiNumber '-\?0[0-7]\+'

syn keyword faiTodo contained TODO FIXME XXX NOTE
syn match faiComment ';.*$' contains=faiTodo

syn match faiLabelDef '[_A-Za-z][_.A-Za-z0-9]*:'

syn region faiWords start="{" end="}" fold transparent contains=faiNumber

syn match faiEscape contained '\\[\\"rn]'

syn region faiString start='be"' end='"' contains=faiEscape
syn region faiString start='le"' end='"' contains=faiEscape
syn region faiString start='BE"' end='"' contains=faiEscape
syn region faiString start='LE"' end='"' contains=faiEscape

syn match faiRelative contained '\$'
syn match faiOperator '[+-]'

syn region faiOperand start="\[" end="\]" transparent
      \ contains=faiRegister,faiNumber,faiRelative

let b:current_syntax = "fai"

hi def link faiFunction   Statement
hi def link faiDirective  PreProc
hi def link faiRegister   Identifier
hi def link faiNumber     Number
hi def link faiTodo       Todo
hi def link faiComment    Comment
hi def link faiLabelDef   Function
hi def link faiEscape     SpecialChar
hi def link faiString     String
hi def link faiRelative   Identifier
hi def link faiOperator   Operator
