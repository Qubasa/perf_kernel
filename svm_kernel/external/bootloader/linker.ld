ENTRY(_start_bootloader)

SECTIONS {
    . = 0x4000;
    . = ALIGN(0x1000);
    __smp_trampoline_start = .;
    .smp_trampoline :
    {
        *(.smp_trampoline)
    }
    . = ALIGN(0x1000);
    __smp_trampoline_end = .;
    . = 1M;

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
            . += 0x4000; /* One p2 table equals 1Gb memory mapped */
        _p2_tables_end = .;
        _p1_tables_start = .;
        . += 0x1000; /* First p1 table to map 0-2Mb with 4Kb pages*/
        _p1_tss_tables_start = .;
        . += 0x1000 * 128; /* 1Mb per core for interrupt stacks (max 256 cores) */
        _p1_tables_end = .;
    __page_table_end = .; 
    __minimum_mem_requirement = .;
}
