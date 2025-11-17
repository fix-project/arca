ENTRY(_start)

MEMORY {
  memory (rwx) : ORIGIN = 0x200000, LENGTH = 1M
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
    . = ORIGIN(memory) + LENGTH(memory) - 64K;
    __stack_bottom = .;
    . = ORIGIN(memory) + LENGTH(memory);
    __stack_top = .;
    . = ALIGN(4096);
  } > memory 
  /DISCARD/ :
  {
    *(.eh_frame*)
  }
}
