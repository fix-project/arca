```
A ::= O | K | E | L         Any
    | $atom                 Special/Primitive
    | @path                 Executable path

B ::= int literal           Blob
    | string literal        Blob

T ::= (A*)                  Tree

R ::= &B | &T               Ref

O ::= B | T | R             Data

K ::= ^O                    Thunk (Identify)
    | *T                    Thunk (Apply)
    | #T                    Thunk (Digest)
    | ~T                    Thunk (Select)
    | O[B]                  Thunk (Select 1)
    | O[B:B]                Thunk (Select N)

E ::= !K                    Encode (Strict)
    | ?K                    Encode (Shallow)

L ::= (let ((name A))      Let
           (A))
```

