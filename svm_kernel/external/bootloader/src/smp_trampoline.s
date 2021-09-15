.section .smp_trampoline, "awx"
.intel_syntax noprefix
.global _smp_trampoline
.global stack_avail
.global undef_instr

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
  lgdt gdt_64_pointer

  # Enable protected mode
  mov eax, cr0
  or  eax, (1 << 0)
  mov cr0, eax

  ljmp 0x18, offset protected_mode_setup

.code32
protected_mode_setup:
  mov bx, 0x10
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
  mov esp, offset __stack_start
  call smp_main

spin:
  jmp spin

undef_instr:
  .long 0xffffffff

.align 2
stack_avail: .byte 1
