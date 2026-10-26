.section .text
.global switch_context
.type switch_context, @function

switch_context:
    /* rdi = old_rsp, rsi = new_rsp */
    pushfq
    pushq %rbx
    pushq %rbp
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15

    /* Save current stack pointer to *old_rsp */
    movq %rsp, (%rdi)

    /* Switch to new stack */
    movq %rsi, %rsp

    /* Restore registers from new stack */
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
    /* Stack layout set by task_create:
       [rsp+0]: function pointer
       [rsp+8]: argument
    */
    popq %rax       /* Entry point */
    popq %rdi       /* Argument */

    /* Enable interrupts on task entry */
    sti

    /* Call the task entry point */
    call *%rax

    /* If task function returns, cleanly terminate task */
    call task_exit

.trampoline_halt:
    hlt
    jmp .trampoline_halt

.global user_trampoline
.type user_trampoline, @function

user_trampoline:
    /* Stack layout set by task_create_user for switch_context return:
       switch_context popped r15..rbx, rflags, and ret'd to here.
       Now the kernel stack contains the iretq frame:
         [rsp+0]:  User RIP
         [rsp+8]:  User CS  (0x23: RPL 3)
         [rsp+16]: User RFLAGS (0x202 = IF enabled)
         [rsp+24]: User RSP
         [rsp+32]: User SS  (0x1B: RPL 3)
    */
    xorq %rax, %rax
    movw %ax, %ds
    movw %ax, %es
    xorq %rax, %rax
    xorq %rbx, %rbx
    xorq %rcx, %rcx
    xorq %rdx, %rdx
    xorq %rsi, %rsi
    xorq %rdi, %rdi
    xorq %rbp, %rbp
    xorq %r8, %r8
    xorq %r9, %r9
    xorq %r10, %r10
    xorq %r11, %r11
    xorq %r12, %r12
    xorq %r13, %r13
    xorq %r14, %r14
    xorq %r15, %r15
    iretq

