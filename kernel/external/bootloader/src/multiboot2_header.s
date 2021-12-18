.section .multiboot_header, "ad"

header_start:
.long 0xE85250D6 # magic number (multiboot 2)
.long 0          # architecture 0 (protected mode i386)
.long header_end - header_start # header length
# checksum
.long 0x100000000 - (0xE85250D6 + 0 + (header_end - header_start))

# required end tag
.word 0
.word 0
.long 8 # size

header_end:

