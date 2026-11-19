"""Render original Purple Shell glyphs as antialiased alpha masks."""
from pathlib import Path

from PIL import Image, ImageDraw

SIZE = 24
SCALE = 5
OUTPUT = Path(__file__).resolve().parents[1] / "assets/icons/shell-icons.alpha"


def glyph(name):
    image = Image.new("L", (SIZE * SCALE, SIZE * SCALE))
    draw = ImageDraw.Draw(image)

    def scaled(values):
        return tuple(round(value * SCALE) for value in values)

    def line(*points):
        draw.line([scaled(point) for point in points], fill=255, width=2 * SCALE, joint="curve")
        for x, y in points:
            draw.ellipse(scaled((x - .85, y - .85, x + .85, y + .85)), fill=255)

    def ring(box):
        draw.ellipse(scaled(box), outline=255, width=2 * SCALE)

    def arc(box, start, end):
        draw.arc(scaled(box), start, end, fill=255, width=2 * SCALE)

    if name == "terminal":
        line((5, 6), (11, 12), (5, 18))
        line((13, 18), (20, 18))
    elif name == "files":
        line((3, 8), (3, 5), (10, 5), (13, 8), (21, 8), (21, 19), (3, 19), (3, 8))
        line((3, 10), (21, 10))
    elif name == "resources":
        line((2, 12), (7, 12), (10, 5), (14, 19), (17, 12), (22, 12))
    elif name == "code":
        line((7, 7), (2, 12), (7, 17))
        line((17, 7), (22, 12), (17, 17))
        line((14, 4), (10, 20))
    elif name == "settings":
        ring((4, 4, 20, 20))
        ring((9, 9, 15, 15))
        for a, b in [((12, 2), (12, 5)), ((12, 19), (12, 22)), ((2, 12), (5, 12)),
                     ((19, 12), (22, 12)), ((5, 5), (7, 7)), ((17, 17), (19, 19)),
                     ((5, 19), (7, 17)), ((17, 7), (19, 5))]:
            line(a, b)
    elif name == "wifi":
        arc((-1, 5, 25, 28), 221, 319)
        arc((4, 10, 20, 25), 220, 320)
        arc((8, 15, 16, 23), 220, 320)
        line((12, 20), (12, 20))
    elif name == "bluetooth":
        line((6, 6), (17, 17), (12, 22), (12, 2), (17, 7), (6, 18))
    elif name == "power":
        arc((3, 4, 21, 22), 312, 588)
        line((12, 2), (12, 12))
    elif name == "chevron":
        line((7, 10), (12, 15), (17, 10))
    elif name == "restart":
        arc((3, 3, 21, 21), 220, 550)
        line((3, 3), (3, 10), (10, 10))

    return image.resize((SIZE, SIZE), Image.Resampling.LANCZOS).tobytes()


if __name__ == "__main__":
    OUTPUT.write_bytes(b"".join(glyph(name) for name in (
        "terminal", "files", "resources", "code", "settings",
        "wifi", "bluetooth", "power", "chevron", "restart",
    )))
