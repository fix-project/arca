ENTRY(_start)                /* the name of the entry label */

/* based on https://wiki.osdev.org/Memory_Map_(x86) */
/* QEMU should copy this to `lower`, but the loader will map it to `upper` */
MEMORY {
  lower (rwx) : ORIGIN = 0x0000000000800000, LENGTH = 2M
  stack (rw)  : ORIGIN = 0xFFFF800000100000, LENGTH = 4M
  upper (rwx) : ORIGIN = 0xFFFF800000800000, LENGTH = 2M
}

SECTIONS {
  .stack (NOLOAD) :
  {
    . = ORIGIN(stack);
    _sstack = .;
    . = . + 0x400000;
    _estack = .;
  } > stack
  .text :
  {
    . = ALIGN(16);
    _stext = .;
    KEEP(*(.text.start))
    *(.text*)
    . = ALIGN(16);
    _etext = .;
  } > upper AT> lower
  .rodata :
  {
    . = ALIGN(16);
    _srodata = .;
    *(.rodata*)
    *(.note*)
    . = ALIGN(16);
    _erodata = .;
  } > upper AT> lower
  .data :
  {
    . = ALIGN(16);
    _sdata = .;
    *(.data*)
    . = ALIGN(16);
    _edata = .;
  } > upper AT> lower
  .got :
  {
    . = ALIGN(16);
    _sgot = .;
    *(.got*)
    . = ALIGN(16);
    _egot = .;
  } > upper AT> lower
  .tdata :
  {
    . = ALIGN(16);
    _stdata = .;
    *(.tdata*)
    . = ALIGN(16);
    _etdata = .;
  } > upper AT> lower
  .tbss (NOLOAD) :
  {
    . = ALIGN(16);
    _stbss = .;
    *(.tbss*)
    . = ALIGN(16);
    _etbss = .;
  } > upper
  .bss (NOLOAD) :
  {
    . = ALIGN(16);
    _sbss = .;
    *(.sbss*)
    *(.bss*)
    . = ALIGN(16);
    _ebss = .;
  } > upper
}
