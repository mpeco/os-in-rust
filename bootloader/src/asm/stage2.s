.section .second_stage, "awx"
.code16


stage2_start:
    mov si, offset second_stage_string
    call bios_println

a20_line_enable:
    call check_a20_line
    test ax, ax
    jnz a20_line_enabled

    call a20_line_kcontroller_enable
    call check_a20_line
    test ax, ax
    jnz a20_line_enabled

    call a20_line_bios_enable
    call check_a20_line
    test ax, ax
    jnz a20_line_enabled

    call a20_line_fast_enable
    call check_a20_line
    test ax, ax
    jnz a20_line_enabled

    jmp error_enable_a20

a20_line_enabled:
    nop

get_memory_map:
    xor bp, bp # used to keep count of entries
    xor ebx, ebx
    mov di, offset memory_map+0x4 #0x9000+0x4 = 0x9004
    mov edx, 0x534D4150
    mov eax, 0xE820
    mov ecx, 24
    mov dword ptr es:[di+20], 1 # force ACPI 3.x entry
    int 0x15
    jc error_memory_map # function not supported

    mov edx, 0x534D4150 # register may be trashed?
    cmp eax, edx
    jne error_memory_map

    test ebx, ebx
    jz error_memory_map
    jmp mm_handle_entry

mm_get_entry:
    mov eax, 0xE820
    mov ecx, 24
    mov dword ptr es:[di+20], 1 # force ACPI 3.x entry
    int 0x15
    jc mm_finish # if carry is set list finished
    mov edx, 0x534D4150 # register may be trashed?

mm_handle_entry:
    jcxz mm_skip_entry # cx = 0 means 0 length entry
    cmp cx, 20
    jbe mm_handle_entry2
    test dword ptr es:[di+20], 1
    jz mm_skip_entry # if ignore bit is clear skip entry

mm_handle_entry2:
    mov eax, es:[di+8]
    or eax, es:[di+12]
    jz mm_skip_entry # if length of entry is 0 skip it
    inc bp
    add di, 24

mm_skip_entry:
    test ebx, ebx
    jnz mm_get_entry # if ebx is 0 end of list

mm_finish:
    mov es:[offset memory_map], bp # store entry count at start
    clc # clear carry flag

vesa:
vesa_get_info:
    mov ax, 0x4F00
    mov di, offset vbe_info_structure
    int 0x10
    cmp ax, 0x4F
    jne error_vbe # VBE not supported
    mov si, [vbe_info_structure_video_mode_ptr] # offset
    mov ax, [vbe_info_structure_video_mode_ptr+2] # segment
    mov fs, ax
    sub si, 2
vesa_search_mode:
    add si, 2
    mov cx, fs:[si]
    cmp cx, 0xFFFF
    je error_vbe_mode_not_found
vesa_get_mode_info:
    push esi
    mov ax, 0x4F01
    mov di, offset vbe_mode_info_structure
    int 0x10
    pop esi
    cmp ax, 0x4F
    jne error_vbe
vesa_check_mode:
    # check if bpp of mode is 24 FIXME: make configurable
    cmp byte ptr [vbe_mode_info_structure_bpp], 24
    jne vesa_search_mode
    # check if width of mode is 800 FIXME: make configurable
    cmp byte ptr [vbe_mode_info_structure_width], 800
    jne vesa_search_mode
    # check if height of mode is 600 FIXME: make configurable
    cmp byte ptr [vbe_mode_info_structure_height], 600
    jne vesa_search_mode
    # check memory model is direct color
    cmp byte ptr [vbe_mode_info_structure_memory_model], 6
    jne vesa_search_mode
    # check if linear framebuffer bit is set
    mov ax, [vbe_mode_info_structure_attributes]
    test ax, 0x80
    jz vesa_search_mode
vesa_set_mode:
    mov bx, cx
    or bx, 0x4000 # enable linear framebuffer
    mov ax, 0x4F02
    int 0x10
    cmp ax, 0x4F
    jne error_vbe

get_vga_bitmap_font:
    # save segment registers
    push ds
    push es

    # address returned in es:bp
    mov ax, 0x1130
    mov bh, 0x6
    int 0x10

    # set ds:si to bitmap fonts address
    push es
    pop ds
    mov si, bp
    # set es:di to buffer
    pop es # restore es
    mov di, offset vga_bitmap_font
    # move dword from ds:si to es:di 1024 times (4k bytes)
    mov cx, 0x400
    rep movsd

    # restore ds
    pop ds

enter_unreal_mode:
    cli
    push es
    lgdt [gdt_descriptor] # load global descriptor table

    # enter protected mode
    mov eax, cr0
    or al, 1
    mov cr0, eax

    # load descriptor 2 on es
    mov bx, 0x10
    mov es, bx

    # leave protected mode
    and al, 0xFE
    mov cr0, eax

    pop es
    sti

load_kernel:
    mov ax, offset kernel_loading_buffer
    mov word ptr [dap_transfer_buffer], ax

    mov ax, offset _binary_kernel_elf_start
    mov bx, offset stage1_start
    sub ax, bx
    shr ax, 9 # divide by 512
    mov word ptr [dap_starting_lba], ax
    mov word ptr [dap_num_sectors], 1

    mov edi, offset kernel_addr

    # number of sectors in kernel
    mov ecx, offset _binary_kernel_elf_size
    add ecx, 511 # align up
    shr ecx, 9 # divide by 512

load_kernel_sector:
    mov si, offset dap
    mov ah, 0x42
    mov dl, [drive_code] # restore drive code
    int 0x13

    # move from buffer to destination
    push ecx
    mov esi, offset kernel_loading_buffer
    mov ecx, 512/4
    rep movsd [edi], [esi]
    pop ecx

    # next iteration
    mov ax, [dap_starting_lba]
    inc ax
    mov [dap_starting_lba], ax
    dec ecx
    jnz load_kernel_sector

enter_protected_mode:
    cli
    lgdt [gdt_descriptor] # load global descriptor table

    # enter protected mode
    mov eax, cr0
    or al, 1
    mov cr0, eax

    # load descriptor 2 in segment registers
    mov bx, 0x10
    mov ds, bx
    mov es, bx
    mov ss, bx
    # loads CS with 0x8 (descriptor 1 in GDT) and jumps to stage 3
    ljmp 0x8, offset stage3_start


# a20 line routines:
# checks if a20 line is enabled, returns 1 on ax if so or 0 otherwise
check_a20_line:
    push es
    mov ax, 0xFFFF
    mov es, ax

    mov si, 0x500
    mov di, 0x510

    mov al, byte ptr ds:[si]
    push ax
    mov al, byte ptr es:[di]
    push ax

    mov byte ptr ds:[si], 0x00
    mov byte ptr es:[di], 0xFF
    cmp byte ptr ds:[si], 0xFF

    pop ax
    mov byte ptr es:[di], al
    pop ax
    mov byte ptr ds:[si], al

    mov ax, 0
    je check_a20_line_disabled # if equal wrap around happened which means a20 line is disabled
check_a20_line_enabled:
    mov ax, 1
check_a20_line_disabled:
    pop es
    ret

a20_line_bios_enable:
    mov ax, 0x2403 # check if method is supported
    int 0x15
    jc a20_line_bios_enable_ret
    test ah, ah
    jnz a20_line_bios_enable_ret
    mov ax, 0x2401 # try to enable
    int 0x15
a20_line_bios_enable_ret:
    ret

a20_line_kcontroller_enable:
    call a20_line_kcontroller_wait
    mov al, 0xD1
    out 0x64, al
    call a20_line_kcontroller_wait
    mov al, 0xDF
    out 0x60, al
    call a20_line_kcontroller_wait
    ret
a20_line_kcontroller_wait:
    in al, 0x64
    test al, 0x2
    jnz a20_line_kcontroller_wait
    ret

a20_line_fast_enable:
    in al, 0x92
    test al, 0x2 # check if bit 1 is already on
    jnz a20_line_fast_enable_ret
    or al, 0x2
    and al, 0xFE # guarantee bit 0 isn't written to (resets)
    out 0x92, al
a20_line_fast_enable_ret:
    ret


# errors routines:
error_enable_a20:
    mov bx, offset error_enable_a20_string
    jmp error_print
error_memory_map:
    mov bx, offset error_memory_map_string
    jmp error_print
error_vbe:
    mov bx, offset error_vbe_string
    jmp error_print
error_vbe_mode_not_found:
    mov bx, offset error_vbe_mode_not_found_string
    jmp error_print



# data structures:
# VESA info structure
vbe_info_structure:
    vbe_info_structure_signature: .ascii "VBE2" # will later be changed to "VESA" once filled
    vbe_info_structure_version: .word 0
    vbe_info_structure_oem_str_ptr: .long 0 #.word 0 .word 0 # offset and segment
    vbe_info_structure_capabilities: .long 0
    vbe_info_structure_video_mode_ptr: .long 0 #.word 0 .word 0 # offset and segment
    vbe_info_structure_software_rev: .word 0
    vbe_info_structure_vendor: .long 0
    vbe_info_structure_product_name: .long 0
    vbe_info_structure_product_rev: .long 0
    vbe_info_structure_reserved: .skip 222, 0
    vbe_info_structure_oem_data: .skip 256, 0

# VESA mode info structure
vbe_mode_info_structure:
    vbe_mode_info_structure_attributes: .word 0
    .skip 16, 0 # not used here
    vbe_mode_info_structure_width: .word 0
    vbe_mode_info_structure_height: .word 0
    .skip 3, 0 # not used here
    vbe_mode_info_structure_bpp: .byte 0
    .skip 1, 0
    vbe_mode_info_structure_memory_model: .byte 0
    .skip 12, 0 # not used here
    vbe_mode_info_structure_framebuffer_addr: .long 0
    .skip 212, 0 # not used here


# strings:
second_stage_string: .asciz "Booting second stage..."
error_enable_a20_string: .asciz "Failed to enable A20 line."
error_memory_map_string: .asciz "Failed to get memory map."
error_vbe_string: .asciz "VESA function not supported/failed."
error_vbe_mode_not_found_string: .asciz "VESA video mode specified in config not found."
