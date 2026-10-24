#ifndef FONT_H
#define FONT_H

#include "types.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    uint32_t codepoint;
    int16_t  width;
    int16_t  height;
    int16_t  bearing_x;
    int16_t  bearing_y;
    int16_t  advance_x;
    const uint8_t *bitmap; /* 8-bit alpha coverage, width * height bytes (0..255) */
} font_glyph_t;

typedef struct {
    const char *name;
    const char *family;
    const char *style;
    int16_t     size_pt;
    int16_t     ascent;
    int16_t     descent;
    int16_t     line_gap;
    int16_t     is_monospace;
    uint32_t    glyph_count;
    const font_glyph_t *glyphs;
    const uint16_t     *ascii_map; /* indices into glyphs for ASCII 32..126, 0xFFFF if missing */
    uint16_t    default_glyph_idx;
} font_t;

typedef enum {
    FONT_UI_REGULAR = 0,   /* Noto Sans Regular 14px */
    FONT_UI_BOLD    = 1,   /* Noto Sans Bold 14px */
    FONT_UI_TITLE   = 2,   /* Noto Sans Bold 16px */
    FONT_MONO_REG   = 3,   /* Noto Sans Mono Regular 15px */
    FONT_MONO_BOLD  = 4,   /* Noto Sans Mono Bold 15px */
    FONT_CLOCK_HERO = 5,   /* Noto Sans Bold 48px */
    FONT_ID_COUNT   = 6
} font_id_t;

typedef struct {
    int32_t x;
    int32_t y;
    int32_t width;
    int32_t height;
} font_rect_t;

/* Core font subsystem management */
void font_init(void);
const font_t *font_get(font_id_t id);
const font_glyph_t *font_lookup_glyph(const font_t *font, uint32_t codepoint);

/* UTF-8 Decoding: returns number of bytes consumed, or 0 on end, with safe error handling */
int font_decode_utf8(const char **s, uint32_t *codepoint);

/* String measurements */
int32_t font_measure_string(const font_t *font, const char *s);
int32_t font_measure_string_n(const font_t *font, const char *s, size_t n);

/* High-quality anti-aliased rendering */
int font_draw_char(const font_t *font, int32_t x, int32_t y, uint32_t codepoint, uint32_t color, const font_rect_t *clip);
int font_draw_string(const font_t *font, int32_t x, int32_t y, const char *text, uint32_t color, const font_rect_t *clip);
int font_draw_string_bg(const font_t *font, int32_t x, int32_t y, const char *text, uint32_t fg, uint32_t bg, const font_rect_t *clip);
int font_draw_string_glow(const font_t *font, int32_t x, int32_t y, const char *text, uint32_t core_color, uint32_t glow_color, int glow_radius, const font_rect_t *clip);

#ifdef __cplusplus
}
#endif

#endif /* FONT_H */
