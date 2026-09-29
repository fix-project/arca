### Text Format

```
P ::= (name = A newline+)* A

A ::= O | K | E             Any
    | name                  Identifier
    | $name                 Stdlib primitive

int ::= num_u8              Integer
      | num_u16
      | num_u32
      | num_u64
      | num_u128

B ::= int                   Blob
    | string
    | @"/path/"             File contents

T ::= (A*)                  Tree

R ::= &B | &T               Ref

O ::= B | T | R             Data

K ::= 'O                    Thunk (Identify)
    | #T                    Thunk (Apply)
    | [A*]                  Thunk (Select)
    | [O int]               Thunk (Select 1)
    | [O int int]           Thunk (Select N)

E ::= *K                    Encode (Strict)
    | +K                    Encode (Shallow)

comment ::= --              Single line
          | {- -}           Multi-line
```

### Example

```
compiler = @"/path/"
*#(*#(compiler $def_limits
    "int f(int x, int y) {return x + y;}")
    $def_limits 19u32 4u32)
```
