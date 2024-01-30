use core::{cmp::{self, Reverse}, sync::atomic::{AtomicBool, Ordering}};
use alloc::{collections::BinaryHeap, sync::Arc};

use crate::{
    def_interrupt_handler, processor, scheduler, secs,
    x86_64::{cpu::tsc, interrupts::{self, apic::lapic::Lapic}}
};
use super::Time;


const TIMER_DEFAULT_QUEUE_CAPACITY: usize = 50;
const TIMER_DEFAULT_FREQUENCY: Time = secs!(1);


// Halts execution for the duration of time_to_wait
pub fn wait(time_to_wait: Time) {
    processor::get().timer().wait(time_to_wait);
}

/**
 * Starts the timer that causes a preemptive schedule, if there was a timer active
 * and this is called before it has completed it will be reset.
 */
pub fn start_schedule_timer(time_to_wait: Time) {
    processor::get().timer().start_schedule_timer(time_to_wait);
}
pub fn stop_schedule_timer() {
    processor::get().timer().stop_schedule_timer();
}
// Adds an alarm that will cause a schedule after the duration of time_to_wait
pub fn add_schedule_alarm(time_to_wait: Time) {
    processor::get().timer().add_schedule_alarm(time_to_wait);
}


enum AlarmType {
    Wait { was_triggered: Arc<AtomicBool> },
    // Sleep    {  },
    Schedule
}
struct Alarm {
    trigger_runtime: Time,
    alarm_type: AlarmType
}
impl Alarm {
    fn new(trigger_runtime: Time, alarm_type: AlarmType) -> Alarm {
        Alarm { trigger_runtime, alarm_type }
    }

    fn notify(&self) {
        match &self.alarm_type {
            AlarmType::Wait { was_triggered } =>
                was_triggered.store(true, Ordering::Release),
            AlarmType::Schedule => {
                scheduler::schedule();
            }
        };
    }
}
impl PartialEq for Alarm {
    fn eq(&self, other: &Self) -> bool {
        self.trigger_runtime.eq(&other.trigger_runtime)
    }
}
impl Eq for Alarm {}
impl PartialOrd for Alarm {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.trigger_runtime.partial_cmp(&other.trigger_runtime)
    }
}
impl Ord for Alarm {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.trigger_runtime.cmp(&other.trigger_runtime)
    }
}


pub struct Timer {
    is_timer_init: bool,
    alarm_queue: BinaryHeap<Reverse<Alarm>>,
    runtime: Time,
    curr_frequency: Time,

    last_lapic_timer_tick_count: u32,

    is_using_tsc: bool,
    last_tsc_read: u64,

    schedule_alarm: Option<Alarm>,

    should_ignore_interrupt: bool,
    is_updating_queue: bool,

    ticks_per_ns: u64,
    ticks_per_us: u64,
    ticks_per_ms: u64,
    ticks_per_sec: u64
}
impl Timer {
    pub fn new() -> Timer {
        Timer {
            is_timer_init: false, alarm_queue: BinaryHeap::with_capacity(TIMER_DEFAULT_QUEUE_CAPACITY),
            runtime: secs!(0), curr_frequency: TIMER_DEFAULT_FREQUENCY, last_lapic_timer_tick_count: 0,
            schedule_alarm: None, is_using_tsc: false, last_tsc_read: 0,
            should_ignore_interrupt: false, is_updating_queue: false,
            ticks_per_sec: 0, ticks_per_ms: 0, ticks_per_us: 0, ticks_per_ns: 0
        }
    }

    pub fn init(&mut self) {
        use crate::x86_64::structures::idt::{Index, Flags};

        assert!(self.is_timer_init == false, "Attempted to initialize timer more than once");

        let lapic = processor::get().lapic();
        lapic.setup_timer(Index::LAPIC_TIMER);

        // set timer handler
        interrupts::set_idt_entry(
            Index::LAPIC_TIMER, timer_handler.get_addr(), 0x8, Flags::BASE, 0
        );

        let calc_ticks_per_time = |timer: &mut Timer| {
            timer.ticks_per_sec = timer.ticks_per_ms.saturating_mul(1000);
            timer.ticks_per_us = cmp::max(timer.ticks_per_ms/1000, 1);
            timer.ticks_per_ns = cmp::max(timer.ticks_per_us/1000, 1);
        };

        if lapic.is_tsc_deadline_supported() {
            self.is_using_tsc = true;
            self.ticks_per_ms = lapic.get_tsc_cycles_per_ms();
            calc_ticks_per_time(self);
            lapic.enable_tsc_deadline();
        }
        else {
            self.ticks_per_ms = lapic.get_timer_ticks_per_ms() as u64;
            calc_ticks_per_time(self);
        }
        self.start_timer(lapic, TIMER_DEFAULT_FREQUENCY);

        self.is_timer_init = true;
    }

    // Halts execution for the duration of time_to_wait
    pub fn wait(&mut self, time_to_wait: Time) {
        assert!(self.is_timer_init, "Attempted to use timer before initializing it");

        let was_triggered = Arc::new(AtomicBool::new(false));
        let alarm_type = AlarmType::Wait { was_triggered: was_triggered.clone() };
        self.add_to_queue(time_to_wait, alarm_type);

        interrupts::hlt_wait(|| was_triggered.load(Ordering::Acquire) );
    }

    /**
     * Starts the timer that causes a preemptive schedule, if there was a timer active
     * and this is called before it has completed it will be reset.
     */
    pub fn start_schedule_timer(&mut self, time_to_wait: Time) {
        /*
        * if this was called as result of an alarm triggered while we update
        * the queue we can simply update it
        */
        if self.is_updating_queue {
            let alarm = Alarm::new(self.runtime + time_to_wait, AlarmType::Schedule);
            self.schedule_alarm = Some(alarm);
        }
        else {
            self.disable_and_update_timer_run_then_reenable(|timer| {
                let alarm = Alarm::new(timer.runtime + time_to_wait, AlarmType::Schedule);
                timer.schedule_alarm = Some(alarm);
            });
        }
    }
    pub fn stop_schedule_timer(&mut self) {
        self.schedule_alarm = None;
    }
    // Adds an alarm that will cause a schedule call after the duration of time_to_wait
    pub fn add_schedule_alarm(&mut self, time_to_wait: Time) {
        self.add_to_queue(time_to_wait, AlarmType::Schedule);
    }

    // Adds an alarm to the queue
    fn add_to_queue(&mut self, time_to_wait: Time, alarm_type: AlarmType) {
        /*
         * if this was called as result of an alarm triggered while we update
         * the queue we can simply push it
         */
        if self.is_updating_queue {
            let alarm = Alarm::new(self.runtime + time_to_wait, alarm_type);
            self.alarm_queue.push(Reverse(alarm));
        }
        else {
            self.disable_and_update_timer_run_then_reenable(|timer| {
                let alarm = Alarm::new(timer.runtime + time_to_wait, alarm_type);
                timer.alarm_queue.push(Reverse(alarm));
            });
        }
    }

    /**
     * Disables and updates the timer runtime, runs closure and then restarts the timer.
     * This must be done for things such as adding a new alarm to the queue to make
     * sure it is based on the most updated runtime and so the timer can adjust
     * to a lower frequency if neccessary.
     */
    fn disable_and_update_timer_run_then_reenable<F>(&mut self, closure: F)
        where F: FnOnce(&mut Timer)
    {
        use crate::x86_64::interrupts::interrupts_disabled;

        let lapic = processor::get().lapic();

        // disable the timers and save the already elapsed ticks
        let mut curr_lapic_ticks: Option<u32> = None;
        interrupts_disabled(|| {
            // make sure any pending timer interrupt will be ignored
            self.should_ignore_interrupt = true;

            if self.is_using_tsc {
                lapic.clear_tsc_deadline();
            }
            else {
                curr_lapic_ticks = Some(lapic.read_curr_timer_tick_count());
                lapic.stop_timer();
            }
        });

        /* Since timer was disabled there should be no concurrency issue      */

        let ticks_elapsed = if let Some(ticks) = curr_lapic_ticks {
            (self.last_lapic_timer_tick_count - ticks) as u64
        }
        else {
            tsc::rdtsc() - self.last_tsc_read
        };
        let time_elapsed = self.ticks_to_time(ticks_elapsed);
        self.runtime += time_elapsed;

        closure(self);

        self.curr_frequency = self.update_queue();

        self.should_ignore_interrupt = false;

        if self.curr_frequency < TIMER_DEFAULT_FREQUENCY {
            self.start_timer(lapic, self.curr_frequency);
        }
        else {
            self.start_timer(lapic, TIMER_DEFAULT_FREQUENCY);
        }
    }

    // Trigger finished alarms and return proper frequency for queue state
    #[inline]
    fn update_queue(&mut self) -> Time {
        let mut timer_required_frequency = TIMER_DEFAULT_FREQUENCY;
        self.is_updating_queue = true;

        if let Some(schedule_alarm_ref) = self.schedule_alarm.as_ref() {
            if schedule_alarm_ref.trigger_runtime <= self.runtime {
                // must take before call to notify in case alarm gets reset
                let schedule_alarm = self.schedule_alarm.take().unwrap();
                schedule_alarm.notify();
                // in case alarm got reset
                if let Some(schedule_alarm_ref) = self.schedule_alarm.as_ref() {
                    if schedule_alarm_ref.trigger_runtime - self.runtime < timer_required_frequency {
                        timer_required_frequency = schedule_alarm_ref.trigger_runtime - self.runtime;
                    }
                }
            }
            else if schedule_alarm_ref.trigger_runtime - self.runtime < timer_required_frequency {
                timer_required_frequency = schedule_alarm_ref.trigger_runtime - self.runtime;
            }
        }

        while let Some(alarm_rev) = self.alarm_queue.peek() {
            let alarm = &alarm_rev.0;
            if alarm.trigger_runtime <= self.runtime {
                alarm.notify();
                self.alarm_queue.pop();
                continue;
            }
            else if alarm.trigger_runtime - self.runtime < timer_required_frequency {
                timer_required_frequency = alarm.trigger_runtime - self.runtime;
            }
            break;
        }

        self.is_updating_queue = false;
        timer_required_frequency
    }

    #[inline]
    fn start_timer(&mut self, lapic: &mut Lapic, time_to_wait: Time) {
        self.curr_frequency = time_to_wait;

        if self.is_using_tsc {
            self.set_timer_tsc_deadline(lapic, time_to_wait);
        }
        else {
            self.enable_lapic_timer(lapic, time_to_wait, false);
        }
    }

    #[inline]
    fn enable_lapic_timer(&mut self, lapic: &mut Lapic, time_to_wait: Time, is_periodic: bool) {
        let ticks_to_wait = self.time_to_ticks(time_to_wait);
        let ticks_to_wait = cmp::min(u32::MAX as u64, ticks_to_wait) as u32;
        self.last_lapic_timer_tick_count = ticks_to_wait;
        lapic.start_timer(ticks_to_wait, is_periodic);
    }

    #[inline]
    fn set_timer_tsc_deadline(&mut self, lapic: &mut Lapic, time_to_wait: Time) {
        let cycles_to_wait = self.time_to_ticks(time_to_wait);
        self.last_tsc_read = lapic.set_tsc_deadline(cycles_to_wait);
    }

    #[inline]
    fn time_to_ticks(&self, time: Time) -> u64 {
        let timestamp = time.to_ts();

        match timestamp.ts_type {
            super::TimestampType::Seconds =>
                timestamp.ts.saturating_mul(self.ticks_per_sec),
            super::TimestampType::Miliseconds =>
                timestamp.ts.saturating_mul(self.ticks_per_ms),
            super::TimestampType::Microseconds =>
                timestamp.ts.saturating_mul(self.ticks_per_us),
            super::TimestampType::Nanoseconds =>
                timestamp.ts.saturating_mul(self.ticks_per_ns),
        }
    }

    #[inline]
    fn ticks_to_time(&self, ticks: u64) -> Time {
        let div_rem = |dividend: u64, divisor: u64| {
            (dividend/divisor, dividend%divisor)
        };

        let (mut secs, ms_ticks) = div_rem(ticks, self.ticks_per_sec);
        let (mut ms, us_ticks) = div_rem(ms_ticks, self.ticks_per_ms);
        let (mut us, ns_ticks) = div_rem(us_ticks, self.ticks_per_us);
        let mut ns = ns_ticks / self.ticks_per_ns;

        if ns >= 1000 { us = us.saturating_add(ns/1000);     ns = ns%1000; }
        if us >= 1000 { ms = ms.saturating_add(us/1000);     us = us%1000; }
        if ms >= 1000 { secs = secs.saturating_add(ms/1000); ms = ms%1000; }

        Time::new(secs, ms as u16, us as u16, ns as u16)
    }
}


def_interrupt_handler!(timer_handler,
    fn timer_handler_fn(_stack_frame: &StackFrame) {
        use crate::x86_64::interrupts::apic::lapic;

        let processor = processor::get();
        let timer = processor.timer();

        if timer.should_ignore_interrupt {
            lapic::eoi();
            return;
        }

        let lapic = processor.lapic();

        // if using tsc update runtime by comparing current tsc with last read
        if timer.is_using_tsc {
            let cycles_elapsed = tsc::rdtsc() - timer.last_tsc_read;
            let time_elapsed = timer.ticks_to_time(cycles_elapsed);
            timer.runtime += time_elapsed;
        }
        else {
            timer.runtime += timer.curr_frequency;
        }

        timer.curr_frequency = timer.update_queue();

        if timer.curr_frequency < TIMER_DEFAULT_FREQUENCY {
            timer.start_timer(lapic, timer.curr_frequency);
        }
        else {
            timer.start_timer(lapic, TIMER_DEFAULT_FREQUENCY);
        }

        lapic::eoi();
    }
);
