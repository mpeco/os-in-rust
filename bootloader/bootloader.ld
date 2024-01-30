ENTRY(stage1_start)

SECTIONS {
    /* start of conventional memory */
    . = 0x500;
    kernel_loading_buffer = .;

    /* where bootloader is loaded */
    . = 0x7C00;

    /* first_stage */
    .first_stage : { *(.first_stage) }
    /* second stage */
    .second_stage : { *(.second_stage) }
    /* third stage */
    .third_stage : {
        *(.text .text.*)
        *(.data .data.*)
        *(.rodata .rodata.*)
        *(.bss .bss.*)
        *(.eh_frame_hdr)
        *(.eh_frame)
    }
    .lm_jump : { *(.lm_jump) }
    end_addr_bootloader = .;

    . = ALIGN(0x1000);
    memory_map = .;
    . += 0x1000;
    vga_bitmap_font = .;
    . += 0x1000;
    pml4t_address = .;
    . += 0x1000;
    pdpt_address = .;
    . += 0x1000;
    pdt_address = .;
    . += 0x1000;
    pt_address = .;
    . += 0x1000;
    k_pdpt_address = .;
    . += 0x1000;
    k_pdt_address = .;
    . += 0x1000;
    k_pt_address = .;
    . += 0x1000;
    conventional_mem_addr = .;

    . = end_addr_bootloader;
    . = ALIGN(512);
    start_addr_kernel = .;
    .kernel : { KEEP(*(.kernel)) }
    end_addr_kernel = .;

    . = 0x100000;
    kernel_addr = .;
}
