# Pure Rust Aegis64 (Purple OS) Makefile

TARGET ?= x86_64-unknown-none
CARGO ?= cargo
OBJCOPY ?= x86_64-elf-objcopy
QEMU ?= qemu-system-x86_64

KERNEL_BIN = target/$(TARGET)/debug/aegis64
KERNEL_ELF = target/kernel.elf

.PHONY: all build check run clean

all: build

build: $(KERNEL_ELF)

$(KERNEL_BIN):
	$(CARGO) build --target $(TARGET)

$(KERNEL_ELF): $(KERNEL_BIN)
	$(OBJCOPY) -O elf32-i386 $< $@

check:
	$(CARGO) check --target $(TARGET)

run: build
	$(QEMU) -kernel $(KERNEL_ELF) -serial stdio -m 256M -smp 4

clean:
	$(CARGO) clean
