bits 64

mov eax, [rel foo]
out dx, eax
hlt
foo:
  dd 0xdeadbeef
