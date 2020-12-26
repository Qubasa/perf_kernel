section .boot-first-stage
bits 16 ; tell NASM this is 16 bit code
global __bootloader_start
global stack_avail
global fresh_boot
global boots
extern bootloader_main
extern _rest_of_bootloader_start_addr
extern _rest_of_bootloader_end_addr

__bootloader_start:
    ; disable interrupts
    cli

    ; clear the direction flag (e.g. go forward in memory when using
    ; instructions like lodsb)
    cld

    mov sp, stack ; initialize stack

    enable_a20:
        in al, 0x92
        test al, 2
        jnz enable_a20_after
        or al, 2
        and al, 0xFE
        out 0x92, al
    enable_a20_after:

    call print_string
    call print_string

    ; Load a 32-bit GDT
    lgdt [gdt]

    ; zero segment registers
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov fs, ax
    mov gs, ax

    call 0x8:print_string

    ; Enable protected mode
    mov eax, cr0
    or  eax, (1 << 0)
    mov cr0, eax

    ; Transition to 32-bit mode by setting CS to a protected mode selector
    jmp 0x0018:pm_entry

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
    ; Enable int13h extension to load bigger kernels from disk
    mov ah, 0x41
    mov bx, 0x55aa
    ; dl contains drive number
    int 0x13

    mov eax, _rest_of_bootloader_start_addr

    ; dap buffer segment
    mov ebx, eax
    shr ebx, 4 ; divide by 16
    mov [dap_buffer_seg], bx

    ; buffer offset
    shl ebx, 4 ; multiply by 16
    sub eax, ebx
    mov [dap_buffer_addr], ax

    mov eax, _rest_of_bootloader_start_addr

    ; number of disk blocks to load
    mov ebx, _rest_of_bootloader_end_addr
    sub ebx, eax ; end - start
    shr ebx, 9 ; divide by 512 (block size)
    mov [dap_blocks], bx

    ; number of start block
    mov ebx, __bootloader_start
    sub eax, ebx
    shr eax, 9 ; divide by 512 (block size)
    mov [dap_start_lba], eax

    mov si, dap
    mov ah, 0x42
    int 0x13

    ; reset segment to 0
    mov word [dap_buffer_seg], 0

    ret

[bits 32]

pm_entry:
    ; Set up all data selectors
    mov ax, 0x20
    mov es, ax
    mov ds, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    ; Wait for the shared early boot stack to be available for use
    .wait_for_stack:
        pause
        xor  al, al
        lock xchg byte [stack_avail], al
        test al, al
        jz   short .wait_for_stack

    mov esp, stack ; set stack pointer
    cmp byte [fresh_boot], 1 ; Check if fresh boot
    jne short not_fresh_boot

    ; ~~~~ fresh boot section ~~~~
    ; Go back to real mode (16 bit mode)
    mov eax, cr0
    and al, 0xfe    ; clear protected mode bit
    mov cr0, eax

    ; Load kernel from disk
    call print_string

    ; Enable protected mode (32 bit mode)
    mov eax, cr0
    or  eax, (1 << 0)
    mov cr0, eax

    ; ~~~~ not fresh boot section ~~~~
    not_fresh_boot:
        mov byte [fresh_boot], 0 ; Set fresh boot to false
        call bootloader_main

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

align 8
gdt_base:
	dq 0x0000000000000000 ; 0x0000 | Null descriptor
	dq 0x00009a007c00ffff ; 0x0008 | 16-bit, code, base 0x7c00, 64k
	dq 0x008092000000ffff ; 0x0010 | 16-bit, data, base 0, 4Gb
	dq 0x00cf9a000000ffff ; 0x0018 | 32-bit, code, base 0, 4Gb
	dq 0x00cf92000000ffff ; 0x0020 | 32-bit, data, base 0, 4Gb
	dq 0x00209a0000000000 ; 0x0028 | 64-bit, code, base 0, ---
	dq 0x0000920000000000 ; 0x0030 | 64-bit, data, base 0, ---

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
