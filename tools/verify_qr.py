#!/usr/bin/env python3
"""Independently verify Lycoris Wallet QR encoding.

Reads the module matrices emitted by
`cargo run -p lycoris-core --example qr_fixtures`, rasterises them to images and
asks an unrelated decoder (zbar, through pyzbar) to read the payload back.
When `segno` is installed it also compares the symbol size with an
independent encoder: our encoder's mode optimiser may add segmentation
headers and pick one version larger than the byte-minimum, which is legal,
but a symbol smaller than the theoretical minimum would mean the payload
does not fit the capacity it was packed into.

Usage:
    python tools/verify_qr.py [fixtures.txt]
"""

import math
import subprocess
import sys
from pathlib import Path

try:
    from PIL import Image
    from pyzbar.pyzbar import decode
except ImportError as error:  # pragma: no cover - environment issue
    print(f"missing dependency: {error}\n  pip install pillow pyzbar")
    sys.exit(2)

try:  # optional: cross-check the chosen version with a second encoder
    import segno
except ImportError:  # pragma: no cover - optional dependency
    segno = None

SCALE = 10
QUIET_ZONE = 4


def render(matrix: str) -> Image.Image:
    size = int(math.isqrt(len(matrix)))
    if size * size != len(matrix):
        raise ValueError(f"matrix of {len(matrix)} modules is not a square")
    extent = (size + QUIET_ZONE * 2) * SCALE
    image = Image.new("L", (extent, extent), 255)
    pixels = image.load()
    for y in range(size):
        for x in range(size):
            if matrix[y * size + x] == "1":
                for dy in range(SCALE):
                    for dx in range(SCALE):
                        pixels[(x + QUIET_ZONE) * SCALE + dx, (y + QUIET_ZONE) * SCALE + dy] = 0
    return image


def parse(text: str):
    fixtures = []
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("qr_"):
            continue
        name, _, value = line.partition("=")
        payload, _, rest = value.partition("|")
        width_text, _, matrix = rest.partition("|")
        fixtures.append((name[len("qr_") :], payload, int(width_text), matrix))
    return fixtures


def main() -> int:
    if len(sys.argv) > 1:
        text = Path(sys.argv[1]).read_text(encoding="utf-8")
    else:
        result = subprocess.run(
            ["cargo", "run", "-q", "-p", "lycoris-core", "--example", "qr_fixtures"],
            capture_output=True,
            text=True,
            check=True,
        )
        text = result.stdout

    fixtures = parse(text)
    if not fixtures:
        print("no qr_ fixtures found")
        return 1

    failures = []
    for name, payload, width, matrix in fixtures:
        notes = []
        if width * width != len(matrix):
            failures.append(f"{name}: declared width {width} does not match {len(matrix)} modules")
            continue
        decoded = decode(render(matrix))
        text_value = decoded[0].data.decode("utf-8") if decoded else None
        notes.append(f"decoded={text_value == payload}")
        if segno is not None:
            minimum = segno.make(payload, error="m").symbol_size(scale=1, border=0)[0]
            notes.append(f"size={width}>={minimum}:{width >= minimum}")
            if width < minimum:
                failures.append(f"{name}: version {width} < minimum {minimum}")
        if text_value != payload:
            failures.append(f"{name}: decoder returned {text_value!r}")
        print(f"  {'ok  ' if text_value == payload else 'FAIL'} {name:9} {width}x{width}  " + "  ".join(notes))

    if failures:
        print(f"\n{len(failures)} problems: {'; '.join(failures)}")
        return 1
    print("\nall QR fixtures decode")
    return 0


if __name__ == "__main__":
    sys.exit(main())
