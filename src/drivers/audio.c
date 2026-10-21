#include "audio.h"
#include "pci.h"
#include "string.h"
#include "serial.h"
#include "purple_detect.h"

#ifndef __KERNEL_HOST_TEST__
#include "vmm.h"
#endif

#define HDA_REG_GCAP      0x0000
#define HDA_REG_VMIN      0x0002
#define HDA_REG_VMAJ      0x0003
#define HDA_REG_GCTL      0x0008
#define HDA_REG_STATESTS  0x000E

static audio_device_t audio_devices[AUDIO_MAX_DEVICES];
static uint32_t audio_dev_count = 0;

static const char *audio_type_to_str(audio_type_t t) {
    switch (t) {
        case AUDIO_TYPE_AC97: return "AC97 Audio";
        case AUDIO_TYPE_HDA:  return "Intel High Definition Audio";
        default:              return "Unknown Audio";
    }
}

int audio_init(void) {
    audio_dev_count = 0;
    memset(audio_devices, 0, sizeof(audio_devices));

    uint32_t pci_count = pci_get_device_count();
    for (uint32_t i = 0; i < pci_count; i++) {
        const pci_device_t *pdev = pci_get_device(i);
        if (!pdev) continue;

        if (pdev->class_code == 0x04) { // Multimedia audio controller
            if (audio_dev_count >= AUDIO_MAX_DEVICES) break;

            audio_device_t *adev = &audio_devices[audio_dev_count++];
            adev->id = audio_dev_count;
            adev->bus = pdev->bus;
            adev->dev = pdev->dev;
            adev->func = pdev->func;
            adev->vendor_id = pdev->vendor_id;
            adev->device_id = pdev->device_id;
            adev->irq = pdev->irq;
            adev->active = 1;

            pci_enable_bus_mastering(pdev->bus, pdev->dev, pdev->func);
            pci_enable_io(pdev->bus, pdev->dev, pdev->func);

            if (pdev->subclass == 0x03) {
                // Intel High Definition Audio
                adev->type = AUDIO_TYPE_HDA;
                adev->mmio_base = pci_get_bar_address(pdev, 0);
                strncpy(adev->name, "Intel HDA Codec Controller", sizeof(adev->name) - 1);

#ifndef __KERNEL_HOST_TEST__
                if (adev->mmio_base) {
                    page_table_t *pml4 = vmm_get_kernel_pml4();
                    if (pml4) {
                        for (uint32_t p = 0; p < 4; p++) {
                            uint64_t a = adev->mmio_base + ((uint64_t)p * 4096ULL);
                            vmm_map_page(pml4, a, a, PAGE_PRESENT | PAGE_WRITABLE);
                        }
                    }

                    volatile uint16_t *gcap = (volatile uint16_t *)(uintptr_t)(adev->mmio_base + HDA_REG_GCAP);
                    uint16_t cap_val = *gcap;
                    adev->num_oss = (cap_val >> 12) & 0x0F;
                    adev->num_iss = (cap_val >> 8) & 0x0F;
                    adev->num_bss = (cap_val >> 3) & 0x1F;

                    volatile uint8_t *vmaj = (volatile uint8_t *)(uintptr_t)(adev->mmio_base + HDA_REG_VMAJ);
                    volatile uint8_t *vmin = (volatile uint8_t *)(uintptr_t)(adev->mmio_base + HDA_REG_VMIN);
                    adev->major_rev = *vmaj;
                    adev->minor_rev = *vmin;

                    volatile uint16_t *statests = (volatile uint16_t *)(uintptr_t)(adev->mmio_base + HDA_REG_STATESTS);
                    adev->codec_mask = *statests;
                }
#else
                adev->num_oss = 4;
                adev->num_iss = 4;
                adev->num_bss = 0;
                adev->major_rev = 1;
                adev->minor_rev = 0;
                adev->codec_mask = 0x0001;
#endif
            } else if (pdev->subclass == 0x01) {
                // AC97 Audio
                adev->type = AUDIO_TYPE_AC97;
                adev->io_mixer = (uint16_t)pci_get_bar_address(pdev, 0);
                adev->io_busmaster = (uint16_t)pci_get_bar_address(pdev, 1);
                strncpy(adev->name, "AC97 Audio Controller", sizeof(adev->name) - 1);
            } else {
                adev->type = AUDIO_TYPE_UNKNOWN;
                strncpy(adev->name, "Generic Audio Controller", sizeof(adev->name) - 1);
            }

            kinfo("Audio: Detected %s [%04x:%04x] at PCI %02x:%02x.%d (MMIO=0x%lx, Streams: %u In / %u Out)",
                  audio_type_to_str(adev->type), adev->vendor_id, adev->device_id,
                  adev->bus, adev->dev, adev->func, adev->mmio_base,
                  adev->num_iss, adev->num_oss);

            purple_detect_register(PURPLE_CAT_AUDIO, adev->name, pci_vendor_to_str(adev->vendor_id),
                                   audio_type_to_str(adev->type), "PCI",
                                   adev->mmio_base ? adev->mmio_base : (uint64_t)adev->io_mixer,
                                   adev->irq, PURPLE_STATE_ACTIVE, PURPLE_DEV_FLAG_BUILTIN);
        }
    }

    if (audio_dev_count == 0) {
        kinfo("Audio: No PCI audio controllers detected on current system");
    } else {
        kinfo("Audio: %u Audio Controller(s) registered and ready", audio_dev_count);
    }
    return 0;
}

uint32_t audio_get_device_count(void) {
    return audio_dev_count;
}

const audio_device_t *audio_get_device(uint32_t index) {
    if (index >= audio_dev_count) return NULL;
    return &audio_devices[index];
}

void audio_dump_devices(void) {
    kprintf("\n=== Audio Controller Inventory (%u device(s)) ===\n", audio_dev_count);
    if (audio_dev_count == 0) {
        kprintf("  No audio controllers detected.\n\n");
        return;
    }

    kprintf("  ID  TYPE                  NAME                      PCI BDF   VER   STREAMS (IN/OUT)  BASE\n");
    kprintf("  ----------------------------------------------------------------------------------------\n");
    for (uint32_t i = 0; i < audio_dev_count; i++) {
        const audio_device_t *a = &audio_devices[i];
        kprintf("  %-2u  %-20s  %-24s  %02x:%02x.%-2d  v%u.%u  %-3u / %-3u         %s=0x%lx\n",
                a->id, audio_type_to_str(a->type), a->name,
                a->bus, a->dev, a->func,
                a->major_rev, a->minor_rev,
                a->num_iss, a->num_oss,
                a->mmio_base ? "MMIO" : "IO",
                a->mmio_base ? a->mmio_base : (uint64_t)a->io_mixer);
    }
    kprintf("\n");
}
