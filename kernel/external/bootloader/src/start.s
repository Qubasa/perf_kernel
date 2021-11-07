.section .init_bootloader, "awx"
.global _start_bootloader
.global switch_to_long_mode
.global jump_to_long_mode
.global gdt_64_pointer
.global STACK_ARRAY
.global STACK_SIZE

.code32
_start_bootloader:
    lgdt gdt_64_pointer
    ljmp 24, offset switch_protected_mode

switch_protected_mode:
    mov dx, 32 # Set ds to 32-bit data segment
    mov ds, dx
    mov ss, dx
    mov esp, offset STACK_ARRAY
    add esp, STACK_SIZE
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

jump_to_long_mode:
    ljmp 8, offset reset_state

.align 8
.code64
reset_state:
    mov byte ptr [stack_avail], 1
    xor rax, rax
    mov ss, rax # in long mode these segment register are ignored
    mov es, rax
    mov gs, rax
    mov ds, rax
    cld
    jmp rsi
