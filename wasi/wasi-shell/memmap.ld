ENTRY(_start)

MEMORY {
  memory (rwx) : ORIGIN = 0x100000000, LENGTH = 2M
}

SECTIONS {
  .text :
  {
    . = ALIGN(16);
    _stext = .;
    KEEP(*(.text.start))
    *(.text*)
    . = ALIGN(16);
    _etext = .;
  } > memory 
  .rodata :
  {
    . = ALIGN(16);
    _srodata = .;
    *(.rodata*)
    *(.note*)
    . = ALIGN(16);
    _erodata = .;
  } > memory 
  .got :
  {
    . = ALIGN(16);
    _sgot = .;
    *(.got*)
    . = ALIGN(16);
    _egot = .;
  } > memory 
  .data :
  {
    . = ALIGN(16);
    _sdata = .;
    *(.data*)
    . = ALIGN(16);
    _edata = .;
  } > memory 
  .bss (NOLOAD) :
  {
    . = ALIGN(16);
    _sbss = .;
    *(.sbss*)
    *(.bss*)
    . = ALIGN(16);
    _ebss = .;
  } > memory 
  .stack (NOLOAD) :
  {
    . = ALIGN(16);
    __stack_bottom = .;
    . = . + 64K;
    __stack_top = .;
  } > memory 
  /DISCARD/ :
  {
    *(.eh_frame*)
  }
}
