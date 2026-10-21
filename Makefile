CC = gcc
LD = ld
OBJCOPY = objcopy
PYTHON = python3
QEMU = qemu-system-x86_64

CFLAGS = -ffreestanding -O2 -Wall -Wextra -fno-pie -fno-pic -fno-stack-protector -mno-red-zone -mgeneral-regs-only -mcmodel=kernel -Iinclude
LDFLAGS = -m elf_x86_64 -nostdlib -T linker.ld

BUILD_DIR = build

ASM_SRCS = src/kernel/boot.S \
           src/kernel/switch.S \
           src/kernel/interrupt.S

C_SRCS = src/kernel/main.c \
         src/kernel/string.c \
         src/kernel/idt.c \
         src/kernel/pic.c \
         src/kernel/task.c \
         src/kernel/shell.c \
         src/drivers/serial.c \
         src/drivers/video.c \
         src/drivers/font8x16.c \
         src/drivers/pit.c \
         src/drivers/keyboard.c \
         src/mm/pmm.c \
         src/mm/vmm.c

OBJS = $(patsubst src/%.S,$(BUILD_DIR)/%.o,$(ASM_SRCS)) \
       $(patsubst src/%.c,$(BUILD_DIR)/%.o,$(C_SRCS))

KERNEL64_ELF = $(BUILD_DIR)/kernel64.elf
KERNEL32_ELF = $(BUILD_DIR)/kernel.elf

.PHONY: all clean test check check-scheduler-qemu run-qemu run-uefi

all: $(KERNEL32_ELF) $(KERNEL64_ELF)

$(BUILD_DIR)/%.o: src/%.S
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -c $< -o $@

$(BUILD_DIR)/%.o: src/%.c
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -c $< -o $@

$(KERNEL64_ELF): $(OBJS) linker.ld
	@mkdir -p $(BUILD_DIR)
	$(LD) $(LDFLAGS) $(OBJS) -o $@

$(KERNEL32_ELF): $(KERNEL64_ELF)
	$(OBJCOPY) -O elf32-i386 $< $@
	@grub-file --is-x86-multiboot $@ && echo "[OK] Multiboot 1 verification passed: $@"

test: $(BUILD_DIR)/test_pmm $(BUILD_DIR)/test_vmm $(BUILD_DIR)/test_scheduler
	@echo "=== Running Host Test Suite ==="
	./$(BUILD_DIR)/test_pmm
	./$(BUILD_DIR)/test_vmm
	./$(BUILD_DIR)/test_scheduler

$(BUILD_DIR)/test_pmm: tests/test_pmm.c src/mm/pmm.c src/kernel/string.c src/drivers/serial.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -D__KERNEL_HOST_TEST__ -Iinclude $^ -o $@

$(BUILD_DIR)/test_vmm: tests/test_vmm.c src/mm/vmm.c src/mm/pmm.c src/kernel/string.c src/drivers/serial.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -D__KERNEL_HOST_TEST__ -Iinclude $^ -o $@

$(BUILD_DIR)/test_scheduler: tests/test_scheduler.c src/kernel/task.c src/kernel/switch.S src/mm/pmm.c src/kernel/string.c src/drivers/serial.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -D__KERNEL_HOST_TEST__ -Iinclude $^ -o $@

check: test all
	@echo "=== Verifying Boot Compatibility ==="
	grub-file --is-x86-multiboot $(KERNEL32_ELF)
	@echo "[OK] Multiboot check succeeded for $(KERNEL32_ELF)"

check-scheduler-qemu: all
	@$(PYTHON) scripts/test_qemu_scheduler.py

run-qemu: all
	$(QEMU) -kernel $(KERNEL32_ELF) -serial stdio -m 256M

run-uefi: all
	@$(PYTHON) scripts/build_uefi_disk.py
	$(QEMU) -drive if=pflash,format=raw,readonly=on,file=/usr/share/edk2/x64/OVMF_CODE.4m.fd \
	        -drive if=pflash,format=raw,file=$(BUILD_DIR)/ovmf_vars.fd \
	        -drive format=raw,file=$(BUILD_DIR)/aegis64-uefi.img \
	        -serial stdio -m 256M

clean:
	rm -rf $(BUILD_DIR)
