.code64
.section .text
.global syscall_entry_asm
.extern syscall_dispatch

.section .bss
.align 16
syscall_user_rsp:
    .quad 0
syscall_kernel_stack:
    .skip 16384
syscall_kernel_stack_top:

.section .text
syscall_entry_asm:
    /* Save user stack and switch to kernel syscall stack */
    movq %rsp, (syscall_user_rsp)
    movq $syscall_kernel_stack_top, %rsp

    /* Preserve user registers and syscall context */
    pushq (syscall_user_rsp)  /* User RSP */
    pushq %r11                /* User RFLAGS */
    pushq %rcx                /* User RIP */
    pushq %rbx
    pushq %rbp
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15

    /* Prepare C calling convention:
       syscall_dispatch(sys_no, a1, a2, a3, a4, a5, a6)
       x86-64 syscall arguments passed in:
         RAX: sys_no
         RDI: a1
         RSI: a2
         RDX: a3
         R10: a4
         R8:  a5
         R9:  a6

       C calling convention parameters:
         RDI = arg1 (sys_no)
         RSI = arg2 (a1)
         RDX = arg3 (a2)
         RCX = arg4 (a3)
         R8  = arg5 (a4)
         R9  = arg6 (a5)
         Stack = arg7 (a6)
    */
    pushq %r9                 /* 7th parameter (a6) on stack */
    movq %r8, %r9             /* 6th parameter (a5) */
    movq %r10, %r8            /* 5th parameter (a4) */
    movq %rdx, %rcx           /* 4th parameter (a3) */
    movq %rsi, %rdx           /* 3rd parameter (a2) */
    movq %rdi, %rsi           /* 2nd parameter (a1) */
    movq %rax, %rdi           /* 1st parameter (sys_no) */

    call syscall_dispatch

    addq $8, %rsp             /* clean up 7th argument */

    /* Restore registers (%rax contains return value) */
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
    popq %rcx                 /* Restore User RIP for sysret */
    popq %r11                 /* Restore User RFLAGS for sysret */
    popq %rsp                 /* Restore User RSP */

    sysretq
