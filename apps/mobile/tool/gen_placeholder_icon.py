#!/usr/bin/env python3
"""Generate the PLACEHOLDER launcher icons for Beadsmith (group D / task 5.1).

A grid of round fuse beads (拼豆) laid out into a heart, bright & flat, on a
warm-cream ground. Deterministic (fixed seed) so re-runs are byte-stable.

Outputs (1024x1024) into apps/mobile/assets/icon/:
  ic_foreground.png  Android adaptive FOREGROUND — heart centered in the middle
                     ~64%, fully transparent margin + transparent bead holes so
                     the background layer shows through (fuse-bead look).
  app_icon_ios.png   iOS full-bleed, cream ground + heart w/ padding, NO alpha.
  app_icon.png       base full-bleed square (Android legacy mipmap + iOS fallback).

Background is a solid hex (BG) fed to flutter_launcher_icons as
adaptive_icon_background — no separate background PNG needed.

This is a disposable design helper; the PNG products are what's committed.
"""
from PIL import Image, ImageDraw
from pathlib import Path
import random

SIZE = 1024
SS = 2                      # supersample for smooth circle edges
BG = (0xFF, 0xF3, 0xE0)    # warm cream #FFF3E0 (also adaptive_icon_background)

# Bright, high-saturation, flat bead colors (warm-led, a couple cool pops).
PALETTE = [
    (0xFF, 0x45, 0x57),  # red
    (0xFF, 0x7A, 0x1A),  # orange
    (0xFF, 0xC3, 0x2B),  # yellow
    (0xFF, 0x5D, 0xA2),  # pink
    (0x9B, 0x5D, 0xE5),  # purple
    (0xF7, 0x25, 0x85),  # magenta
    (0x38, 0xD9, 0x7B),  # green
    (0x2E, 0xC4, 0xD8),  # teal
]


def heart_cells(cols, rows):
    """Cells (col,row) whose center falls inside the classic heart curve."""
    cells = []
    for r in range(rows):
        for c in range(cols):
            x = (c + 0.5) / cols * 3.0 - 1.5
            y = 1.45 - (r + 0.5) / rows * 2.9
            v = (x * x + y * y - 1) ** 3 - x * x * y * y * y
            if v <= 0:
                cells.append((c, r))
    return cells


def bead_layer(content_px):
    """Transparent RGBA canvas (SIZE*SS) with the bead heart drawn, centered,
    its bounding box scaled to fit content_px (in final px)."""
    cols, rows = 16, 16
    cells = heart_cells(cols, rows)
    cs = {(c, r) for c, r in cells}
    min_c = min(c for c, _ in cells)
    max_c = max(c for c, _ in cells)
    min_r = min(r for _, r in cells)
    max_r = max(r for _, r in cells)
    span = max(max_c - min_c + 1, max_r - min_r + 1)

    px = SIZE * SS
    cell = (content_px * SS) / span
    # center the bbox on the canvas
    bbox_w = (max_c - min_c + 1) * cell
    bbox_h = (max_r - min_r + 1) * cell
    off_x = (px - bbox_w) / 2 - min_c * cell
    off_y = (px - bbox_h) / 2 - min_r * cell

    img = Image.new("RGBA", (px, px), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    rng = random.Random(7)
    # stable color per cell, avoid same color touching left/top neighbor
    color_of = {}
    for c, r in sorted(cells):
        banned = {color_of.get((c - 1, r)), color_of.get((c, r - 1))}
        choices = [col for col in PALETTE if col not in banned] or PALETTE
        color_of[(c, r)] = rng.choice(choices)

    outer_r = cell * 0.46      # small gap between beads
    hole_r = cell * 0.19       # fuse-bead center hole
    for (c, r), col in color_of.items():
        cx = off_x + (c + 0.5) * cell
        cy = off_y + (r + 0.5) * cell
        d.ellipse([cx - outer_r, cy - outer_r, cx + outer_r, cy + outer_r],
                  fill=col + (255,))
        # punch a transparent hole (ImageDraw replaces pixels, so this clears alpha)
        d.ellipse([cx - hole_r, cy - hole_r, cx + hole_r, cy + hole_r],
                  fill=(0, 0, 0, 0))
    return img


def save(img, path):
    img.resize((SIZE, SIZE), Image.LANCZOS).save(path)


# Resolve relative to this script (apps/mobile/tool/) so re-runs work from any cwd.
OUT = Path(__file__).resolve().parent.parent / "assets" / "icon"
OUT.mkdir(parents=True, exist_ok=True)

# Android adaptive foreground: heart in center ~64%, transparent margin+holes.
fg = bead_layer(content_px=SIZE * 0.64)
save(fg, OUT / "ic_foreground.png")

# Full-bleed: cream ground + heart with padding (~78% content), then flatten.
bl = bead_layer(content_px=SIZE * 0.78)
full = Image.new("RGBA", (SIZE * SS, SIZE * SS), (*BG, 255))
full.alpha_composite(bl)
full_rgb = full.convert("RGB").resize((SIZE, SIZE), Image.LANCZOS)  # no alpha
full_rgb.save(OUT / "app_icon_ios.png")
full_rgb.save(OUT / "app_icon.png")

print("wrote ic_foreground.png, app_icon_ios.png, app_icon.png ->", OUT)
print("adaptive_icon_background hex = #%02X%02X%02X" % BG)
