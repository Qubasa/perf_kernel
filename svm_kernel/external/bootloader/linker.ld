ENTRY(_start_bootloader)

SECTIONS {
    . = 1M;
    __stack_end = .;
    . += 0x4000;
    __stack_start = .;

    . = ALIGN(0x1000);
    __bootloader_start = .;
    .boot : ALIGN(2)
    {
        /* ensure that the multiboot header is at the beginning */
        KEEP(*(.multiboot_header))
    }
    .bootloader : ALIGN(16)
    {
        *(.init_bootloader)
        *(.text .text.*)
        *(.rodata .rodata.*)
        *(.data .data.*)
        *(.bss .bss.*)
        *(.got .got.*)
    }
    . = ALIGN(0x1000);
    __bootloader_end = .;

    . = 2M;
    __kernel_start = .;
    .kernel :
    {
        KEEP(*(.kernel))
    }
    . = ALIGN(0x1000);
    __kernel_end = .;

    . = ALIGN(0x1000);
    __page_table_start = .;
        _p4 = .;
        . += 0x1000;
        _p3 = .;
        . += 0x1000;
        _p2_tables_start = .;
            . = (8M - 0x2000);
        _p2_tables_end = .;
    __page_table_end = .; /* Allocates 8Mb of RAM to map 4Gb of virt addr*/
    __minimum_mem_requirement = .;
}
