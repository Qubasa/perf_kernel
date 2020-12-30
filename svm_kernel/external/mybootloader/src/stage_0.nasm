section .boot-first-stage
bits 16 ; tell NASM this is 16 bit code
global __bootloader_start
global stack_avail
global fresh_boot
global boots
extern bootloader_main
extern _kernel_size
extern _kernel_start_addr
extern _kernel_buffer

__bootloader_start:
    ; disable interrupts
    cli

    ; clear the direction flag (e.g. go forward in memory when using
    ; instructions like lodsb)
    cld

    ; Wait for the shared early boot stack to be available for use
    .wait_for_stack:
        pause
        xor  al, al
        lock xchg byte [stack_avail], al
        test al, al
        jz   short .wait_for_stack

    ; initialize stack
    mov sp, stack

    ; enable A20 line if not already done
    enable_a20:
        in al, 0x92
        test al, 2
        jnz enable_a20_after
        or al, 2
        and al, 0xFE
        out 0x92, al
    enable_a20_after:

    call print_string

    call load_kernel_from_disk

    call print_string

    lgdt [gdt]

    ; Enable protected mode
    mov eax, cr0
    or  eax, (1 << 0)
    mov cr0, eax

    mov ax, 0x10
    mov ds, ax ; set data segment
    mov es, ax ; set extra segment
    mov ss, ax ; set extra segment

    ; Transition to 32-bit mode by setting CS to a protected mode selector
    jmp 0x8:pm_entry

print_string:
    mov si,hello ; point si register to hello label memory location
    mov ah,0x0e ; 0x0e means 'Write Character in TTY mode'
    .loop:
        lodsb
        or al,al ; is al == 0 ?
        jz myret  ; if (al == 0) jump to halt label
        int 0x10 ; runs BIOS interrupt 0x10 - Video Services
        jmp .loop
    myret:
    ret
hello: db "Hello world!",0


load_kernel_from_disk:
    ; start of memory buffer
    lea eax, [_kernel_buffer]
    mov [dap_buffer_addr], ax

    ; number of disk blocks to load
    mov word [dap_blocks], 1

    ; number of start block
    lea eax, [_kernel_start_addr]
    lea ebx, [__bootloader_start]
    sub eax, ebx
    shr eax, 9 ; divide by 512 (block size)
    mov [dap_start_lba], eax

    ; destination address
    mov edi, 0x400000

    ; block count
    lea ecx, [_kernel_size]
    add ecx, 511 ; align up
    shr ecx, 9

load_next_kernel_block_from_disk:
    ; load block from disk
    lea si, [dap]
    mov ah, 0x42
    int 0x13

    ; copy block to 2MiB
    push ecx
    push esi
    mov ecx, 512 / 4
    ; move with zero extension
    ; because we are moving a word ptr
    ; to esi, a 32-bit register.
    movzx esi, word [dap_buffer_addr]
    ; move from esi to edi ecx times.
    rep movsd
    pop esi
    pop ecx

    ; next block
    mov eax, [dap_start_lba]
    add eax, 1
    mov [dap_start_lba], eax

    sub ecx, 1
    jnz load_next_kernel_block_from_disk
    ret

[bits 32]

pm_entry:

    mov byte [fresh_boot], 0 ; Set fresh boot to false

    ; Print char on vga console
    mov bx, 0x0f01         ; attrib/char of smiley
    mov eax, 0xb8f00       ; note 32 bit offset
    mov word [eax], bx
    ; call bootloader_main

halt:
    hlt ; halt execution

align 8
stack:
    times 64 db 0

; We release the early boot stack
; once we get into the kernel and are using a new stack. We write directly to
; this location.
stack_avail: db 1

; Fresh boot
fresh_boot: db 1

; Number of boots such that we can track the number of boots, including soft
; reboots. This value is not reset upon a soft reboot, and thus persists.
align 8
boots: dq 0

; ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
; x/7g 0x00007d28
align 8
gdt_base:
	dq 0x0000000000000000 ; 0x0000 | Null descriptor
	dq 0x00cf9a000000ffff ; 0x0008 | 32-bit, code, base 0, 4Gb
	dq 0x00cf92000000ffff ; 0x0010 | 32-bit, data, base 0, 4Gb
	dq 0x00209a0000000000 ; 0x0018 | 64-bit, code, base 0, ---
	dq 0x0000920000000000 ; 0x0020 | 64-bit, data, base 0, ---

gdt:
	dw (gdt - gdt_base) - 1
	dd gdt_base

; ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

align 8
dap: ; disk access packet
    db 0x10 ; size of dap
    db 0 ; unused
dap_blocks:
    dw 0 ; number of sectors
dap_buffer_addr:
    dw 0 ; offset to memory buffer
dap_buffer_seg:
    dw 0 ; segment of memory buffer
dap_start_lba:
    dq 0 ; start logical block address

times 510 - ($-$$) db 0 ; pad remaining 510 bytes with zeroes
dw 0xaa55 ; magic bootloader magic - marks this 512 byte sector bootable!
