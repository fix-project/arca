```
H ::= O | K | E             Handle
    | $atom                 Special/Primitive

B ::= int literal           Blob
    | string literal        Blob

T ::= (H*)                  Tree

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
```

