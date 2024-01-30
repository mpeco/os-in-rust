pub mod timer;


#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Time {
    secs: u64,
    ms: u16,
    us: u16,
    ns: u16
}
impl Time {
    #[inline]
    pub const fn new(secs: u64, ms: u16, us: u16, ns: u16) -> Time {
        assert!(ms < 1000 && us < 1000 && ns < 1000);
        Time { secs, ms, us, ns }
    }
    #[inline]
    pub const fn from_ms(ms: u64) -> Time {
        let secs = ms/1000;
        let ms = (ms%1000) as u16;
        Time::new(secs, ms, 0, 0)
    }
    #[inline]
    pub const fn from_us(us: u64) -> Time {
        let ms = us/1000;
        let us = (us%1000) as u16;
        let mut time = Self::from_ms(ms);
        time.us = us;
        time
    }
    pub const fn from_ns(ns: u64) -> Time {
        let us = ns/1000;
        let ns = (ns%1000) as u16;
        let mut time = Self::from_us(us);
        time.ns = ns;
        time
    }

    pub fn add_secs(&mut self, secs: u64) {
        self.secs = self.secs.saturating_add(secs);
    }
    pub fn add_ms(&mut self, ms: u64) {
        *self = *self + Self::from_ms(ms);
    }
    pub fn add_us(&mut self, us: u64) {
        *self = *self + Self::from_us(us);
    }
    pub fn add_ns(&mut self, ns: u64) {
        *self = *self + Self::from_ns(ns);
    }

    // Converts to timestamp of lowest precision required
    pub fn to_ts(&self) -> Timestamp {
        if self.ns > 0 {
            self.to_ns_ts()
        }
        else if self.us > 0 {
            self.to_us_ts()
        }
        else if self.ms > 0 {
            self.to_ms_ts()
        }
        else {
            self.to_secs_ts()
        }
    }
    #[inline]
    pub fn to_secs_ts(&self) -> Timestamp {
        Timestamp::new(self.secs, TimestampType::Seconds)
    }
    #[inline]
    pub fn to_ms_ts(&self) -> Timestamp {
        let mut timestamp = self.to_secs_ts().to_ts_type(TimestampType::Miliseconds);
        timestamp.ts = timestamp.ts.saturating_add(self.ms as u64);
        timestamp
    }
    #[inline]
    pub fn to_us_ts(&self) -> Timestamp {
        let mut timestamp = self.to_ms_ts().to_ts_type(TimestampType::Microseconds);
        timestamp.ts = timestamp.ts.saturating_add(self.us as u64);
        timestamp
    }
    pub fn to_ns_ts(&self) -> Timestamp {
        let mut timestamp = self.to_us_ts().to_ts_type(TimestampType::Nanoseconds);
        timestamp.ts = timestamp.ts.saturating_add(self.ns as u64);
        timestamp
    }
}
impl core::ops::Add for Time {
    type Output = Time;
    fn add(self, rhs: Self) -> Self::Output {
        let div_rem = |dividend: u16| {
            (dividend/1000, dividend%1000)
        };

        let (us_inc, ns) = div_rem(self.ns + rhs.ns);
        let (ms_inc, us) = div_rem(self.us + rhs.us + us_inc);
        let (secs_inc, ms) = div_rem(self.ms + rhs.ms + ms_inc);
        let secs = self.secs.saturating_add(rhs.secs).saturating_add(secs_inc as u64);
        Self::new(secs, ms, us, ns)
    }
}
impl core::ops::AddAssign for Time {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}
impl core::ops::Sub for Time {
    type Output = Time;

    fn sub(self, rhs: Self) -> Self::Output {
        let sub_over = |lhs: u16, rhs: u16| {
            (lhs.saturating_sub(rhs), rhs.saturating_sub(lhs))
        };

        let mut overflow: u16 = 0;

        let (mut ns, ns_over) = sub_over(self.ns, rhs.ns);
        if ns_over > 0 {
            ns = 1000-ns_over;
            overflow = 1;
        }

        let (mut us, us_over) = sub_over(self.us, rhs.us + overflow);
        overflow = 0;
        if us_over > 0 {
            us = 1000-us_over;
            overflow = 1;
        }

        let (mut ms, ms_over) = sub_over(self.ms, rhs.ms + overflow);
        overflow = 0;
        if ms_over > 0 {
            ms = 1000-ms_over;
            overflow = 1;
        }

        let secs = self.secs.saturating_sub(rhs.secs + overflow as u64);


        Self::new(secs, ms, us, ns)
    }
}
impl core::ops::SubAssign for Time {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}
impl core::fmt::Display for Time {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} Seconds, {} Miliseconds, {} Microseconds, {} Nanoseconds",
            self.secs, self.ms, self.us, self.ns
        )
    }
}

#[derive(Clone, Copy)]
pub enum TimestampType { Seconds, Miliseconds, Microseconds, Nanoseconds }
impl core::fmt::Display for TimestampType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let type_as_str = match self {
            TimestampType::Seconds => "Seconds",
            TimestampType::Miliseconds => "Miliseconds",
            TimestampType::Microseconds => "Microseconds",
            TimestampType::Nanoseconds => "Nanoseconds",
        };
        write!(f, "{type_as_str}")
    }
}
#[derive(Clone, Copy)]
pub struct Timestamp {
    pub ts: u64,
    pub ts_type: TimestampType,
}
impl Timestamp {
    pub fn new(ts: u64, ts_type: TimestampType) -> Timestamp {
        Timestamp { ts, ts_type }
    }

    pub fn to_ts_type(&self, ts_type: TimestampType) -> Timestamp {
        let ts_diff = ts_type as i8 - self.ts_type as i8;
        let time_mult = if ts_diff == 0 { 1 }
                             else            { (10 as u64).pow(3*(ts_diff.abs() as u32)) };

        let ts = if ts_diff < 0 { self.ts.saturating_div(time_mult) }
                      else           { self.ts.saturating_mul(time_mult) };

        Timestamp::new(ts, ts_type)
    }
}
impl core::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} {}", self.ts, self.ts_type)
    }
}

#[macro_export]
macro_rules! secs {
    ($x:literal) => { crate::time::Time::new($x, 0, 0, 0) }
}
#[macro_export]
macro_rules! ms {
    ($x:literal) => { crate::time::Time::from_ms($x) }
}
#[macro_export]
macro_rules! us {
    ($x:literal) => { crate::time::Time::from_us($x) }
}
#[macro_export]
macro_rules! ns {
    ($x:literal) => { crate::time::Time::from_ns($x) }
}
