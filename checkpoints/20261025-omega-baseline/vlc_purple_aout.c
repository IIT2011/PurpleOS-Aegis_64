/*****************************************************************************
 * purple.c: Purple OS native audio output module for VLC
 *****************************************************************************
 * Copyright (C) 2026 VLC authors and VideoLAN
 *
 * Authors: Saptanshu Ganguly <saptanshu@purpleos.org>
 *
 * This program is free software; you can redistribute it and/or modify it
 * under the terms of the GNU Lesser General Public License as published by
 * the Free Software Foundation; either version 2.1 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public License
 * along with this program; if not, write to the Free Software Foundation,
 * Inc., 51 Franklin Street, Fifth Floor, Boston MA 02110-1301, USA.
 *****************************************************************************/

#define _GNU_SOURCE 1
#include <string.h>
#include <stdlib.h>

#ifdef HAVE_CONFIG_H
# include "config.h"
#endif

#ifndef MODULE_STRING
# define MODULE_STRING "purpleaudio"
#endif

#include <vlc_common.h>
#include <vlc_plugin.h>
#include <vlc_aout.h>

#ifndef N_
# define N_(s) s
#endif

#include "purple_abi.h"

/*****************************************************************************
 * Module descriptor
 *****************************************************************************/

static int Open(vlc_object_t *);
static void Close(vlc_object_t *);

vlc_module_begin()
    set_shortname(N_("PurpleAudio"))
    set_description(N_("Purple OS Audio Output"))
    set_subcategory(SUBCAT_AUDIO_AOUT)
    set_capability("audio output", 110)
    set_callbacks(Open, Close)
    add_shortcut("purple", "purpleaudio", "aegisaudio")
vlc_module_end()

typedef struct aout_sys_t {
    uint32_t sample_rate;
    uint8_t  channels;
    uint8_t  volume;
    int      device_available;
} aout_sys_t;

static void Play(audio_output_t *aout, block_t *block, vlc_tick_t date) {
    (void)date;
    aout_sys_t *sys = aout->sys;
    if (!sys || !sys->device_available || !block) {
        if (block) block_Release(block);
        return;
    }

    /* Pass decoded PCM samples directly to Purple OS audio hardware */
    purple_audio_play_pcm(block->p_buffer, block->i_buffer, sys->sample_rate, sys->channels);
    block_Release(block);
}

static void Pause(audio_output_t *aout, bool pause, vlc_tick_t date) {
    (void)aout;
    (void)pause;
    (void)date;
}

static void Flush(audio_output_t *aout) {
    (void)aout;
}

static int VolumeSet(audio_output_t *aout, float vol) {
    aout_sys_t *sys = aout->sys;
    if (sys) {
        uint8_t v = (uint8_t)(vol * 100.0f);
        if (v > 100) v = 100;
        sys->volume = v;
        purple_audio_set_volume(v);
        aout_VolumeReport(aout, vol);
    }
    return 0;
}

static int MuteSet(audio_output_t *aout, bool mute) {
    aout_sys_t *sys = aout->sys;
    if (sys) {
        purple_audio_set_volume(mute ? 0 : sys->volume);
        aout_MuteReport(aout, mute);
    }
    return 0;
}

static int Start(audio_output_t *aout, audio_sample_format_t *restrict fmt) {
    aout_sys_t *sys = aout->sys;
    if (!sys) return VLC_EGENERIC;

    sys->sample_rate = fmt->i_rate ? fmt->i_rate : 44100;
    sys->channels = (uint8_t)aout_FormatNbChannels(fmt);
    if (sys->channels == 0) sys->channels = 2;

    fmt->i_format = VLC_CODEC_S16N; /* Standard 16-bit signed native endian PCM */
    sys->device_available = purple_audio_is_available();

    msg_Dbg(aout, "Purple OS audio output started (rate=%u Hz, channels=%u, available=%d)",
            sys->sample_rate, sys->channels, sys->device_available);

    aout_FormatPrepare(fmt);
    return VLC_SUCCESS;
}

static void Stop(audio_output_t *aout) {
    (void)aout;
}

static int Open(vlc_object_t *obj) {
    audio_output_t *aout = (audio_output_t *)obj;

    aout_sys_t *sys = (aout_sys_t *)malloc(sizeof(*sys));
    if (!sys) return VLC_ENOMEM;

    memset(sys, 0, sizeof(*sys));
    sys->volume = 80;
    sys->device_available = purple_audio_is_available();

    aout->sys = sys;
    aout->start = Start;
    aout->stop = Stop;
    aout->play = Play;
    aout->pause = Pause;
    aout->flush = Flush;
    aout->volume_set = VolumeSet;
    aout->mute_set = MuteSet;

    return VLC_SUCCESS;
}

static void Close(vlc_object_t *obj) {
    audio_output_t *aout = (audio_output_t *)obj;
    if (aout && aout->sys) {
        free(aout->sys);
        aout->sys = NULL;
    }
}
