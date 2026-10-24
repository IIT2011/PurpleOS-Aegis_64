#ifndef AUDIO_H
#define AUDIO_H

#include "types.h"
#include "pci.h"

#define AUDIO_MAX_DEVICES 4

typedef enum {
    AUDIO_TYPE_UNKNOWN = 0,
    AUDIO_TYPE_AC97    = 1,
    AUDIO_TYPE_HDA     = 2
} audio_type_t;

typedef struct {
    uint32_t id;
    audio_type_t type;
    char name[32];
    uint8_t bus;
    uint8_t dev;
    uint8_t func;
    uint16_t vendor_id;
    uint16_t device_id;
    uint64_t mmio_base;
    uint16_t io_mixer;
    uint16_t io_busmaster;
    uint32_t irq;
    uint8_t  major_rev;
    uint8_t  minor_rev;
    uint8_t  num_iss;    /* Input Streams */
    uint8_t  num_oss;    /* Output Streams */
    uint8_t  num_bss;    /* Bidirectional Streams */
    uint16_t codec_mask; /* Detected codecs */
    int active;
} audio_device_t;

int audio_init(void);
uint32_t audio_get_device_count(void);
const audio_device_t *audio_get_device(uint32_t index);
void audio_dump_devices(void);

#endif /* AUDIO_H */
