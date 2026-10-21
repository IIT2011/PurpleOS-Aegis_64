#include "rtc.h"
#include "io.h"
#include "serial.h"
#include "string.h"
#include "purple_detect.h"

#define CMOS_ADDR 0x70
#define CMOS_DATA 0x71

static uint8_t cmos_read(uint8_t reg) {
    outb(CMOS_ADDR, reg);
    return inb(CMOS_DATA);
}

static uint8_t bcd_to_bin(uint8_t val) {
    return ((val >> 4) * 10) + (val & 0x0F);
}

static int cmos_is_updating(void) {
    outb(CMOS_ADDR, 0x0A);
    return (inb(CMOS_DATA) & 0x80);
}

int rtc_init(void) {
    rtc_time_t t;
    rtc_get_time(&t);
    char buf[32];
    rtc_format_time(buf, sizeof(buf));
    kinfo("RTC: CMOS Real-Time Clock active (Current Time: %s UTC)", buf);

    purple_detect_register(PURPLE_CAT_TIMER, "RTC", "Motorola",
                           "MC146818 CMOS RTC", "ISA", 0x70, 8,
                           PURPLE_STATE_ACTIVE, PURPLE_DEV_FLAG_BUILTIN);
    return 0;
}

void rtc_get_time(rtc_time_t *out_time) {
    if (!out_time) return;

#ifndef __KERNEL_HOST_TEST__
    uint32_t timeout = 100000;
    while (cmos_is_updating() && timeout--) {
        __asm__ volatile("pause");
    }

    uint8_t sec   = cmos_read(0x00);
    uint8_t min   = cmos_read(0x02);
    uint8_t hour  = cmos_read(0x04);
    uint8_t day   = cmos_read(0x07);
    uint8_t month = cmos_read(0x08);
    uint8_t year  = cmos_read(0x09);
    uint8_t reg_b = cmos_read(0x0B);

    /* Convert BCD to binary if bit 2 of Reg B is 0 */
    if (!(reg_b & 0x04)) {
        sec   = bcd_to_bin(sec);
        min   = bcd_to_bin(min);
        hour  = ((hour & 0x7F) != hour) ? (bcd_to_bin(hour & 0x7F) | 0x80) : bcd_to_bin(hour);
        day   = bcd_to_bin(day);
        month = bcd_to_bin(month);
        year  = bcd_to_bin(year);
    }

    /* Handle 12-hour clock if bit 1 of Reg B is 0 */
    if (!(reg_b & 0x02) && (hour & 0x80)) {
        hour = ((hour & 0x7F) + 12) % 24;
    }

    out_time->second = sec;
    out_time->minute = min;
    out_time->hour   = hour;
    out_time->day    = day;
    out_time->month  = month;
    out_time->year   = 2000 + year;
#else
    out_time->second = 30;
    out_time->minute = 15;
    out_time->hour   = 10;
    out_time->day    = 21;
    out_time->month  = 10;
    out_time->year   = 2026;
#endif
}

void rtc_format_time(char *buf, uint32_t max_len) {
    if (!buf || max_len < 20) return;
    rtc_time_t t;
    rtc_get_time(&t);

    /* Format: YYYY-MM-DD HH:MM:SS */
    buf[0] = '0' + (t.year / 1000) % 10;
    buf[1] = '0' + (t.year / 100) % 10;
    buf[2] = '0' + (t.year / 10) % 10;
    buf[3] = '0' + (t.year % 10);
    buf[4] = '-';
    buf[5] = '0' + (t.month / 10) % 10;
    buf[6] = '0' + (t.month % 10);
    buf[7] = '-';
    buf[8] = '0' + (t.day / 10) % 10;
    buf[9] = '0' + (t.day % 10);
    buf[10] = ' ';
    buf[11] = '0' + (t.hour / 10) % 10;
    buf[12] = '0' + (t.hour % 10);
    buf[13] = ':';
    buf[14] = '0' + (t.minute / 10) % 10;
    buf[15] = '0' + (t.minute % 10);
    buf[16] = ':';
    buf[17] = '0' + (t.second / 10) % 10;
    buf[18] = '0' + (t.second % 10);
    buf[19] = '\0';
}

void power_shutdown(void) {
    kinfo("Power: Executing hardware platform shutdown...");
    // 1. QEMU ACPI poweroff port
    outw(0x604, 0x2000);
    // 2. Bochs / VirtualBox ports
    outw(0xB004, 0x2000);
    outw(0x4004, 0x3400);

    // Halt if shutdown not supported by hypervisor
    while (1) {
        __asm__ volatile("cli; hlt");
    }
}

void power_reboot(void) {
    kinfo("Power: Executing hardware platform reboot...");
    // 1. 8042 Keyboard controller pulse line
    uint8_t good = 0x02;
    while (good & 0x02) good = inb(0x64);
    outb(0x64, 0xFE);

    // 2. PCI reset register
    outb(0xCF9, 0x06);

    // 3. Fallback: Triple fault
    struct {
        uint16_t limit;
        uint64_t base;
    } __attribute__((packed)) null_idt = { 0, 0 };
    __asm__ volatile("lidt %0; int3" : : "m"(null_idt));

    while (1) {
        __asm__ volatile("cli; hlt");
    }
}
