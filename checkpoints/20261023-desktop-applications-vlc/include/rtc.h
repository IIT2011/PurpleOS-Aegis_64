#ifndef RTC_H
#define RTC_H

#include "types.h"

typedef struct {
    uint8_t  second;
    uint8_t  minute;
    uint8_t  hour;
    uint8_t  day;
    uint8_t  month;
    uint32_t year;
} rtc_time_t;

int  rtc_init(void);
void rtc_get_time(rtc_time_t *out_time);
void rtc_format_time(char *buf, uint32_t max_len);

void power_shutdown(void);
void power_reboot(void);

#endif /* RTC_H */
