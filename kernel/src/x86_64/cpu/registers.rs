pub mod rcx {
    use core::arch::asm;

    pub fn read() -> u64 {
        let value: u64;
        unsafe {
            asm!(
                "mov {}, rcx",
                out(reg) value
            );
        }
        value
    }
    pub fn write(value: u64) {
        unsafe {
            asm!(
                "mov rcx, {}",
                in(reg) value
            );
        }
    }
}
pub mod rdi {
    use core::arch::asm;

    pub fn read() -> u64 {
        let value: u64;
        unsafe {
            asm!(
                "mov {}, rdi",
                out(reg) value
            );
        }
        value
    }
    pub fn write(value: u64) {
        unsafe {
            asm!(
                "mov rdi, {}",
                in(reg) value
            );
        }
    }
}
pub mod r10 {
    use core::arch::asm;

    pub fn read() -> u64 {
        let value: u64;
        unsafe {
            asm!(
                "mov {}, r10",
                out(reg) value
            );
        }
        value
    }
    pub fn write(value: u64) {
        unsafe {
            asm!(
                "mov r10, {}",
                in(reg) value
            );
        }
    }
}

pub mod rsp {
    use core::arch::asm;

    pub fn read() -> u64 {
        let value: u64;
        unsafe {
            asm!(
                "mov {}, rsp",
                out(reg) value
            );
        }
        value
    }
    pub fn write(value: u64) {
        unsafe {
            asm!(
                "mov rsp, {}",
                in(reg) value
            );
        }
    }
}
pub mod rbp {
    use core::arch::asm;

    pub fn read() -> u64 {
        let value: u64;
        unsafe {
            asm!(
                "mov {}, rbp",
                out(reg) value
            );
        }
        value
    }
    pub fn write(value: u64) {
        unsafe {
            asm!(
                "mov rbp, {}",
                in(reg) value
            );
        }
    }
}

pub mod cs {
    use core::arch::asm;

    pub fn read() -> u16 {
        let value: u16;
        unsafe {
            asm!(
                "mov {0:x}, cs",
                out(reg) value
            );
        }
        value
    }
}
pub mod ss {
    use core::arch::asm;

    pub fn read() -> u16 {
        let value: u16;
        unsafe {
            asm!(
                "mov {0:x}, ss",
                out(reg) value
            );
        }
        value
    }
}

pub mod rflags {
    use core::arch::asm;

    pub const FLAG_INTERRUPT_ENABLED: u64 = 1<<9;

    pub fn read() -> u64 {
        let value: u64;
        unsafe {
            asm!(
                "pushfq",
                "pop {}",
                out(reg) value
            );
        }
        value
    }

    pub fn is_flag_enabled(flag: u64) -> bool {
        read() & flag != 0
    }
}

pub mod cr2 {
    use core::arch::asm;

    pub fn read() -> u64 {
        let value: u64;
        unsafe {
            asm!(
                "mov {}, cr2",
                out(reg) value
            );
        }
        value
    }
}

pub mod cr3 {
    use core::arch::asm;

    pub fn read() -> u64 {
        let value: u64;
        unsafe {
            asm!(
                "mov {}, cr3",
                out(reg) value
            );
        }
        value
    }
    pub fn write(value: u64) {
        unsafe {
            asm!(
                "mov cr3, {}",
                in(reg) value
            );
        }
    }
    pub fn flush_tlb() {
        unsafe {
            asm!(
                "mov rax, cr3",
                "mov cr3, rax"
            );
        }
    }
}

pub mod cr8 {
    use core::arch::asm;

    pub fn write(value: u64) {
        unsafe {
            asm!(
                "mov cr8, {}",
                in(reg) value
            );
        }
    }
}
