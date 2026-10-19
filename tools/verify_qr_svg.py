#!/usr/bin/env python3
"""Independently verify the *rendered* Lycoris Wallet QR SVG.

`tools/verify_qr.py` checks the module matrix, i.e. what the encoder produced.
This script goes one step further: it takes the actual SVG the app puts on
screen (rounded finder patterns, rounded data tiles), parses the geometry out
of it, rasterises it and asks an unrelated scanner (zbar, through pyzbar) to
read the payload back.

Both palettes are checked, because the dark theme renders an inverted symbol
(light modules on a dark ground) and that must still scan.

Usage:
    python tools/verify_qr_svg.py [fixtures.txt]
"""

import math
import re
import subprocess
import sys
from pathlib import Path

try:
    from PIL import Image, ImageDraw
    from pyzbar.pyzbar import decode
except ImportError as error:  # pragma: no cover - environment issue
    print(f"missing dependency: {error}\n  pip install pillow pyzbar")
    sys.exit(2)

RECT = re.compile(
    r'<rect x="([\d.]+)" y="([\d.]+)" width="([\d.]+)" height="([\d.]+)"'
    r'(?: rx="([\d.]+)")?(?: fill="none"(?: stroke="([^"]+)")? stroke-width="([\d.]+)")?'
)
BACKGROUND = re.compile(r'<rect width="[\d.]+" height="[\d.]+" fill="([^"]+)"')
# The sizes a phone camera realistically has to read the symbol from a screen:
# the app shows it up to ~320 CSS pixels wide; on a 360 px phone the card
# still leaves ~300 px, so 288 px is the floor worth testing.
SCREEN_SIZES = (320, 288)


def parse_color(value: str):
    value = value.lstrip("#")
    if len(value) == 3:
        value = "".join(ch * 2 for ch in value)
    return tuple(int(value[i : i + 2], 16) for i in (0, 2, 4))


def rasterise(svg: str, scale: int = 3, pixels: float = None) -> Image.Image:
    """Draw the SVG with PIL: filled rounded rects, hollow rings and the base."""
    background = parse_color(BACKGROUND.search(svg).group(1))
    group = re.search(r'<g fill="([^"]+)">(.*)</g>', svg, re.S)
    fill = parse_color(group.group(1))
    body = group.group(2)

    # The viewBox defines the canvas; every shape is scaled up for the decoder.
    view = re.search(r'viewBox="0 0 ([\d.]+) ([\d.]+)"', svg)
    extent = float(view.group(1))
    if pixels is not None:
        scale = pixels / extent

    image = Image.new("RGB", (int(extent * scale), int(extent * scale)), background)
    draw = ImageDraw.Draw(image)

    def rounded(x, y, w, h, radius, colour):
        draw.rounded_rectangle(
            [x * scale, y * scale, (x + w) * scale, (y + h) * scale],
            radius=max(0.0, radius * scale),
            fill=colour,
        )

    for match in RECT.finditer(body):
        x, y, w, h = (float(match.group(i)) for i in (1, 2, 3, 4))
        radius = float(match.group(5) or 0)
        stroke_colour = match.group(6)
        stroke = match.group(7)

        if stroke:
            # A ring: the outer boundary is the rect grown by half the stroke
            # width, the hole is the rect shrunk by the same amount. An SVG
            # stroke without a colour is not drawn at all, so a missing
            # `stroke="…"` here has to stay invisible — that is exactly how a
            # browser would render it.
            if not stroke_colour:
                continue
            half = float(stroke) / 2
            thickness = float(stroke)
            colour = parse_color(stroke_colour)
            rounded(x - half, y - half, w + thickness, h + thickness, radius + half, colour)
            rounded(
                x + half,
                y + half,
                w - thickness,
                h - thickness,
                max(0.0, radius - half),
                background,
            )
        else:
            rounded(x, y, w, h, radius, fill)
    return image


def parse(text: str):
    fixtures = []
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("qrsvg_"):
            continue
        name, _, value = line.partition("=")
        payload, _, svg = value.partition("|")
        fixtures.append((name[len("qrsvg_") :], payload, svg))
    return fixtures


def main() -> int:
    if len(sys.argv) > 1:
        text = Path(sys.argv[1]).read_text(encoding="utf-8")
    else:
        result = subprocess.run(
            [
                "cargo",
                "run",
                "-q",
                "-p",
                "lycoris-core",
                "--example",
                "qr_svg_fixtures",
            ],
            capture_output=True,
            text=True,
            check=True,
        )
        text = result.stdout

    fixtures = parse(text)
    if not fixtures:
        print("no qrsvg_ fixtures found")
        return 1

    failures = []
    for name, payload, svg in fixtures:
        # Decode at the size the symbol is actually shown on screen, and at a
        # couple of smaller sizes: a wallet QR has to scan off a phone screen.
        results = []
        for pixels in SCREEN_SIZES:
            image = rasterise(svg, pixels=pixels)
            decoded = decode(image)
            value = decoded[0].data.decode("utf-8") if decoded else None
            results.append((pixels, value == payload))
        ok = all(success for _, success in results)
        summary = "  ".join(f"{px}px:{'ok' if good else 'FAIL'}" for px, good in results)
        print(f"  {'ok  ' if ok else 'FAIL'} {name:22} {summary}")
        if not ok:
            failures.append(f"{name}: failed at {[px for px, good in results if not good]}")

    if failures:
        print(f"\n{len(failures)} problems: {'; '.join(failures)}")
        return 1
    print("\nall rendered QR symbols decode at screen size")
    return 0


if __name__ == "__main__":
    sys.exit(main())
