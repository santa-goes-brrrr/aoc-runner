OUTPUT_ARCH(riscv)
ENTRY(_start)

MEMORY
{
    RAM (rwx) : ORIGIN = 0x80000000, LENGTH = 0x0BF00000
}

SECTIONS
{
    . = ORIGIN(RAM);

    .text : ALIGN(8)
    {
        KEEP(*(.text.entry))
        *(.text .text.*)
    } > RAM

    .rodata : ALIGN(8)
    {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    } > RAM

    .data : ALIGN(8)
    {
        *(.data .data.*)
        *(.sdata .sdata.*)
    } > RAM

    .got : ALIGN(8)
    {
        *(.got .got.*)
    } > RAM

    .bss (NOLOAD) : ALIGN(8)
    {
        __bss_start = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        *(COMMON)
        . = ALIGN(8);
        __bss_end = .;
    } > RAM

    __global_pointer$ = ADDR(.data) + 0x800;
    __heap_start = ALIGN(16);
    __heap_end = 0x8BF00000;
    __stack_bottom = 0x8BF00000;
    __stack_top = 0x8C000000;

    ASSERT(__heap_start < __heap_end, "bare-metal image leaves no room for the heap")

    /DISCARD/ :
    {
        *(.eh_frame .eh_frame.*)
        *(.comment)
    }
}
