ENTRY(_start)
PROVIDE(__stack_size = 8M);

MEMORY {
  memory (rwx) : ORIGIN = 0x7f00000000, LENGTH = 0x100000000
}

PHDRS {
  text PT_LOAD FLAGS(5);
  data PT_LOAD FLAGS(6);
  stack PT_LOAD FLAGS(6);
}

SECTIONS {
  .text :
  {
    . = ALIGN(16);
    _stext = .;
    KEEP(*(.text.start))
    *(.text*)
    *(.ltext*)
    . = ALIGN(16);
    _etext = .;
  } > memory :text
  .rodata :
  {
    . = ALIGN(16);
    _srodata = .;
    *(.rodata*)
    *(.lrodata*)
    *(.note*)
    . = ALIGN(16);
    _erodata = .;
  } > memory :text
  .eh_frame : {
    __eh_frame_start = .;
    KEEP(*(.eh_frame))
    __eh_frame_end = .;
  } > memory :text
  .eh_frame_hdr : {
    __eh_frame_hdr_start = .;
    KEEP(*(.eh_frame_hdr))
    __eh_frame_hdr_end = .;
  } > memory :text
  .gcc_except_table : { *(.gcc_except_table*) } > memory :text
  .init_array : {
    __init_array_start = .;
    KEEP(*(.init_array*))
    __init_array_end = .;
  } > memory :data
  .got :
  {
    . = ALIGN(16);
    _sgot = .;
    *(.got)
    . = ALIGN(16);
    _egot = .;
  } > memory :data
  .got.plt :
  {
    *(.got.plt)
  } > memory :data
  .data :
  {
    . = ALIGN(16);
    _sdata = .;
    *(.data*)
    *(.ldata*)
    . = ALIGN(16);
    _edata = .;
  } > memory :data
  .bss (NOLOAD) :
  {
    . = ALIGN(16);
    _sbss = .;
    *(.sbss*)
    *(.bss*)
    *(.lbss*)
    . = ALIGN(16);
    _ebss = .;
  } > memory :data
  .stack ALIGN(2M) (NOLOAD) :
  {
    __stack_bottom = .;
    . = . + __stack_size;
    __stack_top = .;
  } > memory :stack
  __heap_start = ALIGN(__stack_top, 2M);
  ASSERT(__stack_top - ORIGIN(memory) < 0x80000000,
         "Fix small-PIC image exceeds the 2 GiB static address span")
  __heap_end = ORIGIN(memory) + LENGTH(memory);

}
