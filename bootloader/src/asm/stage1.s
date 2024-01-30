.section .first_stage, "awx"
.global stage1_start
.code16


stage1_start:
    cld # clear direction flag

    # initialize segment registers
    xor ax, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ds, ax
    mov ss, ax

    mov sp, 0x7C00 # initialize the stack

    mov [drive_code], dl # stores drive code

    mov si, offset first_stage_string
    call bios_println

set_bios_long_mode:
    mov ax, 0xEC00
    mov bx, 2
    int 0x15

check_int13h_extensions:
    mov ah, 0x41
    mov bx, 0x55AA
    int 0x13
    jc error_int13h_extensions_not_supported

load_rest_of_bootloader:
    mov bx, 0x7E00 # 0x07C0:0x0200 (512 bytes from start)
    mov word ptr [dap_transfer_buffer], bx

    mov ax, offset end_addr_bootloader
    sub ax, bx # end - start
    add ax, 511 # align up
    shr ax, 9 # divide by 512
    mov word ptr [dap_num_sectors], ax
    mov word ptr [dap_starting_lba], 1 # FIXME: In case MBR chainloading

    mov si, offset dap
    mov ah, 0x42
    mov dl, [drive_code] # restore drive code
    int 0x13
    jc error_load_rest_of_bootloader

    jmp stage2_start


# print routines:
# string in si
bios_println:
    call bios_print
    mov al, 0xA # line Feed
    call bios_write_character
    mov al, 0xD # carriage Return
    call bios_write_character
    ret

# string in si
bios_print:
    cld
bios_print_loop:
    lodsb
    test al, al
    jz bios_print_return
    call bios_write_character
    jmp bios_print_loop
bios_print_return:
    ret

# char in al
bios_write_character:
    mov ah, 0xE
    int 0x10
    ret


# errors routines:
# string in bx
error_print:
    mov si, offset error_string
    call bios_print
    mov si, bx
    call bios_println
spin:
    jmp spin

error_cpuid_not_supported:
    mov bx, offset error_cpuid_not_supported_string
    jmp error_print
error_longmode_not_supported:
    mov bx, offset error_longmode_not_supported_string
    jmp error_print
error_int13h_extensions_not_supported:
    mov bx, offset error_int13h_extensions_string
    jmp error_print
error_load_rest_of_bootloader:
    mov bx, offset error_load_rest_of_bootloader_string
    jmp error_print


# data structures:
# global descriptor table descriptor
gdt_descriptor:
    .word gdt_end - gdt - 1 # size of gdt
    .long gdt               # start of gdt
# global descriptor table
gdt:
    .quad 0
# code descriptor
    .byte 0xff
    .byte 0xff
    .byte 0
    .byte 0
    .byte 0
    .byte 0x9a
    .byte 0xcf
    .byte 0
# data descriptor
    .byte 0xff
    .byte 0xff
    .byte 0
    .byte 0
    .byte 0
    .byte 0x92
    .byte 0xcf
    .byte 0
gdt_end:

# disk address packet
dap:
    .byte 0x10  # size of packet
    .byte 0x0   # unused
dap_num_sectors:
    .word 0x0   # number of sectors to transfer
dap_transfer_buffer: # segment:offset
    .word 0x0   # offset
    .word 0x0   # segment
dap_starting_lba:
    .quad 0x0

# store drive code from dl
drive_code: .byte 0


# strings:
first_stage_string: .asciz "Booting first stage..."
error_string: .asciz "ERROR: "
error_cpuid_not_supported_string: .asciz "CPUID not supported by CPU."
error_longmode_not_supported_string: .asciz "Long Mode not supported by CPU."
error_int13h_extensions_string: .asciz "INT13h Extensions not supported."
error_load_rest_of_bootloader_string: .asciz "Failed to load rest of bootloader."


# fill to 512 bytes
.org 510
.word 0xAA55 # bootable disk signature
