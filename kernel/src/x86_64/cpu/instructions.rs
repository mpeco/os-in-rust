use core::arch::asm;


pub struct CpuidRegs {
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32
}
pub fn cpuid(function: u32) -> CpuidRegs {
    let (eax, ebx, ecx, edx): (u32, u32, u32, u32);
    unsafe {
        asm!(
            "push rbx",
            "mov eax, {0:e}",
            "xor rbx, rbx",
            "cpuid",
            "mov r8, rbx",
            "pop rbx",
            in(reg) function,
            out("eax") eax,
            out("r8") ebx,
            out("ecx") ecx,
            out("edx") edx
        );
    }
    CpuidRegs { eax, ebx, ecx, edx }
}

#[inline]
pub fn wrmsr(ecx: u32, edx: u32, eax: u32) {
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") ecx,
            in("edx") edx,
            in("eax") eax
        );
    }
}
#[inline]
pub fn rdmsr(ecx: u32) -> (u32, u32) {
    let (eax, edx): (u32, u32);
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") ecx,
            out("edx") edx,
            out("eax") eax
        );
    }
    (edx, eax)
}

#[inline]
pub fn mfence() {
    unsafe {
        asm!("mfence");
    }
}

#[inline]
pub fn inb(port: u16) -> u8 {
    let ret: u16;
    unsafe {
        asm!(
            "xor ax, ax",
            "in al, dx",
            out("ax") ret,
            in("dx") port
        );
    }
    ret as u8
}
#[inline]
pub fn inw(port: u16) -> u16 {
    let ret: u16;
    unsafe {
        asm!(
            "in ax, dx",
            out("ax") ret,
            in("dx") port
        );
    }
    ret
}
#[inline]
pub fn inl(port: u16) -> u32 {
    let ret: u32;
    unsafe {
        asm!(
            "in eax, dx",
            out("eax") ret,
            in("dx") port
        );
    }
    ret
}
#[inline]
pub fn outb(port: u16, value: u8) {
    unsafe {
        asm!(
            "out dx, al",
            in("ax") value as u16,
            in("dx") port
        )
    }
    inb(port); // wait for completion
}
#[inline]
pub fn outw(port: u16, value: u16) {
    unsafe {
        asm!(
            "out dx, ax",
            in("ax") value,
            in("dx") port
        )
    }
    inw(port); // wait for completion
}
#[inline]
pub fn outl(port: u16, value: u32) {
    unsafe {
        asm!(
            "out dx, eax",
            in("eax") value,
            in("dx") port
        )
    }
    inl(port); // wait for completion
}

#[inline]
pub fn hlt() { unsafe { asm!("hlt"); } }

// set interrupt flag
#[inline]
pub fn sti() { unsafe { asm!("sti"); } }
// clear interrupt flag
#[inline]
pub fn cli() { unsafe { asm!("cli"); } }
// sti and hlt one after the other, since sti only enables interrupts
// after the next instruction no interrupts can be fired inbetween the instructions
#[inline]
pub fn sti_hlt() { unsafe { asm!("sti", "hlt"); } }

// breakpoint interrupt
#[inline]
pub fn int3() { unsafe { asm!("int3"); } }

// loads gdt descriptor stored at address
#[inline]
pub fn lgdt(address: u64) {
    unsafe {
        asm!(
            "lgdt [{}]",
            in(reg) address
        );
    }
}
// stores gdt descriptor at address
#[inline]
pub fn sgdt(address: u64) {
    unsafe {
        asm!(
            "sgdt [{}]",
            in(reg) address
        );
    }
}

// loads tss stored at segment in gdt
#[inline]
pub fn ltr(segment: u16) {
    unsafe {
        asm!(
            "ltr {0:x}",
            in(reg) segment
        );
    }
}

// loads idt descriptor stored at address
#[inline]
pub fn lidt(address: u64) {
    unsafe {
        asm!(
            "lidt [{}]",
            in(reg) address
        );
    }
}
