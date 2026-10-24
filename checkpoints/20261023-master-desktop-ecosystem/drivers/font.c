#include "font.h"
#include "font_data.h"
#include "video.h"
#include "string.h"

static int font_subsystem_initialized = 0;

void font_init(void) {
    font_subsystem_initialized = 1;
}

const font_t *font_get(font_id_t id) {
    switch (id) {
        case FONT_UI_REGULAR: return &font_ui_reg_font;
        case FONT_UI_BOLD:    return &font_ui_bold_font;
        case FONT_UI_TITLE:   return &font_ui_title_font;
        case FONT_MONO_REG:   return &font_mono_reg_font;
        case FONT_MONO_BOLD:  return &font_mono_bold_font;
        case FONT_CLOCK_HERO: return &font_clock_hero_font;
        default:              return &font_ui_reg_font;
    }
}

const font_glyph_t *font_lookup_glyph(const font_t *font, uint32_t codepoint) {
    if (!font) return NULL;

    /* 1. Fast O(1) ASCII lookup for range 32..126 */
    if (codepoint >= 32 && codepoint <= 126 && font->ascii_map) {
        uint16_t idx = font->ascii_map[codepoint - 32];
        if (idx != 0xFFFF && idx < font->glyph_count) {
            return &font->glyphs[idx];
        }
    }

    /* 2. Binary search across sorted Unicode glyph table */
    if (font->glyph_count > 0 && font->glyphs) {
        int low = 0;
        int high = (int)font->glyph_count - 1;
        while (low <= high) {
            int mid = low + (high - low) / 2;
            uint32_t cp = font->glyphs[mid].codepoint;
            if (cp == codepoint) {
                return &font->glyphs[mid];
            } else if (cp < codepoint) {
                low = mid + 1;
            } else {
                high = mid - 1;
            }
        }
    }

    /* 3. Fallback to default replacement glyph */
    if (font->default_glyph_idx < font->glyph_count) {
        return &font->glyphs[font->default_glyph_idx];
    }
    return font->glyph_count > 0 ? &font->glyphs[0] : NULL;
}

int font_decode_utf8(const char **s, uint32_t *codepoint) {
    if (!s || !*s || !**s) {
        if (codepoint) *codepoint = 0;
        return 0;
    }

    const uint8_t *p = (const uint8_t *)*s;
    uint8_t b0 = p[0];

    if (b0 < 0x80) {
        /* 1-byte ASCII */
        if (codepoint) *codepoint = b0;
        *s += 1;
        return 1;
    } else if ((b0 & 0xE0) == 0xC0) {
        /* 2-byte sequence */
        uint8_t b1 = p[1];
        if ((b1 & 0xC0) == 0x80) {
            if (codepoint) *codepoint = ((b0 & 0x1F) << 6) | (b1 & 0x3F);
            *s += 2;
            return 2;
        }
    } else if ((b0 & 0xF0) == 0xE0) {
        /* 3-byte sequence */
        uint8_t b1 = p[1];
        uint8_t b2 = p[2];
        if ((b1 & 0xC0) == 0x80 && (b2 & 0xC0) == 0x80) {
            if (codepoint) *codepoint = ((b0 & 0x0F) << 12) | ((b1 & 0x3F) << 6) | (b2 & 0x3F);
            *s += 3;
            return 3;
        }
    } else if ((b0 & 0xF8) == 0xF0) {
        /* 4-byte sequence */
        uint8_t b1 = p[1];
        uint8_t b2 = p[2];
        uint8_t b3 = p[3];
        if ((b1 & 0xC0) == 0x80 && (b2 & 0xC0) == 0x80 && (b3 & 0xC0) == 0x80) {
            if (codepoint) *codepoint = ((b0 & 0x07) << 18) | ((b1 & 0x3F) << 12) |
                                        ((b2 & 0x3F) << 6)  | (b3 & 0x3F);
            *s += 4;
            return 4;
        }
    }

    /* Malformed sequence: advance 1 byte and return fallback '?' */
    if (codepoint) *codepoint = '?';
    *s += 1;
    return 1;
}

int32_t font_measure_string(const font_t *font, const char *s) {
    if (!font || !s) return 0;
    int32_t total_width = 0;
    const char *p = s;
    uint32_t cp = 0;

    while (font_decode_utf8(&p, &cp)) {
        const font_glyph_t *g = font_lookup_glyph(font, cp);
        if (g) {
            total_width += g->advance_x;
        }
    }
    return total_width;
}

int32_t font_measure_string_n(const font_t *font, const char *s, size_t n) {
    if (!font || !s || n == 0) return 0;
    int32_t total_width = 0;
    const char *p = s;
    uint32_t cp = 0;
    size_t count = 0;

    while (count < n && font_decode_utf8(&p, &cp)) {
        const font_glyph_t *g = font_lookup_glyph(font, cp);
        if (g) {
            total_width += g->advance_x;
        }
        count++;
    }
    return total_width;
}

int font_draw_char(const font_t *font, int32_t x, int32_t y, uint32_t codepoint, uint32_t color, const font_rect_t *clip) {
    if (!font) return 0;
    const font_glyph_t *g = font_lookup_glyph(font, codepoint);
    if (!g) return 0;

    if (!video_is_available() || g->width == 0 || g->height == 0 || !g->bitmap) {
        return g->advance_x;
    }

    uint32_t scr_w = video_get_width();
    uint32_t scr_h = video_get_height();

    int32_t gx = x + g->bearing_x;
    int32_t gy = y + g->bearing_y;

    uint32_t r_fg = (color >> 16) & 0xFF;
    uint32_t g_fg = (color >> 8) & 0xFF;
    uint32_t b_fg = color & 0xFF;

    for (int32_t r = 0; r < g->height; r++) {
        int32_t py = gy + r;
        if (py < 0 || (uint32_t)py >= scr_h) continue;
        if (clip) {
            if (py < clip->y || py >= clip->y + clip->height) continue;
        }

        const uint8_t *row_src = g->bitmap + r * g->width;

        for (int32_t c = 0; c < g->width; c++) {
            uint8_t alpha = row_src[c];
            if (alpha == 0) continue;

            int32_t px = gx + c;
            if (px < 0 || (uint32_t)px >= scr_w) continue;
            if (clip) {
                if (px < clip->x || px >= clip->x + clip->width) continue;
            }

            if (alpha == 255) {
                video_draw_pixel((uint32_t)px, (uint32_t)py, color);
            } else {
                /* Alpha blend with background pixel */
                uint32_t bg = video_get_pixel((uint32_t)px, (uint32_t)py);
                uint32_t r_bg = (bg >> 16) & 0xFF;
                uint32_t g_bg = (bg >> 8) & 0xFF;
                uint32_t b_bg = bg & 0xFF;

                uint32_t blended_r = (r_fg * alpha + r_bg * (255 - alpha)) / 255;
                uint32_t blended_g = (g_fg * alpha + g_bg * (255 - alpha)) / 255;
                uint32_t blended_b = (b_fg * alpha + b_bg * (255 - alpha)) / 255;

                video_draw_pixel((uint32_t)px, (uint32_t)py,
                                 (blended_r << 16) | (blended_g << 8) | blended_b);
            }
        }
    }

    return g->advance_x;
}

int font_draw_string(const font_t *font, int32_t x, int32_t y, const char *text, uint32_t color, const font_rect_t *clip) {
    if (!font || !text) return 0;
    const char *p = text;
    uint32_t cp = 0;
    int32_t cur_x = x;

    while (font_decode_utf8(&p, &cp)) {
        cur_x += font_draw_char(font, cur_x, y, cp, color, clip);
    }
    return cur_x - x;
}

int font_draw_string_bg(const font_t *font, int32_t x, int32_t y, const char *text, uint32_t fg, uint32_t bg, const font_rect_t *clip) {
    if (!font || !text) return 0;

    int32_t text_w = font_measure_string(font, text);
    int32_t line_h = font->ascent + font->descent + font->line_gap;

    /* Fill background box */
    if (video_is_available() && text_w > 0 && line_h > 0) {
        int32_t bx = x;
        int32_t by = y;
        int32_t bw = text_w;
        int32_t bh = line_h;

        if (clip) {
            if (bx < clip->x) { bw -= (clip->x - bx); bx = clip->x; }
            if (by < clip->y) { bh -= (clip->y - by); by = clip->y; }
            if (bx + bw > clip->x + clip->width) bw = (clip->x + clip->width) - bx;
            if (by + bh > clip->y + clip->height) bh = (clip->y + clip->height) - by;
        }

        if (bw > 0 && bh > 0) {
            video_draw_rect((uint32_t)bx, (uint32_t)by, (uint32_t)bw, (uint32_t)bh, bg);
        }
    }

    return font_draw_string(font, x, y, text, fg, clip);
}

int font_draw_string_glow(const font_t *font, int32_t x, int32_t y, const char *text, uint32_t core_color, uint32_t glow_color, int glow_radius, const font_rect_t *clip) {
    if (!font || !text) return 0;

    /* Render outer glow passes */
    if (glow_radius >= 1) {
        for (int dy = -glow_radius; dy <= glow_radius; dy++) {
            for (int dx = -glow_radius; dx <= glow_radius; dx++) {
                if (dx == 0 && dy == 0) continue;
                font_draw_string(font, x + dx, y + dy, text, glow_color, clip);
            }
        }
    }

    /* Render crisp core */
    return font_draw_string(font, x, y, text, core_color, clip);
}
