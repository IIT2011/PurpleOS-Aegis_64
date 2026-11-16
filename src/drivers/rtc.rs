use crate::arch::x86_64::io::{inb, outb};

const CMOS_ADDR: u16 = 0x70;
const CMOS_DATA: u16 = 0x71;

#[derive(Debug, Copy, Clone, Default)]
pub struct WallTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

unsafe fn read_cmos(reg: u8) -> u8 {
    outb(CMOS_ADDR, reg);
    inb(CMOS_DATA)
}

fn bcd_to_bin(val: u8) -> u8 {
    (val & 0x0F) + ((val / 16) * 10)
}

pub fn init() {
    let t = get_time();
    crate::drivers::serial::print("RTC: Current CMOS Time: 20");
    crate::drivers::serial::print_u64((t.year % 100) as u64);
    crate::drivers::serial::print("-");
    crate::drivers::serial::print_u64(t.month as u64);
    crate::drivers::serial::print("-");
    crate::drivers::serial::print_u64(t.day as u64);
    crate::drivers::serial::print(" ");
    crate::drivers::serial::print_u64(t.hour as u64);
    crate::drivers::serial::print(":");
    crate::drivers::serial::print_u64(t.minute as u64);
    crate::drivers::serial::print(":");
    crate::drivers::serial::print_u64(t.second as u64);
    crate::drivers::serial::println(" UTC");
}

pub fn get_time() -> WallTime {
    unsafe {
        // Wait until RTC update in progress (bit 7 of reg 0x0A) is clear
        while (read_cmos(0x0A) & 0x80) != 0 {
            core::hint::spin_loop();
        }

        let sec = read_cmos(0x00);
        let min = read_cmos(0x02);
        let hour = read_cmos(0x04);
        let day = read_cmos(0x07);
        let month = read_cmos(0x08);
        let year = read_cmos(0x09);
        let reg_b = read_cmos(0x0B);

        let is_bcd = (reg_b & 0x04) == 0;
        let is_24h = (reg_b & 0x02) != 0;

        let second = if is_bcd { bcd_to_bin(sec) } else { sec };
        let minute = if is_bcd { bcd_to_bin(min) } else { min };
        let mut h = if is_bcd { bcd_to_bin(hour & 0x7F) } else { hour & 0x7F };
        let d = if is_bcd { bcd_to_bin(day) } else { day };
        let m = if is_bcd { bcd_to_bin(month) } else { month };
        let y = (if is_bcd { bcd_to_bin(year) } else { year }) as u16 + 2000;

        if !is_24h && (hour & 0x80) != 0 {
            h = (h + 12) % 24;
        }

        WallTime {
            year: y,
            month: m,
            day: d,
            hour: h,
            minute,
            second,
        }
    }
}
