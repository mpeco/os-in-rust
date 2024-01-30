# will be loaded on address 0x8000

.code16
trampoline_start:
    cli
    cld
    xor ax, ax
    mov ds, ax

    # load gloal descriptor table
    mov byte ptr [0x8034], 0xCF
    lgdt [0x8020]

    # enter protected mode
    mov eax, cr0
    or al, 1
    mov cr0, eax

    # jump to protected mode
    ljmp 0x8, 0x8040

.org 0x20 # fill until 0x8020
# global descriptor table descriptor
gdt_descriptor_0x8020:
    .word gdt_end - gdt - 1 # size of gdt
    .long 0x8026            # start of gdt
.org 0x26 # 0x8026, directive to make sure address is correct when assembling
# global descriptor table
gdt:
    .quad 0
# code descriptor
    .byte 0xFF
    .byte 0xFF
    .byte 0
    .byte 0
    .byte 0
    .byte 0x9A
.org 0x34 # 0x8034, directive to make sure address is correct when assembling
    .byte 0
    .byte 0
# data descriptor
    .byte 0xFF
    .byte 0xFF
    .byte 0
    .byte 0
    .byte 0
    .byte 0x92
    .byte 0xCF
    .byte 0
gdt_end:

.code32
.org 0x40 # fill until 0x8040
protected_mode_0x8040:
    # move PML4 table address to cr3
    mov eax, [0x8080]
    mov cr3, eax
    # flip PAE bit in cr4
    mov eax, cr4
    or eax, 0x20
    mov cr4, eax

    # set long mode and NXE bit
    mov ecx, 0xC0000080
    rdmsr
    or eax, 0x900
    wrmsr

    # enable paging
    mov eax, cr0
    or eax, 0x80000000
    mov cr0, eax

    # update gdt for long mode and load it again
    mov byte ptr [0x8034], 0xAF
    lgdt [0x8020]

    # jump to long mode
    ljmp 0x8, 0x8100

.org 0x80 # fill until 0x8080
pml4_addr_0x8080:            # will be filled by kernel
    .quad 0x0
init_ap_fn_addr_0x8088:      # will be filled by kernel
    .quad 0x0
stack_top_addr_ptr_0x8090:   # will be filled by kernel
    .quad 0x0
trampoline_lock_addr_0x8098: # will be filled by kernel
    .quad 0x0

.code64
.org 0x100 # fill until 0x8100
long_mode_0x8100:
    mov rax, [0x8098]
trampoline_spin_loop: # wait for bsp
    pause
    cmp byte ptr [rax], 0
    jnz trampoline_spin_loop

    # setup stack
    mov rax, [0x8090]
    mov rsp, [rax]

    # jump to "init_ap" in smp/mod.rs with stack top addr as 1st param (rdi)
    mov rdi, rsp
    mov rax, [0x8088]
    jmp rax

trampoline_end:
