ENTRY(_start)

SECTIONS {
  . = 0x200000;
  .text : {
    KEEP(*(.text.start))
    *(.text*) *(.ltext*)
  }
  .rodata : { *(.rodata*) *(.lrodata*) *(.note*) }
  .data : { *(.data*) *(.ldata*) *(.got*) }
  .bss (NOLOAD) : { *(.bss*) *(.lbss*) *(.sbss*) }
  .stack (NOLOAD) : {
    . = ALIGN(4096);
    . += 16K;
    __stack_top = .;
  }
  __heap_start = ALIGN(__stack_top, 4096);
  __heap_end = 0x40000000;
  /DISCARD/ : { *(.eh_frame*) }
}
