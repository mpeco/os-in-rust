.section .lm_jump, "awx"
.global lm_jump
.code64

lm_jump:
    shl rbx, 32
    or rax, rbx

    # load data descriptor in segment registers
    mov dx, 0x10
    mov ds, dx
    mov es, dx
    mov fs, dx
    mov gs, dx
    mov ss, dx

    jmp rax
