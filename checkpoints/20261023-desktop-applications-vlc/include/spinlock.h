#ifndef SPINLOCK_H
#define SPINLOCK_H

#include "types.h"
#include "io.h"

typedef struct {
    volatile uint32_t locked;
    volatile uint32_t cpu_id;
} spinlock_t;

#define SPINLOCK_INIT { .locked = 0, .cpu_id = 0xFFFFFFFF }

static inline void spinlock_init(spinlock_t *lock) {
    if (lock) {
        lock->locked = 0;
        lock->cpu_id = 0xFFFFFFFF;
    }
}

static inline void spinlock_acquire(spinlock_t *lock) {
    if (!lock) return;
#ifdef __KERNEL_HOST_TEST__
    lock->locked = 1;
#else
    while (__atomic_test_and_set(&lock->locked, __ATOMIC_ACQUIRE)) {
        pause();
    }
#endif
}

static inline int spinlock_try_acquire(spinlock_t *lock) {
    if (!lock) return 0;
#ifdef __KERNEL_HOST_TEST__
    if (lock->locked) return 0;
    lock->locked = 1;
    return 1;
#else
    return !__atomic_test_and_set(&lock->locked, __ATOMIC_ACQUIRE);
#endif
}

static inline void spinlock_release(spinlock_t *lock) {
    if (!lock) return;
#ifdef __KERNEL_HOST_TEST__
    lock->locked = 0;
#else
    __atomic_clear(&lock->locked, __ATOMIC_RELEASE);
#endif
}

static inline void spinlock_acquire_irqsave(spinlock_t *lock, uint64_t *rflags) {
    uint64_t flags = read_rflags();
    cli();
    if (rflags) *rflags = flags;
    spinlock_acquire(lock);
}

static inline void spinlock_release_irqrestore(spinlock_t *lock, uint64_t rflags) {
    spinlock_release(lock);
    if (rflags & (1ULL << 9)) {
        sti();
    }
}

#endif /* SPINLOCK_H */
