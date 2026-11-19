@echo off
setlocal
cd /d "%~dp0" || exit /b 1
cargo build --target x86_64-unknown-none || exit /b 1
rust-objcopy -O elf32-i386 target/x86_64-unknown-none/debug/aegis64 target/kernel.elf || exit /b 1
qemu-system-x86_64 -kernel target/kernel.elf -serial stdio -display sdl,show-cursor=off -vga std -m 256M -smp 4 -drive file=build/disk.img,format=raw,index=0,media=disk
exit /b %ERRORLEVEL%
