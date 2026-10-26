#ifndef SYSCALL_H
#define SYSCALL_H

#include "types.h"

/* =========================================================================
 * PURPLE OS SYSTEM CALL TABLE
 * ========================================================================= */
#define SYS_EXIT             1
#define SYS_THREAD_YIELD     2
#define SYS_THREAD_CREATE    3
#define SYS_THREAD_EXIT      4
#define SYS_SLEEP_MS         5
#define SYS_GET_TICKS        6
#define SYS_GET_TIME_MS      7
#define SYS_GET_WALLCLOCK    8

/* Memory */
#define SYS_MMAP             10
#define SYS_MUNMAP           11

/* Filesystem */
#define SYS_OPEN             20
#define SYS_READ             21
#define SYS_WRITE            22
#define SYS_SEEK             23
#define SYS_CLOSE            24
#define SYS_STAT             25
#define SYS_EXISTS           26
#define SYS_DIR_OPEN         27
#define SYS_DIR_READ         28
#define SYS_DIR_CLOSE        29

/* Graphics & Windows */
#define SYS_WINDOW_CREATE    40
#define SYS_WINDOW_DESTROY   41
#define SYS_WINDOW_SURFACE   42
#define SYS_WINDOW_PRESENT   43
#define SYS_WINDOW_TITLE     44
#define SYS_POLL_EVENT       45
#define SYS_WAIT_EVENT       46

/* Audio */
#define SYS_AUDIO_AVAILABLE  50
#define SYS_AUDIO_PLAY_PCM   51
#define SYS_AUDIO_SET_VOL    52

/* Diagnostics */
#define SYS_PRINT            60

/* Standard Error Codes */
#define SYS_OK               0
#define SYS_ERR_PERM        -1
#define SYS_ERR_NOENT       -2
#define SYS_ERR_NOMEM       -12
#define SYS_ERR_FAULT       -14
#define SYS_ERR_INVAL       -22
#define SYS_ERR_NOSYS       -38

/* User address space limits: [USER_MIN_VADDR .. USER_MAX_VADDR] */
#define USER_MIN_VADDR       0x0000000000400000ULL /* 4 MiB */
#define USER_MAX_VADDR       0x00007FFFFFFFFFFFULL /* 128 TiB canonical user limit */

void syscall_init(void);
void syscall_ap_init(void);
int64_t syscall_dispatch(uint64_t sys_no, uint64_t a1, uint64_t a2, uint64_t a3, uint64_t a4, uint64_t a5, uint64_t a6);
int syscall_validate_user_ptr(const void *ptr, size_t len, int write_access);
int syscall_validate_user_string(const char *str, size_t max_len);

#endif /* SYSCALL_H */
