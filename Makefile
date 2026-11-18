# Pure Rust Aegis64 (Purple OS) Makefile

TARGET ?= x86_64-unknown-none
CARGO ?= cargo
OBJCOPY ?= rust-objcopy
QEMU ?= qemu-system-x86_64

KERNEL_BIN = target/$(TARGET)/debug/aegis64
KERNEL_ELF = target/kernel.elf
DISK_IMG ?= build/disk.img

.PHONY: all build check run clean $(KERNEL_BIN)

all: build

build: $(KERNEL_ELF)

$(KERNEL_BIN):
	$(CARGO) build --target $(TARGET)

$(KERNEL_ELF): $(KERNEL_BIN)
	$(OBJCOPY) -O elf32-i386 $< $@

check:
	$(CARGO) check --target $(TARGET)

run: build
	$(QEMU) -kernel $(KERNEL_ELF) -serial stdio -display none -m 256M -smp 4 -drive file=$(DISK_IMG),format=raw,index=0,media=disk -no-reboot

clean:
	$(CARGO) clean
