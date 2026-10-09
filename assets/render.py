# /// script
# requires-python = ">=3.10"
# dependencies = ["pillow"]
# ///
"""Render frame dumps from `tests/record.rs` (and `examples/simulate.rs`) to
PNG (one frame) or GIF (several).

    uv run assets/render.py assets/*.frames

Braille cells are drawn as round dots rather than font glyphs, so the images
look the same whatever fonts are installed. Text needs DejaVu Sans Mono; set
GIZMO_FONT_DIR if it isn't found.
"""

import os
import re
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

CW, CH = 12, 24  # cell size in pixels: braille dots on a 6 px grid
PAD = 16
FPS = 25
BG = (30, 30, 46)
NAMED = {  # Catppuccin Mocha
    "Reset": (205, 214, 244),
    "White": (255, 255, 255),
    "Gray": (166, 173, 200),
    "DarkGray": (108, 112, 134),
    "LightMagenta": (245, 194, 231),
    "Magenta": (203, 166, 247),
    "Cyan": (137, 220, 235),
    "Green": (166, 227, 161),
    "Yellow": (249, 226, 175),
    "Red": (243, 139, 168),
}
BITS = [(0, 0, 0x01), (0, 1, 0x02), (0, 2, 0x04), (1, 0, 0x08), (1, 1, 0x10), (1, 2, 0x20), (0, 3, 0x40), (1, 3, 0x80)]


def find_font(name):
    dirs = [os.environ.get("GIZMO_FONT_DIR", ""), "~/Library/Fonts", "/Library/Fonts", "/usr/share/fonts"]
    for d in dirs:
        for path in Path(d).expanduser().rglob(name) if d else []:
            return str(path)
    sys.exit(f"{name} not found; set GIZMO_FONT_DIR")


FONTS = {
    flags: ImageFont.truetype(find_font(file), 20)
    for flags, file in [
        ("", "DejaVuSansMono.ttf"),
        ("b", "DejaVuSansMono-Bold.ttf"),
        ("i", "DejaVuSansMono-Oblique.ttf"),
        ("bi", "DejaVuSansMono-BoldOblique.ttf"),
    ]
}


def color(fg):
    m = re.match(r"Rgb\((\d+), (\d+), (\d+)\)", fg)
    return tuple(map(int, m.groups())) if m else NAMED.get(fg, NAMED["Reset"])


def parse(path):
    frames = []
    for line in open(path, encoding="utf-8"):
        parts = line.rstrip("\n").split("\t")
        if parts[0] == "frame":
            frames.append((int(parts[1]), int(parts[2]), []))
        else:
            frames[-1][2].append(parts)
    return frames


def draw(w, h, cells):
    scale = 2  # supersample for smooth dots
    img = Image.new("RGB", ((w * CW + 2 * PAD) * scale, (h * CH + 2 * PAD) * scale), BG)
    d = ImageDraw.Draw(img)
    text = []
    for x, y, sym, fg, flags in cells:
        x, y, col = int(x), int(y), color(fg)
        o = ord(sym[0])
        if 0x2800 <= o <= 0x28FF:
            for dx, dy, bit in BITS:
                if (o - 0x2800) & bit:
                    cx = (PAD + x * CW + dx * 6 + 3) * scale
                    cy = (PAD + y * CH + dy * 6 + 3) * scale
                    r = 2.3 * scale
                    d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=col)
        else:
            text.append((x, y, sym, col, flags))
    img = img.resize((img.width // scale, img.height // scale), Image.LANCZOS)
    d = ImageDraw.Draw(img)
    for x, y, sym, col, flags in text:
        d.text((PAD + x * CW, PAD + y * CH + 1), sym, fill=col, font=FONTS[flags])
    return img


def main():
    for path in map(Path, sys.argv[1:]):
        frames = [draw(*f) for f in parse(path)]
        if len(frames) == 1:
            out = path.with_suffix(".png")
            frames[0].save(out, optimize=True)
        else:
            out = path.with_suffix(".gif")
            frames = [f.quantize(colors=128, method=Image.Quantize.MEDIANCUT) for f in frames]
            frames[0].save(out, save_all=True, append_images=frames[1:], duration=1000 // FPS, loop=0, optimize=True)
        print(f"{out}  {len(frames)} frame(s)  {out.stat().st_size // 1024} KiB")


if __name__ == "__main__":
    main()
