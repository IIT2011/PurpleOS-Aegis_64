.code64
.section .text
.global switch_context
.type switch_context, @function

/*
 * switch_context(uint64_t **old_sp, uint64_t *new_sp, uint8_t *lock_to_release)
 * rdi = old_sp (pointer to current task's saved rsp field)
 * rsi = new_sp (new task's rsp value to restore)
 * rdx = lock_to_release (optional spinlock pointer to release after stack switch)
 */
switch_context:
    // Push callee-saved registers
    pushfq
    pushq %rbx
    pushq %rbp
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15

    // Save old stack pointer
    movq %rsp, (%rdi)

    // Switch stack
    movq %rsi, %rsp

    // Release spinlock now that old task's stack frame is completely saved
    // and this CPU is officially executing on the new stack
    testq %rdx, %rdx
    jz 1f
    movb $0, (%rdx)

1:
    // Restore callee-saved registers from new stack
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
    popfq

    ret

.global task_trampoline
.type task_trampoline, @function
task_trampoline:
    popq %rdi       /* entry function pointer */
    popq %rsi       /* arg pointer */
    call task_entry_wrapper
2:  hlt
    jmp 2b
