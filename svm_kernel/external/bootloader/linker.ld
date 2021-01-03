ENTRY(bootloader_main)

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
    }


    .kernel :
    {
        KEEP(*(.kernel))
    }
}
