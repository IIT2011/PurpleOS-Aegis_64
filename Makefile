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
           src/kernel/interrupt.S \
           src/kernel/smp_trampoline.S

C_SRCS = src/kernel/main.c \
         src/kernel/string.c \
         src/kernel/idt.c \
         src/kernel/pic.c \
         src/kernel/cpu.c \
         src/kernel/smp.c \
         src/kernel/acpi.c \
         src/kernel/task.c \
         src/kernel/shell.c \
         src/kernel/storage.c \
         src/kernel/ksym.c \
         src/kernel/apapp.c \
         src/drivers/serial.c \
         src/drivers/video.c \
         src/drivers/font8x16.c \
         src/drivers/pit.c \
         src/drivers/keyboard.c \
         src/drivers/mouse.c \
         src/drivers/desktop.c \
         src/drivers/netdev.c \
         src/drivers/apdrv.c \
         src/drivers/apic.c \
         src/drivers/pci.c \
         src/drivers/purple_detect.c \
         src/drivers/dma.c \
         src/drivers/block_dev.c \
         src/drivers/storage/ata_pio.c \
         src/drivers/storage/ahci.c \
         src/drivers/storage/nvme.c \
         src/fs/gpt.c \
         src/fs/partition_dev.c \
         src/fs/purplefs_v2.c \
         src/crypto/sha256.c \
         src/security/aegis_detect.c \
         src/mm/pmm.c \
         src/mm/vmm.c

OBJS = $(patsubst src/%.S,$(BUILD_DIR)/%.o,$(ASM_SRCS)) \
       $(patsubst src/%.c,$(BUILD_DIR)/%.o,$(C_SRCS))

KERNEL64_ELF = $(BUILD_DIR)/kernel64.elf
KERNEL32_ELF = $(BUILD_DIR)/kernel.elf

.PHONY: all clean test test-asan check check-scheduler-qemu check-storage-qemu check-smp-qemu run-qemu run-uefi

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

test: $(BUILD_DIR)/test_pmm $(BUILD_DIR)/test_vmm $(BUILD_DIR)/test_scheduler $(BUILD_DIR)/test_purplefs_v2 $(BUILD_DIR)/test_storage_gpt $(BUILD_DIR)/test_acpi $(BUILD_DIR)/test_aegis_detect $(BUILD_DIR)/test_purple_detect $(BUILD_DIR)/test_desktop $(BUILD_DIR)/test_apdrv $(BUILD_DIR)/test_apapp $(BUILD_DIR)/test_video $(BUILD_DIR)/test_block_dev $(BUILD_DIR)/test_ahci $(BUILD_DIR)/test_nvme
	@echo "=== Running Host Test Suite ==="
	./$(BUILD_DIR)/test_pmm
	./$(BUILD_DIR)/test_vmm
	./$(BUILD_DIR)/test_scheduler
	./$(BUILD_DIR)/test_purplefs_v2
	./$(BUILD_DIR)/test_storage_gpt
	./$(BUILD_DIR)/test_acpi
	./$(BUILD_DIR)/test_aegis_detect
	./$(BUILD_DIR)/test_purple_detect
	./$(BUILD_DIR)/test_desktop
	./$(BUILD_DIR)/test_apdrv
	./$(BUILD_DIR)/test_apapp
	./$(BUILD_DIR)/test_video
	./$(BUILD_DIR)/test_block_dev
	./$(BUILD_DIR)/test_ahci
	./$(BUILD_DIR)/test_nvme

test-asan:
	@mkdir -p $(BUILD_DIR)
	$(CC) -fsanitize=address,undefined -g -Wall -Wextra -Iinclude tests/test_purplefs_v2.c src/fs/purplefs_v2.c host/purplefs_v2_host.c src/kernel/string.c -o $(BUILD_DIR)/test_purplefs_v2_asan
	./$(BUILD_DIR)/test_purplefs_v2_asan
	$(CC) -fsanitize=address,undefined -g -Wall -Wextra -Iinclude tests/test_storage_gpt.c src/drivers/storage/ata_pio.c src/drivers/block_dev.c src/fs/gpt.c src/fs/partition_dev.c src/fs/purplefs_v2.c host/purplefs_v2_host.c src/kernel/string.c src/drivers/serial.c -o $(BUILD_DIR)/test_storage_gpt_asan
	./$(BUILD_DIR)/test_storage_gpt_asan
	$(CC) -fsanitize=address,undefined -g -Wall -Wextra -D__KERNEL_HOST_TEST__ -Iinclude tests/test_acpi.c src/kernel/acpi.c src/kernel/cpu.c src/kernel/string.c src/drivers/serial.c -o $(BUILD_DIR)/test_acpi_asan
	./$(BUILD_DIR)/test_acpi_asan
	$(CC) -fsanitize=address,undefined -g -Wall -Wextra -D__KERNEL_HOST_TEST__ -Iinclude tests/test_aegis_detect.c src/crypto/sha256.c src/security/aegis_detect.c src/kernel/string.c src/drivers/serial.c -o $(BUILD_DIR)/test_aegis_detect_asan
	./$(BUILD_DIR)/test_aegis_detect_asan
	$(CC) -fsanitize=address,undefined -g -Wall -Wextra -D__KERNEL_HOST_TEST__ -Iinclude tests/test_purple_detect.c src/drivers/purple_detect.c src/drivers/pci.c src/kernel/string.c src/drivers/serial.c -o $(BUILD_DIR)/test_purple_detect_asan
	./$(BUILD_DIR)/test_purple_detect_asan
	$(CC) -fsanitize=address,undefined -g -Wall -Wextra -D__KERNEL_HOST_TEST__ -Iinclude tests/test_desktop.c src/drivers/desktop.c src/drivers/mouse.c src/drivers/video.c src/drivers/font8x16.c src/kernel/cpu.c src/mm/pmm.c src/kernel/string.c src/drivers/serial.c -o $(BUILD_DIR)/test_desktop_asan
	./$(BUILD_DIR)/test_desktop_asan
	$(CC) -fsanitize=address,undefined -g -Wall -Wextra -D__KERNEL_HOST_TEST__ -Iinclude tests/test_apdrv.c src/drivers/apdrv.c src/kernel/ksym.c src/drivers/netdev.c src/drivers/pci.c src/drivers/purple_detect.c src/drivers/desktop.c src/drivers/mouse.c src/drivers/video.c src/drivers/font8x16.c src/kernel/cpu.c src/mm/pmm.c src/kernel/string.c src/drivers/serial.c -o $(BUILD_DIR)/test_apdrv_asan
	./$(BUILD_DIR)/test_apdrv_asan
	$(CC) -fsanitize=address,undefined -g -Wall -Wextra -D__KERNEL_HOST_TEST__ -Iinclude tests/test_apapp.c src/kernel/apapp.c src/crypto/sha256.c src/drivers/desktop.c src/drivers/mouse.c src/drivers/video.c src/drivers/font8x16.c src/kernel/cpu.c src/mm/pmm.c src/kernel/string.c src/drivers/serial.c -o $(BUILD_DIR)/test_apapp_asan
	./$(BUILD_DIR)/test_apapp_asan
	$(CC) -fsanitize=address,undefined -g -Wall -Wextra -D__KERNEL_HOST_TEST__ -Iinclude tests/test_video.c src/drivers/video.c src/drivers/font8x16.c src/kernel/string.c src/drivers/serial.c -o $(BUILD_DIR)/test_video_asan
	./$(BUILD_DIR)/test_video_asan
	$(CC) -fsanitize=address,undefined -g -Wall -Wextra -D__KERNEL_HOST_TEST__ -Iinclude tests/test_block_dev.c src/drivers/block_dev.c src/drivers/dma.c src/kernel/string.c src/drivers/serial.c -o $(BUILD_DIR)/test_block_dev_asan
	./$(BUILD_DIR)/test_block_dev_asan
	$(CC) -fsanitize=address,undefined -g -Wall -Wextra -D__KERNEL_HOST_TEST__ -Iinclude tests/test_ahci.c src/drivers/storage/ahci.c src/drivers/block_dev.c src/drivers/dma.c src/drivers/pci.c src/kernel/string.c src/drivers/serial.c -o $(BUILD_DIR)/test_ahci_asan
	./$(BUILD_DIR)/test_ahci_asan
	$(CC) -fsanitize=address,undefined -g -Wall -Wextra -D__KERNEL_HOST_TEST__ -Iinclude tests/test_nvme.c src/drivers/storage/nvme.c src/drivers/block_dev.c src/drivers/dma.c src/drivers/pci.c src/kernel/string.c src/drivers/serial.c -o $(BUILD_DIR)/test_nvme_asan
	./$(BUILD_DIR)/test_nvme_asan

$(BUILD_DIR)/test_nvme: tests/test_nvme.c src/drivers/storage/nvme.c src/drivers/block_dev.c src/drivers/dma.c src/drivers/pci.c src/kernel/string.c src/drivers/serial.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -D__KERNEL_HOST_TEST__ -Iinclude $^ -o $@

$(BUILD_DIR)/test_ahci: tests/test_ahci.c src/drivers/storage/ahci.c src/drivers/block_dev.c src/drivers/dma.c src/drivers/pci.c src/kernel/string.c src/drivers/serial.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -D__KERNEL_HOST_TEST__ -Iinclude $^ -o $@

$(BUILD_DIR)/test_block_dev: tests/test_block_dev.c src/drivers/block_dev.c src/drivers/dma.c src/kernel/string.c src/drivers/serial.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -D__KERNEL_HOST_TEST__ -Iinclude $^ -o $@

$(BUILD_DIR)/test_video: tests/test_video.c src/drivers/video.c src/drivers/font8x16.c src/kernel/string.c src/drivers/serial.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -D__KERNEL_HOST_TEST__ -Iinclude $^ -o $@

$(BUILD_DIR)/test_apapp: tests/test_apapp.c src/kernel/apapp.c src/crypto/sha256.c src/drivers/desktop.c src/drivers/mouse.c src/drivers/video.c src/drivers/font8x16.c src/kernel/cpu.c src/mm/pmm.c src/kernel/string.c src/drivers/serial.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -D__KERNEL_HOST_TEST__ -Iinclude $^ -o $@

$(BUILD_DIR)/test_apdrv: tests/test_apdrv.c src/drivers/apdrv.c src/kernel/ksym.c src/drivers/netdev.c src/drivers/pci.c src/drivers/purple_detect.c src/drivers/desktop.c src/drivers/mouse.c src/drivers/video.c src/drivers/font8x16.c src/kernel/cpu.c src/mm/pmm.c src/kernel/string.c src/drivers/serial.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -D__KERNEL_HOST_TEST__ -Iinclude $^ -o $@

$(BUILD_DIR)/test_desktop: tests/test_desktop.c src/drivers/desktop.c src/drivers/mouse.c src/drivers/video.c src/drivers/font8x16.c src/kernel/cpu.c src/mm/pmm.c src/kernel/string.c src/drivers/serial.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -D__KERNEL_HOST_TEST__ -Iinclude $^ -o $@

$(BUILD_DIR)/test_purple_detect: tests/test_purple_detect.c src/drivers/purple_detect.c src/drivers/pci.c src/kernel/string.c src/drivers/serial.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -D__KERNEL_HOST_TEST__ -Iinclude $^ -o $@

$(BUILD_DIR)/test_aegis_detect: tests/test_aegis_detect.c src/crypto/sha256.c src/security/aegis_detect.c src/kernel/string.c src/drivers/serial.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -D__KERNEL_HOST_TEST__ -Iinclude $^ -o $@

$(BUILD_DIR)/test_acpi: tests/test_acpi.c src/kernel/acpi.c src/kernel/cpu.c src/kernel/string.c src/drivers/serial.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -D__KERNEL_HOST_TEST__ -Iinclude $^ -o $@

$(BUILD_DIR)/mkpfs2_disk: host/mkpfs2_disk.c src/drivers/storage/ata_pio.c src/drivers/block_dev.c src/fs/purplefs_v2.c src/fs/gpt.c host/purplefs_v2_host.c src/crypto/sha256.c src/kernel/string.c src/drivers/serial.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -Wall -Wextra -Iinclude $^ -o $@

$(BUILD_DIR)/test_storage_gpt: tests/test_storage_gpt.c src/drivers/storage/ata_pio.c src/drivers/block_dev.c src/fs/gpt.c src/fs/partition_dev.c src/fs/purplefs_v2.c host/purplefs_v2_host.c src/kernel/string.c src/drivers/serial.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -Wall -Wextra -Iinclude $^ -o $@

$(BUILD_DIR)/test_purplefs_v2: tests/test_purplefs_v2.c src/fs/purplefs_v2.c host/purplefs_v2_host.c src/kernel/string.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -Wall -Wextra -Iinclude $^ -o $@

$(BUILD_DIR)/test_pmm: tests/test_pmm.c src/mm/pmm.c src/kernel/string.c src/drivers/serial.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -D__KERNEL_HOST_TEST__ -Iinclude $^ -o $@

$(BUILD_DIR)/test_vmm: tests/test_vmm.c src/mm/vmm.c src/mm/pmm.c src/kernel/string.c src/drivers/serial.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -D__KERNEL_HOST_TEST__ -Iinclude $^ -o $@

$(BUILD_DIR)/test_scheduler: tests/test_scheduler.c src/kernel/task.c src/kernel/switch.S src/kernel/cpu.c src/mm/pmm.c src/kernel/string.c src/drivers/serial.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -g -D__KERNEL_HOST_TEST__ -Iinclude $^ -o $@

check: test all
	@echo "=== Verifying Boot Compatibility ==="
	grub-file --is-x86-multiboot $(KERNEL32_ELF)
	@echo "[OK] Multiboot check succeeded for $(KERNEL32_ELF)"

check-scheduler-qemu: all
	@$(PYTHON) scripts/test_qemu_scheduler.py

check-storage-qemu: all $(BUILD_DIR)/mkpfs2_disk
	@$(PYTHON) scripts/test_qemu_storage.py

check-writable-storage-qemu: all $(BUILD_DIR)/mkpfs2_disk
	@$(PYTHON) scripts/test_qemu_writable_storage.py

check-smp-qemu: all
	@$(PYTHON) scripts/test_qemu_smp.py

check-modern-storage-qemu: all
	@$(PYTHON) scripts/test_qemu_ahci_nvme.py

check-graphics-qemu: all
	@$(PYTHON) scripts/test_qemu_graphics.py

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
