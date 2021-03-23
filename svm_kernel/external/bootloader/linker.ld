ENTRY(init_bootloader)

SECTIONS {
    . = 2M;
    __offset = .; /* Do not move this label */
    __stack_end = .;
    . += 0x2000;
    __stack_start = .;
    __bootloader_start = .;
    .boot :
    {
        /* ensure that the multiboot header is at the beginning */
        KEEP(*(.multiboot_header))
    }
    .text :
    {
        *(.text)
        *(.init_bootloader)
    }
    __bootloader_end = .;

    . = ALIGN(0x1000);
    .kernel :
    {
        KEEP(*(.kernel))
    }

    . = ALIGN(0x1000);
    __page_table_start = .;
        _p4 = .;
        . += 0x1000;
        _p3 = .;
        . += 0x1000;
        _p2_tables_start = .;
            . = (0x800000 - 0x1000*2);
        _p2_tables_end = .;
    __page_table_end = .; /* Allocates 8Mb of RAM */
}
