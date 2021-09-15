.section .init_bootloader, "awx"
.intel_syntax noprefix
.global _start_bootloader
.global switch_to_long_mode
.global jump_to_long_mode
.global gdt_64_pointer

.code32
_start_bootloader:
    lgdt gdt_64_pointer
    ljmp 0x18, offset switch_protected_mode

switch_protected_mode:
    mov esp, offset __stack_start
    push ebx
    push eax
    call bootloader_main

switch_to_long_mode:
    pop eax # return addr (discarded)
    pop edi # mem map
    pop esi # entry_point
    pop esp # stack pointer

    # Write back cache and add a memory fence. I'm not sure if this is
    # necessary, but better be on the safe side.
    wbinvd
    mfence

    # enable PAE-flag in cr4 (Physical Address Extension)
    mov eax, cr4
    or eax, (1 << 5)
    mov cr4, eax

    # set the long mode bit in the EFER MSR (model specific register)
    mov ecx, 0xC0000080
    rdmsr
    or eax, (1 << 8)
    wrmsr

    # enable paging in the cr0 register
    mov eax, cr0
    or eax, (1 << 31)
    mov cr0, eax

load_64bit_gdt:
    lgdt gdt_64_pointer                # Load GDT.Pointer defined below.

jump_to_long_mode:
    push 0x8
    mov eax, offset reset_state
    push eax
    retf # Load CS with 64 bit segment and flush the instruction cache

.code64
reset_state:
    mov byte ptr [stack_avail], 1
    xor rax, rax
    mov ss, rax
    mov es, rax
    mov gs, rax
    mov rax, 16 # offset to 3rd entry in gdt_64
    mov ds, rax
    jmp rsi

.align 4
gdt_64:
    .quad 0x0000000000000000          # Null Descriptor - should be present.
    .quad 0x00209A0000000000          # 64-bit code descriptor (exec/read).
    .quad 0x0000920000000000          # 64-bit data descriptor (read/write).
    .quad 0x00cf9a000000ffff          # 32-bit code descriptor (exec/read).
    .quad 0x00cf92000000ffff          # 32-bit data descriptor (read/write).

.align 4
gdt_64_pointer:
    .word gdt_64_pointer - gdt_64 - 1    # 16-bit Size (Limit) of GDT.
    .long gdt_64                         # 64-bit Base Address of GDT. (CPU will zero extend to 64-bit)
