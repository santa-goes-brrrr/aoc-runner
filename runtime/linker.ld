OUTPUT_ARCH(riscv)
ENTRY(_start)

/*
 * 0x80000000  solution image, then the heap
 * 0x8BEE0000  runtime boot and trap code
 * 0x8BEF0000  stack guard (locked with PMP, any access traps)
 * 0x8BF00000  stack, growing down from 0x8C000000
 * 0x8C000000  input packet
 * 0x8D000000  output mailbox and trap record
 */
MEMORY
{
    RAM (rwx) : ORIGIN = 0x80000000, LENGTH = 0x0BEE0000
    BOOT (rx) : ORIGIN = 0x8BEE0000, LENGTH = 0x00010000
}

SECTIONS
{
    /* Runtime code that is not part of any score. It sits outside the
     * solution image so that changing it never moves solution code or data. */
    .boot :
    {
        KEEP(*(.aoc_boot))
    } > BOOT

    . = ORIGIN(RAM);

    .text : ALIGN(8)
    {
        KEEP(*(.text.entry))
        __entry_end = .;
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
    /* The size of the guard must be a power of two and its start a multiple
     * of its size: the boot code encodes it as one NAPOT PMP entry. */
    __stack_guard_start = 0x8BEF0000;
    __stack_guard_end = 0x8BF00000;

    __heap_start = ALIGN(16);
    __heap_end = ORIGIN(BOOT);
    __stack_bottom = __stack_guard_end;
    __stack_top = 0x8C000000;

    ASSERT(_start == ORIGIN(RAM), "the entry stub must be the first instruction in RAM")
    ASSERT(__entry_end == ORIGIN(RAM) + 68, "the entry stub must stay 68 bytes long")
    ASSERT(__heap_start < __heap_end, "bare-metal image leaves no room for the heap")

    /DISCARD/ :
    {
        *(.eh_frame .eh_frame.*)
        *(.comment)
    }
}
