ENTRY(init_bootloader)

SECTIONS {
    . = 1M;
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


    .kernel :
    {
        KEEP(*(.kernel))
    }
}
