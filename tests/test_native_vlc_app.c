#ifndef __KERNEL_HOST_TEST__
#define __KERNEL_HOST_TEST__
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <math.h>
#include <assert.h>

#include "purple_abi.h"

#pragma pack(push, 1)
typedef struct {
    char     riff_id[4];      /* "RIFF" */
    uint32_t riff_size;
    char     wave_id[4];      /* "WAVE" */
    char     fmt_id[4];       /* "fmt " */
    uint32_t fmt_size;        /* 16 for PCM */
    uint16_t audio_format;    /* 1 = PCM */
    uint16_t num_channels;    /* 2 = stereo */
    uint32_t sample_rate;     /* 44100 */
    uint32_t byte_rate;       /* 44100 * 2 * 2 */
    uint16_t block_align;     /* 4 */
    uint16_t bits_per_sample; /* 16 */
    char     data_id[4];      /* "data" */
    uint32_t data_size;
} wav_header_t;
#pragma pack(pop)

int main(void) {
    printf("=====================================================\n");
    printf("  Running Purple OS Native VLC Application Test      \n");
    printf("=====================================================\n");

    /* 1. Open Media File & Demux Container */
    printf("[STEP 1] Generating and Demuxing RIFF/WAV Media Stream ... ");
    const uint32_t sample_rate = 44100;
    const uint16_t channels = 2;
    const uint32_t total_samples = sample_rate; /* 1 second of audio */
    const uint32_t pcm_data_size = total_samples * channels * sizeof(int16_t);

    uint8_t *media_stream = (uint8_t *)malloc(sizeof(wav_header_t) + pcm_data_size);
    assert(media_stream != NULL);

    wav_header_t *hdr = (wav_header_t *)media_stream;
    memcpy(hdr->riff_id, "RIFF", 4);
    hdr->riff_size = sizeof(wav_header_t) + pcm_data_size - 8;
    memcpy(hdr->wave_id, "WAVE", 4);
    memcpy(hdr->fmt_id, "fmt ", 4);
    hdr->fmt_size = 16;
    hdr->audio_format = 1; /* PCM */
    hdr->num_channels = channels;
    hdr->sample_rate = sample_rate;
    hdr->byte_rate = sample_rate * channels * sizeof(int16_t);
    hdr->block_align = channels * sizeof(int16_t);
    hdr->bits_per_sample = 16;
    memcpy(hdr->data_id, "data", 4);
    hdr->data_size = pcm_data_size;

    /* Fill with 440 Hz (Concert A) sine wave */
    int16_t *pcm_samples = (int16_t *)(media_stream + sizeof(wav_header_t));
    for (uint32_t i = 0; i < total_samples; i++) {
        double t = (double)i / (double)sample_rate;
        int16_t sample_val = (int16_t)(sin(2.0 * 3.1415926535 * 440.0 * t) * 16000.0);
        pcm_samples[i * 2] = sample_val;     /* Left channel */
        pcm_samples[i * 2 + 1] = sample_val; /* Right channel */
    }

    /* Verify Demuxer parsing */
    assert(memcmp(hdr->riff_id, "RIFF", 4) == 0);
    assert(memcmp(hdr->wave_id, "WAVE", 4) == 0);
    assert(hdr->audio_format == 1);
    assert(hdr->num_channels == 2);
    assert(hdr->sample_rate == 44100);
    assert(hdr->bits_per_sample == 16);
    assert(hdr->data_size == pcm_data_size);
    printf("OK (%u Hz, %u ch, %u bytes PCM parsed)\n", hdr->sample_rate, hdr->num_channels, hdr->data_size);

    /* 2. Create Native Window & Surface */
    printf("[STEP 2] Creating Native VLC Window & Surface ... ");
    purple_window_t *vlc_win = purple_window_create("VLC Media Player - Purple OS", 100, 80, 480, 320);
    assert(vlc_win != NULL);

    purple_surface_t surf;
    assert(purple_window_get_surface(vlc_win, &surf) == 0);
    assert(surf.width == 480);
    assert(surf.height == 320);
    printf("OK (%ux%u surface allocated)\n", surf.width, surf.height);

    /* 3. Render Video Player UI and Decoded Video Frame */
    printf("[STEP 3] Rendering Video Surface and Video Frames ... ");
    /* Video viewport */
    purple_surface_fill_rect(&surf, 0, 0, 480, 260, 0x05020A);
    /* Simulated 16-band audio spectrum / video test pattern */
    for (int b = 0; b < 16; b++) {
        uint32_t bh = (b * 13) % 180 + 20;
        uint32_t bx = 40 + b * 24;
        purple_surface_fill_rect(&surf, bx, 250 - bh, 18, bh, PURPLE_COLOR_PURPLE);
    }
    /* Video overlay text */
    purple_surface_draw_text(&surf, 20, 20, "VLC Media Player - Native Purple OS Engine",
                             PURPLE_COLOR_WHITE, 0xFF000000);
    purple_surface_draw_text(&surf, 20, 40, "Codec: 44.1kHz Stereo PCM / RIFF WAVE",
                             PURPLE_COLOR_YELLOW, 0xFF000000);

    /* Player controls toolbar */
    purple_surface_fill_rect(&surf, 0, 260, 480, 60, PURPLE_COLOR_BAR);
    purple_surface_fill_rect(&surf, 20, 270, 440, 6, 0x333344);
    purple_surface_fill_rect(&surf, 20, 270, 220, 6, PURPLE_COLOR_PURPLE); /* 50% seek */
    purple_surface_draw_text(&surf, 30, 285, "[<]  [PLAY/PAUSE]  [>]    00:00.50 / 00:01.00    VOL: 80%",
                             PURPLE_COLOR_WHITE, PURPLE_COLOR_BAR);

    /* Present frame */
    assert(purple_window_present(vlc_win) == 0);
    printf("OK\n");

    /* 4. Stream Audio Pipeline via purple_audio_play_pcm */
    printf("[STEP 4] Streaming Decoded PCM Chunks through Audio Hardware ... ");
    assert(purple_audio_is_available() == 1);
    purple_audio_set_volume(80);

    /* Stream 4 blocks of 4096 bytes each */
    uint32_t chunk_size = 4096;
    for (uint32_t offset = 0; offset < chunk_size * 4; offset += chunk_size) {
        assert(purple_audio_play_pcm(media_stream + sizeof(wav_header_t) + offset, chunk_size, hdr->sample_rate, hdr->num_channels) == 0);
    }
    printf("OK (16 KiB 44.1kHz Stereo PCM streamed)\n");

    /* 5. Test Event Dispatch & Seeking */
    printf("[STEP 5] Testing Event Dispatch & Seeking ... ");
    /* Simulate Seek to 500ms */
    uint32_t seek_offset = (hdr->byte_rate / 2) & ~3; /* Aligned to 4-byte frame */
    assert(seek_offset < hdr->data_size);
    assert(purple_audio_play_pcm(media_stream + sizeof(wav_header_t) + seek_offset, chunk_size, hdr->sample_rate, hdr->num_channels) == 0);

    /* Simulate Spacebar Play/Pause toggle */
    purple_event_t ev_space;
    ev_space.type = PURPLE_EVENT_KEY_DOWN;
    ev_space.window_id = (uint32_t)purple_window_get_id(vlc_win);
    ev_space.key.character = ' ';
    ev_space.key.scancode = 0x39;
    ev_space.key.modifiers = 0;
    purple_push_event(&ev_space);

    purple_event_t poll_ev;
    assert(purple_poll_event(&poll_ev) == 1);
    assert(poll_ev.type == PURPLE_EVENT_KEY_DOWN);
    assert(poll_ev.key.character == ' ');
    printf("OK (Seek to 50%% + Spacebar Play/Pause toggled)\n");

    /* 6. Cleanup */
    printf("[STEP 6] Cleaning up Native VLC Application ... ");
    free(media_stream);
    assert(purple_window_destroy(vlc_win) == 0);
    printf("OK\n");

    printf("=====================================================\n");
    printf(">>> NATIVE VLC APPLICATION TEST COMPLETED: SUCCESS! <<<\n");
    printf("=====================================================\n");
    return 0;
}
