use core::sync::atomic::{AtomicBool, Ordering};

pub struct InitOnce(AtomicBool);
impl InitOnce {
    pub const fn new() -> InitOnce {
        InitOnce(AtomicBool::new(false))
    }

    pub fn init(&self) -> Result<(), ()>{
        let is_init = self.0.load(Ordering::Acquire);
        if is_init == true {
            return Err(());
        }
        if let Err(_) = self.0.compare_exchange_weak(
            is_init, true, Ordering::AcqRel, Ordering::Acquire
        )
        {
            return Err(());
        }

        Ok(())
    }

    pub fn is_init(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
