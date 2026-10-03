#!/usr/bin/env python3
"""Renders Corvene's Android icon from the geometry of the adaptive icon
layers (app/src/main/res/drawable/ic_launcher_*.xml): the 512 px store icon
(full bleed, no mask or shadow, as Google Play and F-Droid want it) and, with
--preview, the masks a launcher applies.

    python3 packaging/android/icon.py [--preview out.png]

Needs Pillow."""
import math
import sys
from pathlib import Path

from PIL import Image, ImageDraw

SCALE = 0.098  # glyph units (assets/icon/Corvene.svg) per dp
TX, TY = 54 - 489.5 * SCALE, 54 - 512 * SCALE
NODES = [(665, 343), (665, 681), (528, 502)]
TOP, BOTTOM = (0x8A, 0x5C, 0xFF), (0x43, 0x18, 0xB8)
INNER = (0x4C, 0x22, 0xC4)


def layers(px, glyph=(255, 255, 255), inner=INNER):
    """Background and foreground of the 108 dp canvas at `px` pixels."""
    k = px / 108
    background = Image.new("RGBA", (px, px))
    for y in range(px):
        t = y / (px - 1)
        colour = tuple(round(a + (b - a) * t) for a, b in zip(TOP, BOTTOM))
        ImageDraw.Draw(background).line([(0, y), (px, y)], fill=colour + (255,))
    foreground = Image.new("RGBA", (px, px), (0, 0, 0, 0))
    draw = ImageDraw.Draw(foreground)

    def point(x, y):
        return ((TX + x * SCALE) * k, (TY + y * SCALE) * k)

    def dot(x, y, r, fill):
        cx, cy = point(x, y)
        r *= SCALE * k
        draw.ellipse([cx - r, cy - r, cx + r, cy + r], fill=fill)

    def stroke(points):
        for x, y in points:
            dot(x, y, 38, glyph + (255,))

    # the arc: a circle of radius 228 around (512, 512) from the upper node
    # round the left to the lower one, open to the right
    start = math.atan2(343 - 512, 665 - 512)
    sweep = 2 * math.pi + 2 * start
    stroke(
        (512 + 228 * math.cos(a), 512 + 228 * math.sin(a))
        for a in (start - sweep * i / 600 for i in range(601))
    )
    # the branch: a cubic from the middle node to the lower one
    p = [(528, 502), (528, 612), (578, 652), (665, 681)]
    stroke(
        tuple(
            (1 - t) ** 3 * p[0][i]
            + 3 * (1 - t) ** 2 * t * p[1][i]
            + 3 * (1 - t) * t**2 * p[2][i]
            + t**3 * p[3][i]
            for i in (0, 1)
        )
        for t in (i / 300 for i in range(301))
    )
    for x, y in NODES:
        dot(x, y, 68, glyph + (255,))
        if inner:
            dot(x, y, 31, inner + (255,))
    return background, foreground


def store_icon(size=512):
    # Play's icon is the 72 dp centre of the canvas scaled up, unmasked
    px = size * 4 * 108 // 72
    background, foreground = layers(px)
    icon = Image.alpha_composite(background, foreground)
    margin = (px - size * 4) // 2
    return icon.crop((margin, margin, px - margin, px - margin)).resize(
        (size, size), Image.LANCZOS
    )


def preview(path):
    px = 432
    background, foreground = layers(px)
    icon = Image.alpha_composite(background, foreground)
    visible = icon.crop((72, 72, 360, 360))
    sheet = Image.new("RGBA", (288 * 3 + 80, 288 + 40), (32, 33, 36, 255))
    for i, radius in enumerate((144, 80, 48)):
        mask = Image.new("L", (288, 288), 0)
        ImageDraw.Draw(mask).rounded_rectangle([0, 0, 287, 287], radius, fill=255)
        sheet.paste(visible, (20 + i * 308, 20), mask)
    sheet.save(path)


if __name__ == "__main__":
    root = Path(__file__).parent
    if len(sys.argv) == 3 and sys.argv[1] == "--preview":
        preview(sys.argv[2])
    else:
        out = root / "fastlane/metadata/android/en-US/images/icon.png"
        out.parent.mkdir(parents=True, exist_ok=True)
        store_icon().save(out)
        print(out)
