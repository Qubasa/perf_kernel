.section .init_bootloader, "awx"
.intel_syntax noprefix

init_bootloader:
    push ebx
    push eax
    call bootloader_main
