ENTRY(_start)

MEMORY {
  memory (rwx) : ORIGIN = 0x7f00000000, LENGTH = 0x100000000
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
    *(.got)
    . = ALIGN(16);
    _egot = .;
  } > memory 
  .got.plt :
  {
    *(.got.plt)
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
    . = ALIGN(4K);
    __stack_bottom = .;
    . = . + 2M;
    __stack_top = .;
  } > memory 
  __heap_start = ALIGN(__stack_top, 2M);
  __heap_end = ORIGIN(memory) + LENGTH(memory);
  /DISCARD/ :
  {
    *(.eh_frame*)
  }
}
