#!/usr/bin/env python3
"""Generate the Lycoris Wallet icon set.

The mark is a five-petal lycoris flower in vermilion on warm paper, drawn at
4x and downsampled for clean edges. Run from the repository root:

    python tools/make_icons.py
"""

import math
import pathlib

from PIL import Image, ImageDraw

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT = ROOT / "src-tauri" / "icons"
SIZE = 1024
SS = 4  # supersampling factor

PAPER = (247, 241, 232)
VERMILION = (211, 47, 47)
VERMILION_DARK = (150, 28, 34)
INK = (42, 41, 40)


def petal(draw, cx, cy, angle_deg, length, width, color):
    """One petal: a lens shape built from two curves, rotated about the centre."""
    angle = math.radians(angle_deg)
    steps = 64
    points = []
    for sign in (1.0, -1.0):
        indices = range(steps + 1) if sign > 0 else range(steps, -1, -1)
        for i in indices:
            u = i / steps  # fraction of the petal length
            r = length * u
            half = width * 0.5 * math.sin(math.pi * (u**0.85))
            x = r * math.cos(angle) - half * sign * math.sin(angle)
            y = r * math.sin(angle) + half * sign * math.cos(angle)
            points.append((cx + x, cy + y))
    draw.polygon(points, fill=color)


def render(size: int) -> Image.Image:
    canvas = size * SS
    # 圆角块之外保持透明；透明像素也填纸张色，LANCZOS 缩小后边缘不会发黑。
    image = Image.new("RGBA", (canvas, canvas), PAPER + (0,))
    draw = ImageDraw.Draw(image)

    # subtle inner border so the mark survives on both light and dark shells
    margin = canvas * 0.055
    draw.rounded_rectangle(
        [margin, margin, canvas - margin, canvas - margin],
        radius=canvas * 0.22,
        fill=PAPER + (255,),
        outline=(226, 214, 198, 255),
        width=int(canvas * 0.012),
    )

    cx = cy = canvas / 2
    length = canvas * 0.30
    width = canvas * 0.115

    for index in range(5):
        angle = -90 + index * 72
        petal(draw, cx, cy, angle, length * 1.06, width * 1.12, VERMILION_DARK)
        petal(draw, cx, cy, angle, length, width, VERMILION)

    core = canvas * 0.062
    draw.ellipse([cx - core, cy - core, cx + core, cy + core], fill=INK)
    ring = canvas * 0.098
    draw.ellipse(
        [cx - ring, cy - ring, cx + ring, cy + ring],
        outline=INK,
        width=int(canvas * 0.012),
    )

    return image.resize((size, size), Image.LANCZOS)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    master = render(SIZE)
    master.save(OUT / "icon.png")
    for size, name in [
        (32, "32x32.png"),
        (128, "128x128.png"),
        (256, "128x128@2x.png"),
        (512, "icon-512.png"),
        (150, "Square150x150Logo.png"),
        (44, "Square44x44Logo.png"),
    ]:
        master.resize((size, size), Image.LANCZOS).save(OUT / name)
    master.save(
        OUT / "icon.ico",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    print(f"wrote icons to {OUT}")


if __name__ == "__main__":
    main()
