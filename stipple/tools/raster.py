# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy", "pillow"]
# ///
"""Rasterise `stipple --dump` output to PNG (one frame) or GIF (all frames),
drawing each glyph the way a terminal does: braille as a 2×4 grid of dots,
box drawing as thin lines, cell backgrounds as flat blocks.

    uv run stipple/tools/raster.py frames.txt out.gif [--cell 8x16] [--fps 15]

At the default 8×16 cell, braille dots are 2×2 pixels on a 4 pixel pitch.
"""

import argparse

import numpy as np
from PIL import Image

SS = 4  # supersampling for anti-aliased masks

# The glyph filling exactly the 2×4 sub-blocks in bit pattern i (bit
# row * 2 + column): block elements, quadrants and Unicode 16 octants.
BLOCKS = " 𜺨𜺫🮂𜴀▘𜴁𜴂𜴃𜴄▝𜴅𜴆𜴇𜴈▀𜴉𜴊𜴋𜴌🯦𜴍𜴎𜴏𜴐𜴑𜴒𜴓𜴔𜴕𜴖𜴗𜴘𜴙𜴚𜴛𜴜𜴝𜴞𜴟🯧𜴠𜴡𜴢𜴣𜴤𜴥𜴦𜴧𜴨𜴩𜴪𜴫𜴬𜴭𜴮𜴯𜴰𜴱𜴲𜴳𜴴𜴵🮅𜺣𜴶𜴷𜴸𜴹𜴺𜴻𜴼𜴽𜴾𜴿𜵀𜵁𜵂𜵃𜵄▖𜵅𜵆𜵇𜵈▌𜵉𜵊𜵋𜵌▞𜵍𜵎𜵏𜵐▛𜵑𜵒𜵓𜵔𜵕𜵖𜵗𜵘𜵙𜵚𜵛𜵜𜵝𜵞𜵟𜵠𜵡𜵢𜵣𜵤𜵥𜵦𜵧𜵨𜵩𜵪𜵫𜵬𜵭𜵮𜵯𜵰𜺠𜵱𜵲𜵳𜵴𜵵𜵶𜵷𜵸𜵹𜵺𜵻𜵼𜵽𜵾𜵿𜶀𜶁𜶂𜶃𜶄𜶅𜶆𜶇𜶈𜶉𜶊𜶋𜶌𜶍𜶎𜶏▗𜶐𜶑𜶒𜶓▚𜶔𜶕𜶖𜶗▐𜶘𜶙𜶚𜶛▜𜶜𜶝𜶞𜶟𜶠𜶡𜶢𜶣𜶤𜶥𜶦𜶧𜶨𜶩𜶪𜶫▂𜶬𜶭𜶮𜶯𜶰𜶱𜶲𜶳𜶴𜶵𜶶𜶷𜶸𜶹𜶺𜶻𜶼𜶽𜶾𜶿𜷀𜷁𜷂𜷃𜷄𜷅𜷆𜷇𜷈𜷉𜷊𜷋𜷌𜷍𜷎𜷏𜷐𜷑𜷒𜷓𜷔𜷕𜷖𜷗𜷘𜷙𜷚▄𜷛𜷜𜷝𜷞▙𜷟𜷠𜷡𜷢▟𜷣▆𜷤𜷥█"


def masks(cw, ch, round_dots):
    """Coverage mask in 0..1 for each glyph, at cw × ch pixels."""
    W, H = cw * SS, ch * SS
    yy, xx = np.mgrid[0:H, 0:W] + 0.5
    line = max(1, round(cw / 8)) * SS

    def rect(x0, y0, x1, y1):
        return (xx >= x0 * SS) & (xx < x1 * SS) & (yy >= y0 * SS) & (yy < y1 * SS)

    def hline(x0, x1, y=ch / 2):
        return (xx >= x0 * SS) & (xx < x1 * SS) & (np.abs(yy - y * SS) < line / 2)

    def vline(x, y0=0, y1=ch):
        return (np.abs(xx - x * SS) < line / 2) & (yy >= y0 * SS) & (yy < y1 * SS)

    out = {" ": np.zeros((H, W), bool)}
    bits = [(0, 0, 0x01), (0, 1, 0x02), (0, 2, 0x04), (1, 0, 0x08), (1, 1, 0x10), (1, 2, 0x20), (0, 3, 0x40), (1, 3, 0x80)]
    dots = []
    for c, r, bit in bits:
        cx, cy = cw * (2 * c + 1) / 4, ch * (2 * r + 1) / 8
        if round_dots:
            rad = cw * 0.19
            m = (xx - cx * SS) ** 2 + (yy - cy * SS) ** 2 < (rad * SS) ** 2
        else:
            s = max(1, round(cw / 4))
            m = rect(round(cx - s / 2), round(cy - s / 2), round(cx - s / 2) + s, round(cy - s / 2) + s)
        dots.append((bit, m))
    for code in range(256):
        m = np.zeros((H, W), bool)
        for bit, d in dots:
            if code & bit:
                m |= d
        out[chr(0x2800 + code)] = m
    out.update(
        {
            "│": vline(cw / 2),
            "┃": (np.abs(xx - cw / 2 * SS) < line) & (yy >= 0),
            "╵": vline(cw / 2, 0, ch / 2),
            "╷": vline(cw / 2, ch / 2, ch),
            "▏": rect(0, 0, max(1, cw / 8), ch),
            "─": hline(0, cw),
            "╴": hline(0, cw / 2),
            "╶": hline(cw / 2, cw),
            "-": hline(cw * 0.25, cw * 0.75),
            "╌": hline(cw * 0.05, cw * 0.45) | hline(cw * 0.55, cw * 0.95),
            "▀": rect(0, 0, cw, ch / 2),
            "▄": rect(0, ch / 2, cw, ch),
            "█": rect(0, 0, cw, ch),
            "▌": rect(0, 0, cw / 2, ch),
            "▐": rect(cw / 2, 0, cw, ch),
        }
    )
    for bits, g in enumerate(BLOCKS):
        if g == " ":
            continue
        m = np.zeros((H, W), bool)
        for i in range(8):
            if bits & (1 << i):
                c, r = i % 2, i // 2
                m |= rect(c * cw / 2, r * ch / 4, (c + 1) * cw / 2, (r + 1) * ch / 4)
        out[g] = m
    return {g: m.reshape(ch, SS, cw, SS).mean((1, 3)) for g, m in out.items()}


def frames(path):
    with open(path, encoding="utf-8") as f:
        lines = f.read().split("\n")
    i = 0
    while i < len(lines) and lines[i].startswith("frame"):
        _, cols, rows = lines[i].split("\t")
        cols, rows = int(cols), int(rows)
        cells = [l.split("\t") for l in lines[i + 1 : i + 1 + cols * rows]]
        i += 1 + cols * rows
        yield cols, rows, cells


def hexes(values):
    v = np.array([int(h, 16) for h in values], np.uint32)
    return np.stack([(v >> 16) & 255, (v >> 8) & 255, v & 255], -1).astype(np.float32)


def render(cols, rows, cells, atlas, names, cw, ch):
    syms = [c[0] for c in cells]
    unknown = {s for s in syms if s not in names}
    if unknown:
        print("unknown glyphs drawn blank:", "".join(sorted(unknown)))
    index = np.array([names.get(s, 0) for s in syms]).reshape(rows, cols)
    fg = hexes([c[1] for c in cells]).reshape(rows, cols, 1, 1, 3)
    bg = hexes([c[2] for c in cells]).reshape(rows, cols, 1, 1, 3)
    m = atlas[index][..., None]  # rows, cols, ch, cw, 1
    img = bg * (1 - m) + fg * m
    return Image.fromarray(img.transpose(0, 2, 1, 3, 4).reshape(rows * ch, cols * cw, 3).round().astype(np.uint8))


def main():
    p = argparse.ArgumentParser()
    p.add_argument("dump")
    p.add_argument("out")
    p.add_argument("--cell", default="8x16")
    p.add_argument("--fps", type=float, default=15)
    p.add_argument("--round", action="store_true", help="round dots (for larger cells)")
    p.add_argument("--frames", default=None, help="a:b slice of frames")
    a = p.parse_args()
    cw, ch = map(int, a.cell.split("x"))
    glyphs = masks(cw, ch, a.round)
    names = {g: i for i, g in enumerate(glyphs)}
    atlas = np.stack(list(glyphs.values()))
    imgs = [render(*f, atlas, names, cw, ch) for f in frames(a.dump)]
    if a.frames:
        lo, hi = (int(x) if x else None for x in a.frames.split(":"))
        imgs = imgs[lo:hi]
    if a.out.endswith(".gif"):
        q = [im.quantize(colors=255, method=Image.Quantize.MEDIANCUT) for im in imgs]
        q[0].save(a.out, save_all=True, append_images=q[1:], duration=round(1000 / a.fps), loop=0, optimize=True)
    else:
        imgs[0].save(a.out)
    print(f"{a.out}: {len(imgs)} frame(s), {imgs[0].width}×{imgs[0].height}")


if __name__ == "__main__":
    main()
