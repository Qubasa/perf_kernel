ENTRY(init_bootloader)

SECTIONS {
    . = 1M;
     __page_table_start = .;
        . = ALIGN(0x1000);
        _p4 = .;
        . += 0x1000;
        _p3 = .;
        . += 0x1000;
        _p2 = .;
        . += 0x1000;
        _p1 = .;
        . += 0x1000;
     __page_table_end = .;
     . = ALIGN(0x1000);
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
}
