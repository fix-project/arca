ENTRY(_start)                /* the name of the entry label */

/* based on https://wiki.osdev.org/Memory_Map_(x86) */
/* QEMU should copy this to `lower`, but the loader will map it to `upper` */
MEMORY {
  lower (rwx) : ORIGIN = 0x0000000000200000, LENGTH = 2M
  stack (rw)  : ORIGIN = 0xFFFF800000100000, LENGTH = 1M
  upper (rwx) : ORIGIN = 0xFFFF800000200000, LENGTH = 2M
}

SECTIONS {
  .stack (NOLOAD) :
  {
    . = ORIGIN(stack);
    _sstack = .;
    . = . + 0x100000;
    _estack = .;
  } > stack
  .text :
  {
    _stext = ALIGN(16);
    KEEP(*(.text.start))
    *(.text*)
    _etext = ALIGN(16);
  } > upper AT> lower
  .rodata :
  {
    _srodata = ALIGN(16);
    *(.rodata*)
    *(.note*)
    _erodata = ALIGN(16);
  } > upper AT> lower
  .rodata :
  {
    _sgot = ALIGN(16);
    *(.got*)
    _egot = ALIGN(16);
  } > upper AT> lower
  .data :
  {
    _sdata = ALIGN(16);
    *(.data*)
    _edata = ALIGN(16);
  } > upper AT> lower
  .bss :
  {
    _sbss = ALIGN(16);
    *(.bss*)
    _ebss = ALIGN(16);
  } > upper AT> lower
  . = ALIGN(4);
}
