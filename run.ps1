cargo build --target x86_64-unknown-none
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
rust-objcopy -O elf32-i386 target/x86_64-unknown-none/debug/aegis64 target/kernel.elf
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
qemu-system-x86_64 -kernel target/kernel.elf -serial stdio -vga std -m 256M -smp 4 -drive file=build/disk.img,format=raw,index=0,media=disk -no-reboot
