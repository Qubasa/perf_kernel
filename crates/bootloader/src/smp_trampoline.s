.section .smp_trampoline, "awx"
.global _smp_trampoline
.global stack_avail
.global undefined_instruction
.global gdt_64_pointer

.code16
_smp_trampoline:
  # clear the direction flag (e.g. go forward in memory when using
  # instructions like lodsb)
  cld
  # disable interrupts
  cli

  # Set the A20 line
  in al, 0x92
  or al, 2
  out 0x92, al

  # Load 32-bit GDT
  lgdt gdt_64_pointer

  # Enable protected mode
  mov eax, cr0
  or  eax, (1 << 0)
  mov cr0, eax

  ljmp 24, offset protected_mode_setup

.align 4
.code32
protected_mode_setup:
  mov bx, 32
  mov ds, bx
  mov es, bx
  mov fs, bx
  mov gs, bx
  mov ss, bx


# spin loop till stack is available
wait_for_stack:
  xor al, al
  lock xchg byte ptr [stack_avail], al
  test al, al
  jz wait_for_stack
  mov byte ptr [stack_avail], 0

  mov esp, offset STACK_ARRAY
  add esp, STACK_SIZE
  call smp_main

spin:
  jmp spin

undefined_instruction:
  .long 0xffffffff

.align 2
stack_avail: .byte 1

.align 4
gdt_64:
    .quad 0x0000000000000000          # Null Descriptor - should be present.
    .quad 0x00209A0000000000          # 8  | 64-bit code descriptor (exec/read).
    .quad 0x0000920000000000          # 16 | 64-bit data descriptor (read/write).
    .quad 0x00cf9a000000ffff          # 24 | 32-bit code descriptor (exec/read).
    .quad 0x00cf92000000ffff          # 32 | 32-bit data descriptor (read/write).
    .quad 0x00009a004000ffff          # 40 | 16-bit code descriptor
    .quad 0x00009a007c00ffff          # 48 | 16-bit data descriptor

.align 4
gdt_64_pointer:
    .word gdt_64_pointer - gdt_64 - 1    # 16-bit Size (Limit) of GDT.
    .long gdt_64                         # 64-bit Base Address of GDT. (CPU will zero extend to 64-bit)
