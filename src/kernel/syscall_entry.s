.code64
.section .text
.global syscall_entry_asm
.extern syscall_dispatch

/*
 * Hardware SYSCALL entry:
 * Hardware saves:
 *   rcx = user rip
 *   r11 = user rflags
 * Hardware switches:
 *   cs = STAR[47:32]
 *   ss = STAR[47:32] + 8
 */
syscall_entry_asm:
    pushq %r11          /* Saved user RFLAGS */
    pushq %rcx          /* Saved user RIP */

    pushq %rbp
    pushq %rbx
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15

    /* Call Rust syscall_dispatch(num, a1, a2, a3, a4, a5, a6) */
    /* Incoming: rax=num, rdi=a1, rsi=a2, rdx=a3, r10=a4, r8=a5, r9=a6 */
    /* System V AMD64 call convention: rdi, rsi, rdx, rcx, r8, r9, 7th arg on stack */
    pushq %r9           /* 7th arg: a6 */
    movq %r8, %r9       /* 6th arg: a5 */
    movq %r10, %r8      /* 5th arg: a4 */
    movq %rdx, %rcx     /* 4th arg: a3 */
    movq %rsi, %rdx     /* 3rd arg: a2 */
    movq %rdi, %rsi     /* 2nd arg: a1 */
    movq %rax, %rdi     /* 1st arg: num */

    call syscall_dispatch

    addq $8, %rsp       /* clean up 7th arg on stack */

    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbx
    popq %rbp

    popq %rcx          /* Restore user RIP */
    popq %r11          /* Restore user RFLAGS */

    sysretq
