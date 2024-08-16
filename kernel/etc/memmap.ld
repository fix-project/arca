ENTRY(_multiboot)                /* the name of the entry label */

/* based on https://wiki.osdev.org/Memory_Map_(x86) */
/* QEMU should copy this to `lower`, but the loader will map it to `upper` */
MEMORY {
  cls   (rwx) : ORIGIN = 0x0000000000000000, LENGTH = 2M
  upper (rw)  : ORIGIN = 0xFFFF800000100000, LENGTH = 1M
}

SECTIONS {
  .multiboot :
  {
    . = ALIGN(16);
    KEEP(*(.multiboot))
  } > upper
  .text :
  {
    . = ALIGN(16);
    _stext = .;
    KEEP(*(.text.start))
    *(.text*)
    . = ALIGN(16);
    _etext = .;
  } > upper 
  .rodata :
  {
    . = ALIGN(16);
    _srodata = .;
    *(.rodata*)
    *(.note*)
    . = ALIGN(16);
    _erodata = .;
  } > upper 
  .got :
  {
    . = ALIGN(16);
    _sgot = .;
    *(.got*)
    . = ALIGN(16);
    _egot = .;
  } > upper 
  .cdata :
  {
    . = ALIGN(16);
    _scdata = .;
    _lcdata = LOADADDR(.cdata);
    QUAD(0xffffffffffffffff); // gs base
    *(.cdata*)
    . = ALIGN(16);
    _ecdata = .;
  } > cls AT> upper
  .data :
  {
    . += SIZEOF(.cdata); // cdata is in "cls", so no space is reserved automatically
    . = ALIGN(16);
    _sdata = .;
    *(.data*)
    . = ALIGN(16);
    _edata = .;
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
  /DISCARD/ :
  {
    *(.eh_frame*)
  }
}
