ENTRY(_start)                /* the name of the entry label */

/* based on https://wiki.osdev.org/Memory_Map_(x86) */
MEMORY {
  trampoline (rwx) : ORIGIN = 0x00001000, LENGTH = 0x7EFF
  kernel     (x) : ORIGIN = 0x00100000, LENGTH = 13M
  bootloader (rwx) : ORIGIN = 0x00E00000, LENGTH = 1M
}

SECTIONS {
  . = ORIGIN(bootloader);
  _estack = .;
  .multiboot :
  {
    *(.multiboot*)
  } > bootloader
  .trampoline :
  {
    _strampoline = .;
    _ltrampoline = LOADADDR(.trampoline);
    *(.text.trampoline)
    _etrampoline = .;
  } > trampoline AT> bootloader
  .text :
  {
    _stext = .;
    *(.text.start)
    *(.text*)
    _etext = .;
  } > bootloader
  .rodata :
  {
    _srodata = .;
    *(.rodata*)
    *(.note*)
    _erodata = .;
  } > bootloader
  .data :
  {
    _sdata = .;
    *(.data*)
    _edata = .;
  } > bootloader
  .bss :
  {
    _sbss = .;
    *(.bss*)
    _ebss = .;
  } > bootloader
  . = ALIGN(4);
  _sheap = .;
  _eheap = ORIGIN(bootloader) + LENGTH(bootloader);
}
