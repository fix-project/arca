ENTRY(_start)                /* the name of the entry label */

/* based on https://wiki.osdev.org/Memory_Map_(x86) */
/* QEMU should copy this to `lower`, but the loader will map it to `upper` */
MEMORY {
  lower (rwx) : ORIGIN = 0x0000000000200000, LENGTH = 14M
  upper (rwx) : ORIGIN = 0xFFFF800000200000, LENGTH = 14M
}

SECTIONS {
  . = ORIGIN(upper);
  _sstack = ORIGIN(upper)-0x100000;
  _estack = .;
  .text :
  {
    _stext = .;
    KEEP(*(.text.start))
    *(.text*)
    _etext = .;
  } > upper AT> lower
  .rodata :
  {
    _srodata = .;
    *(.rodata*)
    *(.note*)
    _erodata = .;
  } > upper AT> lower
  .rodata :
  {
    _sgot = .;
    *(.got*)
    _egot = .;
  } > upper AT> lower
  .data :
  {
    _sdata = .;
    *(.data*)
    _edata = .;
  } > upper AT> lower
  .bss :
  {
    _sbss = .;
    *(.bss*)
    _ebss = .;
  } > upper AT> lower
  . = ALIGN(4);
  _sheap = .;
  _eheap = ORIGIN(upper) + LENGTH(upper);
}
