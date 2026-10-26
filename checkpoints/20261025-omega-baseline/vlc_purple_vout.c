/*****************************************************************************
 * purple.c: Purple OS native video output display module for VLC
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
# define MODULE_STRING "purple"
#endif

#include <vlc_common.h>
#include <vlc_plugin.h>
#include <vlc_vout_display.h>

#ifndef N_
# define N_(s) s
#endif

#include "purple_abi.h"

/*****************************************************************************
 * Module descriptor
 *****************************************************************************/

static int Open(vout_display_t *vd,
                video_format_t *fmtp, vlc_video_context *context);
static void Close(vout_display_t *vd);

vlc_module_begin()
    set_shortname(N_("Purple"))
    set_description(N_("Purple OS Desktop Video Output"))
    set_subcategory(SUBCAT_VIDEO_VOUT)
    set_capability("vout display", 120)
    set_callback_display(Open, 0)
    add_shortcut("purple", "purpleos", "aegis64")
vlc_module_end()

typedef struct vout_display_sys_t {
    purple_window_t *window;
    purple_surface_t surface;
    uint32_t         width;
    uint32_t         height;
    uint32_t         frames_displayed;
} vout_display_sys_t;

static void Display(vout_display_t *vd, picture_t *picture) {
    vout_display_sys_t *sys = vd->sys;
    if (!sys || !sys->window) return;

    /* Copy decoded frame picture plane into Purple OS window surface */
    if (picture && picture->p[0].p_pixels) {
        uint32_t src_pitch = picture->p[0].i_pitch;
        uint32_t src_lines = picture->p[0].i_lines;
        uint32_t copy_lines = (src_lines < sys->surface.height) ? src_lines : sys->surface.height;
        uint32_t copy_bytes = (src_pitch < sys->surface.pitch) ? src_pitch : sys->surface.pitch;

        const uint8_t *src = picture->p[0].p_pixels;
        uint8_t *dst = (uint8_t *)sys->surface.pixels;

        for (uint32_t l = 0; l < copy_lines; l++) {
            memcpy(dst + l * sys->surface.pitch, src + l * src_pitch, copy_bytes);
        }

        /* Present to Purple OS Desktop compositor */
        purple_window_present(sys->window);
        sys->frames_displayed++;
    }
}

static const struct vlc_display_operations ops_purple = {
    .close = Close,
    .display = Display,
};

static int Open(vout_display_t *vd,
                video_format_t *fmtp, vlc_video_context *context) {
    (void)context;

    vout_display_sys_t *sys = (vout_display_sys_t *)malloc(sizeof(*sys));
    if (!sys) return VLC_ENOMEM;

    memset(sys, 0, sizeof(*sys));
    sys->width = fmtp->i_visible_width ? fmtp->i_visible_width : 640;
    sys->height = fmtp->i_visible_height ? fmtp->i_visible_height : 360;

    /* Request 32-bit RGBX/BGRX pixel format matching Purple Desktop ABI */
    fmtp->i_chroma = VLC_CODEC_RGBX;

    /* Create or attach native Purple OS window */
    sys->window = purple_window_create("VLC Media Player - Purple OS", 80, 60, sys->width, sys->height);
    if (!sys->window) {
        free(sys);
        return VLC_EGENERIC;
    }

    if (purple_window_get_surface(sys->window, &sys->surface) != 0) {
        purple_window_destroy(sys->window);
        free(sys);
        return VLC_EGENERIC;
    }

    vd->sys = sys;
    vd->ops = &ops_purple;

    msg_Dbg(vd, "Purple OS video output initialized (%ux%u at pitch %u)",
            sys->width, sys->height, sys->surface.pitch);
    return VLC_SUCCESS;
}

static void Close(vout_display_t *vd) {
    vout_display_sys_t *sys = vd->sys;
    if (sys) {
        if (sys->window) {
            purple_window_destroy(sys->window);
            sys->window = NULL;
        }
        free(sys);
        vd->sys = NULL;
    }
}
