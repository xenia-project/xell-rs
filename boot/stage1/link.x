# ENTRY(_start)

SECTIONS
{
  . = 0x800000001c000000;
  
  .text : {
    KEEP(*(.text.startup));
    *(.text .text.*);
  }
  .data : { *(.data) }
  .sdata : { *(.sdata) }
  .rodata : { *(.rodata) }

  .got : {
    __toc_start = .;
    
    *(.got)
    *(.toc)
  }

  . = ALIGN(256);
  __bss_start = .;
  .bss : { *(.bss .bss.*) }
  .sbss : { *(.sbss .sbss.*) }
  __bss_end = .;
}
