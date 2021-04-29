.section .smp_trampoline, "awx"
.intel_syntax noprefix
.global _smp_trampoline
.global stack_avail

.code16
_smp_trampoline:
  # clear the direction flag (e.g. go forward in memory when using
  # instructions like lodsb)
  cld
  # disable interrupts
  cli

  # zero data segment
  xor ax, ax
  mov ds, ax

  # Set the A20 line
  in al, 0x92
  or al, 2
  out 0x92, al

  # Load 32-bit GDT
  lgdt gdt32_pointer

  # Enable protected mode
  mov eax, cr0
  or  eax, (1 << 0)
  mov cr0, eax

  ljmp 0x8, offset protected_mode_setup

.code32
protected_mode_setup:
  jmp smp_main
  mov bx, 0x10
  mov ds, bx

# spin loop till stack is available
wait_for_stack:
  xor al, al
  lock xchg byte ptr [stack_avail], al
  test al, al
  jz wait_for_stack
  mov byte ptr [stack_avail], 0
  jmp smp_main


stack_avail: .byte 1

.align 4
gdt32:
  .quad 0x0000000000000000          # Null Descriptor - should be present.
  .quad 0xffff0000009acf00          # 32-bit code descriptor (exec/read).
  .quad 0xffff00000092cf00          # 32-bit data descriptor (read/write)
gdt32_end:

.align 4
gdt32_pointer:
  .word gdt32_end - gdt32 - 1  # 16-bit Size (Limit) of GDT.
  .long gdt32                  # 32-bit Base Address of GDT. (CPU will zero extend to 64-bit)
