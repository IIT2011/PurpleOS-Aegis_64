use crate::drivers::pit::get_ticks;
use crate::drivers::rtc::get_time;

pub fn get_wall_clock() -> crate::drivers::rtc::WallTime {
    get_time()
}

pub fn get_system_ticks() -> u64 {
    get_ticks()
}
