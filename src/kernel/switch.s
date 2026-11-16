.code64
.section .text
.global switch_context
.type switch_context, @function

/*
 * switch_context(uint64_t **old_sp, uint64_t *new_sp)
 * rdi = old_sp (pointer to current task's saved rsp field)
 * rsi = new_sp (new task's rsp value to restore)
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

    // Restore callee-saved registers from new stack
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
    popfq

    ret
