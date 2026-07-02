#!/usr/bin/env python3
# Deterministic generator for rich_fixture.png — the multi-color input used by the
# non-vacuous option-forwarding cases in determinism_gate_test.dart.
#
# The fixture is designed so that, matched against palettes/artkal_s.json at the
# 16x20 gate size, ALL THREE hold:
#   (a) >8 distinct matched colors (so --max-colors 8 actually reduces),
#   (b) at least one same-color 4-connected speckle of <=2 beads (so --despeckle 2
#       actually merges something),
#   (c) staged output != gerstner output.
# Source is larger than 16x20 so the Gerstner superpixel path (S>=1) is legal.
#
# Deterministic: fixed seed, no time/OS input. Re-run to reproduce the committed PNG.
from pathlib import Path

from PIL import Image

W, H = 32, 40  # 2x the 16x20 gate -> real Triangle downscale, Gerstner S>=1

# A small deterministic LCG so output never depends on Python's hash seed.
state = 0x1234_5678


def rnd():
    global state
    state = (state * 1103515245 + 12345) & 0x7FFF_FFFF
    return state


# Palette of vivid, well-separated hues so matching yields many distinct codes.
base = [
    (234, 0, 0), (0, 200, 0), (0, 0, 220), (255, 210, 0),
    (255, 120, 0), (150, 0, 200), (0, 200, 200), (255, 0, 150),
    (120, 80, 0), (0, 120, 60), (255, 255, 255), (20, 20, 20),
]

img = Image.new("RGB", (W, H))
px = img.load()
for y in range(H):
    for x in range(W):
        px[x, y] = base[rnd() % len(base)]

# Sprinkle a few isolated single-pixel dots of a rare hue to guarantee specks
# survive as small components after downscale+match.
for (dx, dy) in [(3, 5), (10, 12), (20, 30), (27, 8), (15, 22)]:
    px[dx, dy] = (255, 255, 255) if (dx + dy) % 2 else (0, 0, 0)

img.save(Path(__file__).with_name("rich_fixture.png"))
print(f"wrote rich_fixture.png {W}x{H}")
