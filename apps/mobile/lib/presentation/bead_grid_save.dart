import 'dart:math' as math;
import 'dart:typed_data' show Uint8List;
import 'dart:ui' show Color;

import 'package:image/image.dart' as img;

import '../infrastructure/palette_codec.dart' show PaletteColor;
import 'bead_grid_layout.dart';

/// Texture/OOM ceiling: the PNG's longest side never exceeds this.
const _maxEdgePx = 4096;

/// Headless CPU rasterizer for the pegboard bead grid — the "保存到相册" adapter.
///
/// Renders `BeadPattern.cells` + palette to PNG bytes on the CPU via the `image`
/// package, sharing geometry with the on-screen preview through [BeadGridLayout]
/// (design D5, so the two never drift). It mirrors `_BeadGridPainter`'s draw:
/// board-rect border whitespace, per-cell fills with a gap that reveals the base
/// as fine grid lines, every-10 bold separators, and 1-based axis labels.
///
/// **No `dart:ui` rasterization** — deliberately avoids `Picture.toImage` /
/// `RepaintBoundary.toImage` (blocked on the iOS simulator + GPU texture caps,
/// same reason the crop path bypasses them). Pure CPU, no Widget/Riverpod.
///
/// Pixel budget: cells start at `4096 / (max(W,H)+2k)` clamped to 4..10 px; if
/// the resulting canvas longest side still exceeds 4096 px (huge patterns) the
/// cell size is scaled down proportionally — below 4 px if needed — so the PNG
/// never blows past the 4096 px texture/OOM ceiling (a hard clamp that does not
/// rely on any external upper bound on W/H).
Uint8List renderPatternPng({
  required List<int> cells,
  required int width,
  required int height,
  required List<PaletteColor> palette,
  required int borderRings,
}) {
  final k = borderRings;
  final layout = saveLayoutFor(width, height, k);

  final cellSize = layout.cellSize;
  final canvas = layout.canvasSize;
  // Hard-clamp both edges to the ceiling: at an exactly-4096 longest side, FP +
  // ceil could land on 4097, so min() keeps the PNG within the texture cap
  // (trims a sub-pixel last margin row on only the largest patterns).
  final image = img.Image(
    width: math.min(canvas.width.ceil(), _maxEdgePx),
    height: math.min(canvas.height.ceil(), _maxEdgePx),
  );

  // ponytail: fixed neutrals — this is a headless renderer with no theme.
  // Cream base doubles as the fine grid-line color revealed through cell gaps
  // (mirrors the preview filling the content with lineColor first).
  final base = img.ColorRgb8(250, 248, 242); // 奶白 base + fine line
  final border = img.ColorRgb8(232, 230, 224); // 浅中性 border whitespace
  final boldColor = img.ColorRgb8(90, 90, 96);
  final tickColor = img.ColorRgb8(70, 70, 76);

  img.fill(image, color: base);
  if (layout.hasBorder) {
    final b = layout.boardRect;
    img.fillRect(image,
        x1: b.left.round(),
        y1: b.top.round(),
        x2: b.right.round() - 1,
        y2: b.bottom.round() - 1,
        color: border);
  }

  // Pre-convert palette to image colors once.
  final colors = [for (final p in palette) _toImgColor(p.rgb)];
  final content = layout.contentRect;

  // Same gap math as `_BeadGridPainter`: gap shrinks with cell size, capped so
  // the drawn cell stays positive even at sub-pixel cells.
  final gap = (cellSize * 0.08).clamp(0.0, 3.0);
  final cellGap = math.min(gap, cellSize * 0.5);
  final inset = cellGap / 2;

  for (int r = 0; r < height; r++) {
    for (int c = 0; c < width; c++) {
      final idx = cells[r * width + c];
      // Belt-and-suspenders like the painter: an out-of-range index degrades to
      // the base color instead of throwing.
      final color = (idx >= 0 && idx < colors.length) ? colors[idx] : base;
      final cx = content.left + c * cellSize;
      final cy = content.top + r * cellSize;
      img.fillRect(image,
          x1: (cx + inset).round(),
          y1: (cy + inset).round(),
          x2: (cx + cellSize - inset).round() - 1,
          y2: (cy + cellSize - inset).round() - 1,
          color: color);
    }
  }

  // Every-10 bold separators, drawn after cells so they win at boundaries.
  if (layout.boldColLineXs.isNotEmpty || layout.boldRowLineYs.isNotEmpty) {
    final boldW = (cellSize * 0.12).clamp(1.5, 4.0);
    // At k=0 with a dimension that is a multiple of 10, the last every-10 line
    // sits exactly on content.right/bottom == the canvas edge; `img.drawLine`
    // clips off-canvas coords (valid 0..dim-1) and would DROP it. Clamp the
    // far edge to the last pixel, like the engine's `out_w-1` (renderer/mod.rs).
    final maxX = image.width - 1;
    final maxY = image.height - 1;
    for (final x in layout.boldColLineXs) {
      final lx = math.min(x.round(), maxX);
      img.drawLine(image,
          x1: lx,
          y1: content.top.round(),
          x2: lx,
          y2: math.min(content.bottom.round(), maxY),
          color: boldColor,
          thickness: boldW);
    }
    for (final y in layout.boldRowLineYs) {
      final ly = math.min(y.round(), maxY);
      img.drawLine(image,
          x1: content.left.round(),
          y1: ly,
          x2: math.min(content.right.round(), maxX),
          y2: ly,
          color: boldColor,
          thickness: boldW);
    }
  }

  // 1-based axis labels in the tick margin. Fixed bitmap fonts are far taller
  // than the save renderer's small margins (cellPx 4..10 → marginTop ≤ 14px,
  // marginLeft ≤ ~18px), so each number is rendered once then proportionally
  // shrunk to fit ENTIRELY inside its band — never spilling onto the first
  // row/column of beads or off the canvas. D5: position still comes from the
  // layout (colTicks/rowTicks.along + marginTopLeft); only the glyph packing is
  // save-specific, so it need not match the preview pixel-for-pixel.
  final font =
      cellSize < 8 ? img.arial14 : (cellSize < 20 ? img.arial24 : img.arial48);
  final pad = cellSize / 5; // keeps the row label off the board edge
  final marginTop = layout.marginTopLeft.dy;
  final marginLeft = layout.marginTopLeft.dx;
  // Column labels: right edge at the boundary x, bottom on the board top — the
  // whole glyph lands in the top margin (y < marginTop).
  for (final t in layout.colTicks) {
    _drawScaledLabel(image, '${t.value}', font, tickColor,
        maxW: t.along, maxH: marginTop - 1,
        anchorRight: t.along, anchorBottom: marginTop);
  }
  // Row labels: right edge just inside the board left, bottom on the boundary y
  // — the whole glyph lands in the left margin (x < marginLeft).
  for (final t in layout.rowTicks) {
    _drawScaledLabel(image, '${t.value}', font, tickColor,
        maxW: marginLeft - pad, maxH: t.along,
        anchorRight: marginLeft - pad, anchorBottom: t.along);
  }

  return img.encodePng(image);
}

/// Save-side layout: derive the per-cell pixel budget, then hard-clamp the whole
/// canvas to [_maxEdgePx]. Cells start at `4096 / (max(W,H)+2k)` clamped to
/// 4..10 px; if the resulting canvas longest side still exceeds the ceiling
/// (huge patterns) the cell size scales down proportionally — below 4 px if
/// needed — so the PNG never blows past the texture/OOM cap. This is the single
/// production derivation the drift test anchors on (no local mirror).
BeadGridLayout saveLayoutFor(int width, int height, int borderRings) {
  final k = borderRings;
  final span = math.max(width, height) + 2 * k;
  var cellPx = (_maxEdgePx ~/ span).clamp(4, 10).toDouble();
  var layout = BeadGridLayout(
      width: width, height: height, borderRings: k, cellSize: cellPx);
  if (layout.canvasSize.longestSide > _maxEdgePx) {
    // canvasSize is linear in cellSize with zero intercept, so this lands the
    // longest side exactly on _maxEdgePx (may push cellPx < 4 — the ceiling wins).
    cellPx = cellPx * _maxEdgePx / layout.canvasSize.longestSide;
    layout = BeadGridLayout(
        width: width, height: height, borderRings: k, cellSize: cellPx);
  }
  return layout;
}

/// Render [s] with [font] onto a transparent buffer, proportionally shrink it to
/// fit within `maxW × maxH`, then composite it bottom-right-anchored at
/// (`anchorRight`, `anchorBottom`). The size cap + bottom-right anchoring keep
/// the whole glyph inside its margin band and off the board.
void _drawScaledLabel(
  img.Image image,
  String s,
  img.BitmapFont font,
  img.Color color, {
  required double maxW,
  required double maxH,
  required double anchorRight,
  required double anchorBottom,
}) {
  if (maxW <= 0 || maxH <= 0) return;
  var w = 0;
  for (final c in s.codeUnits) {
    final ch = font.characters[c];
    if (ch != null) w += ch.xAdvance;
  }
  final h = font.lineHeight;
  if (w <= 0 || h <= 0) return;

  final label = img.Image(width: w, height: h, numChannels: 4);
  img.drawString(label, s, font: font, x: 0, y: 0, color: color);

  // Shrink to the band (never upscale past the glyph's natural size).
  final scale = math.min(1.0, math.min(maxW / w, maxH / h));
  final dw = math.max(1, (w * scale).round());
  final dh = math.max(1, (h * scale).round());
  final scaled = (dw == w && dh == h)
      ? label
      : img.copyResize(label, width: dw, height: dh);

  img.compositeImage(image, scaled,
      dstX: anchorRight.round() - dw, dstY: anchorBottom.round() - dh);
}

img.Color _toImgColor(Color c) {
  final v = c.toARGB32();
  return img.ColorRgb8((v >> 16) & 0xFF, (v >> 8) & 0xFF, v & 0xFF);
}
