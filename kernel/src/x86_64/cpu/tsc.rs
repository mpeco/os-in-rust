const CPUID_FUNC_GET_FEATURES: u32        = 1;
const CPUID_GET_FEATURES_ECX_TSC_BIT: u32 = 1 << 24;

const CPUID_FUNC_8_BASE: u32                   = 0x80000000;
const CPUID_FUNC_GET_CAPABILITIES: u32         = CPUID_FUNC_8_BASE | 0x7;
const CPUID_GET_CAPABILITIES_EDX_ITSC_BIT: u32 = 1 << 8;


pub fn is_invariant_tsc_supported() -> bool {
    use super::instructions::cpuid;

    let cpuid_regs = cpuid(CPUID_FUNC_GET_FEATURES);
    if cpuid_regs.ecx & CPUID_GET_FEATURES_ECX_TSC_BIT == 0 { return false; }

    let cpuid_regs = cpuid(CPUID_FUNC_8_BASE);
    if cpuid_regs.eax < CPUID_FUNC_GET_CAPABILITIES { return false; }

    let cpuid_regs = cpuid(CPUID_FUNC_GET_CAPABILITIES);
    if cpuid_regs.edx & CPUID_GET_CAPABILITIES_EDX_ITSC_BIT == 0 { return false; }

    true
}

#[inline]
pub fn rdtsc() -> u64 {
    let (high, low): (u64, u64);

    unsafe {
        core::arch::asm!(
            "rdtsc",
            out("rax") low,
            out("rdx") high,
        );
    }

    low | (high << 32)
}
