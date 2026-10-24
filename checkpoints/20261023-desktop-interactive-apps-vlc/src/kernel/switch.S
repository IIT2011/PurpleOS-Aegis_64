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
