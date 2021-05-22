# ENTRY(_start)

SECTIONS
{
  . = 0x800000001c000000;
  
  .text 0x800000001c000000 : {
    KEEP(*(.text.startup));
    *(.text .text.*);
  }
  .data : { *(.data) }
  .sdata : { *(.sdata) }
  .rodata : { *(.rodata) }

  .dynsym : { *(.dynsym) }
  .gnu.hash : { *(.gnu.hash) }
  .hash : { *(.hash) }
  .dynstr : { *(.dynstr) }
  .rela.dyn : { *(.rela.dyn) }
  .rela.opd : { *(.rela.opd) }
  .eh_frame_hdr : { *(.eh_frame_hdr) }
  .eh_frame : { *(.eh_frame) }

  .got : {
    __toc_start = .;
    
    *(.got)
    *(.toc)
  }

  . = ALIGN(256);
  __bss_start = .;
  .bss (NOLOAD) : { *(.bss .bss.*) }
  .sbss : { *(.sbss .sbss.*) }
  __bss_end = .;
}
