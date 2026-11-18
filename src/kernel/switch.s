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

/*
 * enter_user_mode(entry_rip: u64, user_rsp: u64) -> !
 * Privilege transition from Ring 0 (Kernel) to Ring 3 (User) via iretq.
 * rdi = entry_rip (Canonical user virtual address of code)
 * rsi = user_rsp  (Canonical user virtual address of stack top)
 */
.global enter_user_mode
.type enter_user_mode, @function
enter_user_mode:
    cli

    /* Load user data segment selectors (GDT_USER_DS = 0x1B) */
    movw $0x1B, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %fs
    movw %ax, %gs

    /* Push iretq stack frame */
    pushq $0x1B          /* SS: User Data Segment (DPL 3) */
    pushq %rsi           /* RSP: User Stack Pointer */
    pushq $0x202         /* RFLAGS: Interrupts Enabled (IF=1, bit 1=1) */
    pushq $0x23          /* CS: User Code Segment (DPL 3) */
    pushq %rdi           /* RIP: User Instruction Pointer */

    /* Zero all general-purpose registers to eliminate kernel data leakage */
    xorq %rax, %rax
    xorq %rbx, %rbx
    xorq %rcx, %rcx
    xorq %rdx, %rdx
    xorq %rsi, %rsi
    xorq %rdi, %rdi
    xorq %rbp, %rbp
    xorq %r8,  %r8
    xorq %r9,  %r9
    xorq %r10, %r10
    xorq %r11, %r11
    xorq %r12, %r12
    xorq %r13, %r13
    xorq %r14, %r14
    xorq %r15, %r15

    iretq

/*
 * user_task_trampoline
 * Trampoline popped by switch_context when a new user-mode task begins.
 * Kernel stack contains:
 *   [sp+0]: user RIP (entry point)
 *   [sp+8]: user RSP (stack top)
 */
.global user_task_trampoline
.type user_task_trampoline, @function
user_task_trampoline:
    popq %rdi           /* user RIP */
    popq %rsi           /* user RSP */
    pushq %rsi          /* preserve user RSP across call */
    pushq %rdi          /* preserve user RIP across call */
    call prepare_user_entry
    popq %rdi           /* restore user RIP */
    popq %rsi           /* restore user RSP */
    call enter_user_mode
3:  hlt
    jmp 3b
