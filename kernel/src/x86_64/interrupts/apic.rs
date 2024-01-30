use crate::x86_64::{structures::acpi::madt::MADT, cpu};


const PIC1_DATA: u16 = 0x21;
const PIC2_DATA: u16 = 0xA1;

// uses cpuid to determine whether cpu supports apic
fn supports_apic() -> bool {
    let cpuid_regs = cpu::instructions::cpuid(1);
    cpuid_regs.edx & 0x200 != 0
}

pub fn init_apic(madt: &'static MADT) -> Result<(), &'static str> {
    if !supports_apic() {
        return Err("APIC not supported by CPU.");
    }

    // disable PIC
    cpu::instructions::outb(PIC1_DATA, 0xFF);
    cpu::instructions::outb(PIC2_DATA, 0xFF);

    lapic::init_base_addr(madt.get_lapic_addr());
    io_apic::init(madt)?;

    Ok(())
}


pub mod lapic {
    use crate::{
        def_interrupt_handler,
        x86_64::{self, cpu, structures::idt::{Index, Flags}},
        utils::lazy_static::LazyStatic, memory::address::PhysAddr,
    };


    #[derive(Clone, Copy)]
    enum TimerMode {
        Disabled,
        OneShot,
        Periodic,
        TSCDeadline,
    }


    const LAPIC_ID_OFFSET: usize = 0x20;
    const EOI_OFFSET: usize = 0xB0;
    const ICR_OFFSET1: usize = 0x300;
    const ICR_OFFSET2: usize = 0x310;

    const ICR_OFFSET1_BITMASK: u32 = 0b11111111_11110011_00100000_00000000;
    const ICR_OFFSET2_BITMASK: u32 = 0b00000000_11111111_11111111_11111111;
    const ICR_FIXED_BITMASK: u32 = !(0b111<<8);
    const ICR_INIT_BITS: u32 = 0b101<<8;
    const ICR_STARTUP_BITS: u32 = 0b110<<8;
    const ICR_ASSERT_BIT: u32 = 1<<14;
    const ICR_DELIVERY_STATUS_PENDING_BIT: u32 = 1<<12;
    const ICR_DESTINATION_BROADCAST_EXCLUDING_SELF_BITS: u32 = 0b11<<18;


    static BASE_ADDR: LazyStatic<PhysAddr> = LazyStatic::new();


    pub fn init_base_addr(base_addr: PhysAddr) {
        BASE_ADDR.init(base_addr);
    }

    pub fn get_id() -> u32 {
        read(LAPIC_ID_OFFSET) >> 24 // id stored in the highest 8 bitsS
    }

    // Sends IPI to all LAPICS excluding self
    pub fn broadcast_ipi(vector: u8) {
        let value_with_vec = (read(ICR_OFFSET1) & ICR_OFFSET1_BITMASK)
            & ICR_FIXED_BITMASK | ICR_DESTINATION_BROADCAST_EXCLUDING_SELF_BITS | vector as u32;
        write(ICR_OFFSET1, value_with_vec);
        wait_for_ipi_delivery();
    }

    pub fn send_init_ipi(receiver_lapic_id: u32) {
        write_id_to_icr(receiver_lapic_id);

        // assert init IPI
        let value_with_init = (read(ICR_OFFSET1) & ICR_OFFSET1_BITMASK) | ICR_INIT_BITS | ICR_ASSERT_BIT;
        write(ICR_OFFSET1, value_with_init);
        wait_for_ipi_delivery();

        // deassert init IPI
        let value_with_deassert = (read(ICR_OFFSET1) & ICR_OFFSET1_BITMASK) | ICR_INIT_BITS & !ICR_ASSERT_BIT;
        write(ICR_OFFSET1, value_with_deassert);
        wait_for_ipi_delivery();
    }

    pub fn send_startup_ipi(receiver_lapic_id: u32, address: u32) {
        write_id_to_icr(receiver_lapic_id);

        let startup_flags: u32 = ICR_STARTUP_BITS | (address/0x1000);
        let value_with_startup = (read(ICR_OFFSET1) & ICR_OFFSET1_BITMASK) | startup_flags;
        write(ICR_OFFSET1, value_with_startup);
        wait_for_ipi_delivery();
    }

    fn write_id_to_icr(receiver_lapic_id: u32) {
        let value_with_id = (read(ICR_OFFSET2) & ICR_OFFSET2_BITMASK) | (receiver_lapic_id << 24);
        write(ICR_OFFSET2, value_with_id);
    }

    fn wait_for_ipi_delivery() {
        while read(ICR_OFFSET1) & ICR_DELIVERY_STATUS_PENDING_BIT != 0 {
            core::hint::spin_loop();
        }
    }

    #[inline]
    pub fn eoi() {
        write(EOI_OFFSET, 0xdeadbeef);
    }

    #[inline]
    pub fn write(offset: usize, value: u32) {
        assert!(BASE_ADDR.is_init(), "Attempted to write to LAPIC before initializing base address");
        let ptr = BASE_ADDR.offset::<u8>(offset).to_mut_virtual().as_ptr::<u32>();
        unsafe { ptr.write_volatile(value); }
    }
    #[inline]
    pub fn read(offset: usize) -> u32 {
        assert!(BASE_ADDR.is_init(), "Attempted to write to LAPIC before initializing base address");
        let ptr = BASE_ADDR.offset::<u8>(offset).to_mut_virtual().as_ptr::<u32>();
        unsafe { ptr.read_volatile() }
    }

    def_interrupt_handler!(spurious_handler,
        fn spurious_handler_fn(_stack_frame: &StackFrame) {
            x86_64::interrupts::apic::lapic::eoi();
        }
    );


    pub struct Lapic {
        is_enabled: bool,
        is_timer_setup: bool,
        timer_ticks_per_ms: u32,
        is_timer_tsc_mode_supported: bool,
        tsc_cycles_per_ms: u64,
        timer_mode: TimerMode,
    }
    impl Lapic {
        const APIC_MSR_INDEX: u32 = 0x1B;
        const APIC_MSR_ENABLE_BIT: u32 = 1<<11;
        const APIC_MSR_X2APIC_MODE_BIT: u32 = 1<<10;

        const SIVR_OFFSET: usize = 0xF0;
        const LVT_TIMER_OFFSET: usize = 0x320;
        const INITIAL_COUNT_OFFSET: usize = 0x380;
        const CURRENT_COUNT_OFFSET: usize = 0x390;
        const DIVISOR_CONFIG_OFFSET: usize = 0x3E0;
        const SIVR_VALUE: u32 = (1<<8) | Index::SPURIOUS as u32;
        const MASK_BIT: u32 = 1<<16;

        const TIMER_CLEAR_MODE_BITMASK: u32 = !(0b11<<17);
        const TIMER_PERIODIC_MODE_BIT: u32 = 1<<17;
        const TIMER_TSC_DEADLINE_MODE_BIT: u32 = 1<<18;
        const TIMER_TSC_DEADLINE_MSR_ADDR: u32 = 0x6E0;
        const TIMER_DIVISOR: u32 = 0x3; // 16

        pub fn new() -> Lapic {
            Lapic {
                is_enabled: false, is_timer_setup: false, timer_ticks_per_ms: 0,
                is_timer_tsc_mode_supported: false, tsc_cycles_per_ms: 0, timer_mode: TimerMode::Disabled
            }
        }

        pub fn enable(&mut self) {
            assert!(BASE_ADDR.is_init(), "Attempted to use LAPIC before initializing base address");

            // set spurious interrupt handler
            x86_64::interrupts::set_idt_entry(
                Index::SPURIOUS, spurious_handler.get_addr(), 0x8, Flags::BASE, 0
            );

            x86_64::interrupts::set_task_priority_level(0);

            // make sure the APIC is enabled and not in x2APIC mode (not implemented yet)
            let (edx, mut eax) = cpu::instructions::rdmsr(Self::APIC_MSR_INDEX);
            eax |= Self::APIC_MSR_ENABLE_BIT;
            eax &= !Self::APIC_MSR_X2APIC_MODE_BIT;
            cpu::instructions::wrmsr(Self::APIC_MSR_INDEX, edx, eax);

            // enable APIC and set spurious interrupt vector
            write(Self::SIVR_OFFSET, Self::SIVR_VALUE);

            self.is_enabled = true;
        }

        pub fn setup_timer(&mut self, interrupt_vector: u8) {
            use crate::x86_64::{interrupts, pit, cpu::tsc};

            assert!(self.is_enabled, "Attempted to setup lapic timer before enabling it");
            assert!(self.is_timer_setup == false, "Attempt to setup lapic timer more than once");
            write(Self::DIVISOR_CONFIG_OFFSET, Self::TIMER_DIVISOR);

            // setup wait of 1ms
            let mut pit = pit::lock();
            pit.prepare_wait(1000);

            // set initial counter to -1
            write(Self::INITIAL_COUNT_OFFSET, 0xFFFFFFFF);
            pit.wait();
            // get number of ticks in 1ms
            self.timer_ticks_per_ms = 0xFFFFFFFF - read(Self::CURRENT_COUNT_OFFSET);

            if tsc::is_invariant_tsc_supported() {
                let tsc_start = tsc::rdtsc();
                pit.wait();
                let tsc_end = tsc::rdtsc();

                self.is_timer_tsc_mode_supported = true;
                self.tsc_cycles_per_ms = tsc_end - tsc_start;
            }

            pit::unlock(pit);

            // set apic timer interrupt vector and make sure its masked
            write(Self::LVT_TIMER_OFFSET, read(Self::LVT_TIMER_OFFSET) | Self::MASK_BIT | interrupt_vector as u32);
            write(Self::DIVISOR_CONFIG_OFFSET, 0x3);

            // remove temporary handler
            interrupts::remove_idt_entry(interrupt_vector);

            self.is_timer_setup = true;
        }

        pub fn get_timer_ticks_per_ms(&self) -> u32 {
            debug_assert!(self.is_timer_setup, "Attempted to retrieve timer ticks before calculating");
            self.timer_ticks_per_ms
        }
        pub fn get_tsc_cycles_per_ms(&self) -> u64 {
            debug_assert!(self.is_timer_setup, "Attempted to retrieve tsc cycles before calculating");
            self.tsc_cycles_per_ms
        }

        pub fn start_timer(&mut self, ticks_to_wait: u32, is_periodic: bool) {
            debug_assert!(self.is_timer_setup, "Attempted to start timer before setting it up");

            if is_periodic {
                write(Self::LVT_TIMER_OFFSET, read(Self::LVT_TIMER_OFFSET) & !Self::MASK_BIT & Self::TIMER_CLEAR_MODE_BITMASK | Self::TIMER_PERIODIC_MODE_BIT);
                self.timer_mode = TimerMode::Periodic;
            }
            else {
                write(Self::LVT_TIMER_OFFSET, read(Self::LVT_TIMER_OFFSET) & !Self::MASK_BIT & Self::TIMER_CLEAR_MODE_BITMASK);
                self.timer_mode = TimerMode::OneShot;
            }

            write(Self::INITIAL_COUNT_OFFSET, ticks_to_wait);
        }
        pub fn read_curr_timer_tick_count(&self) -> u32 {
            read(Self::CURRENT_COUNT_OFFSET)
        }
        pub fn stop_timer(&mut self) {
            debug_assert!(self.is_timer_setup, "Attempted to stop timer before setting it up");
            write(Self::INITIAL_COUNT_OFFSET, 0);
        }

        pub fn is_tsc_deadline_supported(&self) -> bool {
            self.is_timer_tsc_mode_supported
        }
        pub fn enable_tsc_deadline(&mut self) {
            debug_assert!(self.is_timer_setup, "Attempted to start timer before setting it up");
            debug_assert!(self.is_timer_tsc_mode_supported, "Attempted to enable timer in TSC mode but it's not supported");

            write(Self::LVT_TIMER_OFFSET, read(Self::LVT_TIMER_OFFSET) & !Self::MASK_BIT & Self::TIMER_CLEAR_MODE_BITMASK | Self::TIMER_TSC_DEADLINE_MODE_BIT);
            cpu::instructions::mfence(); // make sure the write to the LVT is ordered before any WRMSR
        }
        // returns the current tsc value used to calculate the deadline
        pub fn set_tsc_deadline(&mut self, cycles_to_wait: u64) -> u64 {
            debug_assert!(self.is_timer_setup, "Attempted to set TSC deadline before setting up the timer");
            debug_assert!(self.is_timer_tsc_mode_supported, "Attempted to set TSC deadline but it's not supported");

            let tsc = cpu::tsc::rdtsc();
            let tsc_deadline = tsc.saturating_add(cycles_to_wait);

            cpu::instructions::wrmsr(
                Self::TIMER_TSC_DEADLINE_MSR_ADDR, (tsc_deadline >> 32) as u32, tsc_deadline as u32
            );

            self.timer_mode = TimerMode::TSCDeadline;
            tsc
        }
        pub fn clear_tsc_deadline(&mut self) {
            debug_assert!(self.is_timer_setup, "Attempted to clear TSC deadline before setting up the timer");
            debug_assert!(self.is_timer_tsc_mode_supported, "Attempted to clear TSC deadline but it's not supported");
            cpu::instructions::wrmsr(Self::TIMER_TSC_DEADLINE_MSR_ADDR, 0, 0);
        }
    }
}


pub mod io_apic {
    use crate::{
        memory::address::PhysAddr, utils::lazy_static::LazyStatic,
        x86_64::structures::acpi::madt::MADT
    };
    use super::lapic;


    #[derive(Clone, Copy)]
    struct IsoFlags(u16);
    impl IsoFlags {
        fn to_io_apic_fields(&self) -> u64 {
            let mut ret = 0;
            if self.0 & 0b0011 != 0 { ret |= 0x2000; } // active low
            if self.0 & 0b1100 != 0 { ret |= 0x8000; } // level-triggered
            ret
        }
    }


    const _MASK_BIT: u64 = 1<<16;
    const IRQ_INDEX_BASE: u32 = 0x10;

    const SYSTEM_TIMER_IRQ_SOURCE: u8 = 0;
    const KEYBOARD_IRQ_SOURCE: u8 = 1;

    static BASE_ADDR: LazyStatic<PhysAddr> = LazyStatic::new();
    static mut SYSTEM_TIMER_INDEX: u32 = IRQ_INDEX_BASE + ((SYSTEM_TIMER_IRQ_SOURCE as u32)*2);
    static mut SYSTEM_TIMER_FLAGS: IsoFlags = IsoFlags(0);
    static mut KEYBOARD_INDEX: u32 = IRQ_INDEX_BASE + ((KEYBOARD_IRQ_SOURCE as u32)*2);
    static mut KEYBOARD_FLAGS: IsoFlags = IsoFlags(0);


    pub fn init(madt: &'static MADT) -> Result<(), &'static str> {
        unsafe {
            BASE_ADDR.init(madt.get_io_apic_addr_base_0()?);
            // update if interrupt source number has an override entry in the MADT
            if let Some(iso) = madt.get_interrupt_source_override(SYSTEM_TIMER_IRQ_SOURCE) {
                SYSTEM_TIMER_INDEX = IRQ_INDEX_BASE + (iso.global_system_interrupt*2);
                SYSTEM_TIMER_FLAGS = IsoFlags(iso.flags);
            }
            if let Some(iso) = madt.get_interrupt_source_override(KEYBOARD_IRQ_SOURCE) {
                KEYBOARD_INDEX = IRQ_INDEX_BASE + (iso.global_system_interrupt*2);
                KEYBOARD_FLAGS = IsoFlags(iso.flags);
            }
        }
        Ok(())
    }

    pub fn enable_system_timer(vector_number: u8) {
        let apic_id = (lapic::get_id() as u64) << 56;
        let sys_timer_index = unsafe { SYSTEM_TIMER_INDEX };
        let sys_timer_flags = unsafe { SYSTEM_TIMER_FLAGS };
        write(sys_timer_index, sys_timer_flags.to_io_apic_fields() | apic_id | vector_number as u64);
    }

    pub fn enable_keyboard(vector_number: u8) {
        let apic_id = (lapic::get_id() as u64) << 56;
        let kb_index = unsafe { KEYBOARD_INDEX };
        let kb_flags = unsafe { KEYBOARD_FLAGS };
        write(kb_index, kb_flags.to_io_apic_fields() | apic_id | vector_number as u64);
    }

    fn write(index: u32, value: u64) {
        let ioregsel = BASE_ADDR.to_mut_virtual().as_ptr::<u32>();
        let iowin = BASE_ADDR.to_mut_virtual().offset::<u8>(0x10).as_ptr::<u32>();
        unsafe {
            ioregsel.write_volatile(index);
            iowin.write_volatile(value as u32);
            ioregsel.write_volatile(index+1);
            iowin.write_volatile((value >> 32) as u32);
        }
    }

    fn _read(index: u32) -> u64 {
        let ioregsel = BASE_ADDR.to_mut_virtual().as_ptr::<u32>();
        let iowin = BASE_ADDR.to_virtual().offset::<u8>(0x10).as_ptr::<u32>();
        unsafe {
            ioregsel.write_volatile(index);
            let low_bytes = iowin.read_volatile() as u64;
            ioregsel.write_volatile(index+1);
            let high_bytes = (iowin.read_volatile() as u64) << 32;
            high_bytes | low_bytes
        }
    }
}
