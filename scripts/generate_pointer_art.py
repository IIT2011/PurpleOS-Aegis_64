"""Rasterize the Zen pointer assets and the Purple Bar mark for the compositor."""
from pathlib import Path
import re
import xml.etree.ElementTree as ET

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
ZEN = ROOT / "assets/cursors/zen"
SCALE = 4
SIZE = 24


def path_points(path):
    """Read the absolute M/L/C/Z commands used by these two upstream sprites."""
    start = current = (0, 0)
    for verb, coords in re.findall(r"([MLCZ])([^MLCZ]*)", path):
        values = [float(value) for value in re.findall(r"-?\d+(?:\.\d+)?", coords)]
        if verb == "C":
            x0, y0 = current
            x1, y1, x2, y2, x3, y3 = values
            for step in range(1, 13):
                t = step / 12
                s = 1 - t
                yield (
                    s**3 * x0 + 3 * s**2 * t * x1 + 3 * s * t**2 * x2 + t**3 * x3,
                    s**3 * y0 + 3 * s**2 * t * y1 + 3 * s * t**2 * y2 + t**3 * y3,
                )
            current = (x3, y3)
        else:
            current = start if verb == "Z" else tuple(values)
            if verb == "M":
                start = current
            yield current


def write_sprite(image, path, size):
    path.write_bytes(image.resize((size, size), Image.Resampling.LANCZOS).tobytes())


def cursor(name, origin):
    image = Image.new("RGBA", (SIZE * SCALE, SIZE * SCALE))
    draw = ImageDraw.Draw(image)
    for path in ET.parse(ZEN / f"{name}.svg").iter("{http://www.w3.org/2000/svg}path"):
        points = [
            ((x - origin[0]) * SCALE, (y - origin[1]) * SCALE)
            for x, y in path_points(path.attrib["d"])
        ]
        if "fill" in path.attrib:
            fill = {"#000000": "#231635", "#FFFFFF": "#F8F1FF"}[path.attrib["fill"]]
            draw.polygon(points, fill=fill)
        if "stroke" in path.attrib:
            width = round(float(path.attrib["stroke-width"]) * SCALE)
            draw.line(points, fill="#231635", width=width, joint="curve")
    write_sprite(image, ZEN / f"{name}.rgba", SIZE)


def logo():
    size = 22
    image = Image.new("RGBA", (size * SCALE, size * SCALE))
    draw = ImageDraw.Draw(image)

    def facet(points, color):
        draw.polygon([(x * SCALE, y * SCALE) for x, y in points], fill=color)

    # Angled stem and a folded ribbon around the counter.
    facet([(4, 5), (8, 1), (8, 17), (4, 21)], "#A66CFF")
    facet([(4, 5), (8, 1), (16, 1), (20, 5)], "#F2DEFF")
    facet([(16, 1), (20, 5), (20, 10), (16, 14), (16, 5)], "#C497FF")
    facet([(8, 10), (16, 10), (20, 10), (16, 14), (8, 14)], "#7945E5")
    write_sprite(image, ROOT / "assets/icons/purple-mark.rgba", size)


if __name__ == "__main__":
    cursor("default", (11, 7))
    cursor("dnd-move", (5.5, 5.5))
    logo()
