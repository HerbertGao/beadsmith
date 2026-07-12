import 'dart:io' show File, Platform;
import 'dart:math' as math;
import 'dart:ui' show Color;

import 'package:beadsmith/infrastructure/palette_codec.dart' show PaletteColor;
import 'package:beadsmith/presentation/bead_grid_layout.dart';
import 'package:beadsmith/presentation/bead_grid_save.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:image/image.dart' as img;

/// Two-color palette used by the deterministic renders below.
const _bw = [
  PaletteColor(code: 'A', name: 'black', rgb: Color(0xFF000000)),
  PaletteColor(code: 'B', name: 'white', rgb: Color(0xFFFFFFFF)),
];

/// Mirror of the PREVIEW adapter's cellSize derivation (bead_grid_view.dart):
/// letterbox the whole canvas into a viewport by canvasAspect, reverse-derive
/// the fitted cellSize.
BeadGridLayout _previewLayout(int w, int h, int k, double vw, double vh) {
  final unit = BeadGridLayout(width: w, height: h, borderRings: k, cellSize: 1);
  final gridAspect = unit.canvasAspect;
  final viewAspect = vw / vh;
  final gw = gridAspect > viewAspect ? vw : vh * gridAspect;
  final cellSize = gw / unit.canvasSize.width;
  return BeadGridLayout(width: w, height: h, borderRings: k, cellSize: cellSize);
}

void main() {
  test('renderPatternPng returns a non-empty PNG (20x20, k=2)', () {
    final cells = [for (int i = 0; i < 20 * 20; i++) i % 2];
    final png = renderPatternPng(
      cells: cells,
      width: 20,
      height: 20,
      palette: _bw,
      borderRings: 2,
    );
    expect(png, isNotEmpty);
    // PNG magic number: 0x89 'P' 'N' 'G'.
    expect(png.sublist(0, 4), [0x89, 0x50, 0x4E, 0x47]);
  });

  // ── 5.3 border not counted: renderer indexes content cells only ──
  test('5.3 border adds no beads: content-only cells render at any k', () {
    // Non-square content; cells length == W*H (content only), NOT board size.
    const w = 12, h = 8;
    final cells = [for (int i = 0; i < w * h; i++) i % 2];
    // If the renderer wrongly indexed the (W+2k)*(H+2k) board it would read past
    // `cells` and throw — so a clean render proves the border is pure whitespace.
    for (final k in const [0, 2, 5]) {
      final png =
          renderPatternPng(cells: cells, width: w, height: h, palette: _bw, borderRings: k);
      expect(png, isNotEmpty, reason: 'k=$k renders from content-only cells');
      // The board grows with k but the bead count (stats source) never does.
      final board = BeadGridLayout(width: w, height: h, borderRings: k, cellSize: 4);
      expect(board.width * board.height, w * h);
      if (k > 0) {
        expect(board.boardCols * board.boardRows, greaterThan(w * h));
      }
    }
  });

  // ── 5.4 anti-drift: preview & save read the SAME layout function; their
  // scale-independent products agree even though their pixel scales differ ──
  test('5.4 preview vs save layouts agree on scale-independent products', () {
    const w = 30, h = 45, k = 2; // non-square, both margins, a border
    final preview = _previewLayout(w, h, k, 400, 400); // fit small viewport
    final save = saveLayoutFor(w, h, k); // production pixel-budget derivation

    // Sanity: the two adapters land on DIFFERENT pixel scales (so equality below
    // is a real scale-independence claim, not a same-scale tautology).
    expect(preview.cellSize, isNot(closeTo(save.cellSize, 1e-6)));
    expect(preview.marginTopLeft.dx, isNot(closeTo(save.marginTopLeft.dx, 1e-6)));

    // Drift anchors — MUST match across scales.
    expect(preview.canvasAspect, closeTo(save.canvasAspect, 1e-9));
    expect(preview.borderRings, save.borderRings);
    expect(preview.colTicks.map((t) => t.value).toList(),
        save.colTicks.map((t) => t.value).toList());
    expect(preview.rowTicks.map((t) => t.value).toList(),
        save.rowTicks.map((t) => t.value).toList());
    // Normalized (canvas-fraction) bold-line positions match.
    double nx(BeadGridLayout l, double x) => x / l.canvasSize.width;
    double ny(BeadGridLayout l, double y) => y / l.canvasSize.height;
    for (int i = 0; i < preview.boldColLineXs.length; i++) {
      expect(nx(preview, preview.boldColLineXs[i]),
          closeTo(nx(save, save.boldColLineXs[i]), 1e-9));
    }
    for (int i = 0; i < preview.boldRowLineYs.length; i++) {
      expect(ny(preview, preview.boldRowLineYs[i]),
          closeTo(ny(save, save.boldRowLineYs[i]), 1e-9));
    }
  });

  // ── 5.4 CPU raster golden: byte-for-byte, no dart:ui / toImage ──
  test('5.4 CPU raster PNG matches committed golden (byte-exact)', () {
    const w = 12, h = 12, k = 1;
    // Deterministic checker so the golden is fully reproducible.
    final cells = [for (int i = 0; i < w * h; i++) ((i ~/ w) + (i % w)) % 2];
    final png = renderPatternPng(
        cells: cells, width: w, height: h, palette: _bw, borderRings: k);

    final golden = File('test/golden/bead_grid_12x12_k1.png');
    // Opt-in reseed only (no silent seed): a missing golden FAILS by default so
    // a machine without it can't false-green; regenerate with
    // `UPDATE_GOLDEN=1 flutter test test/bead_grid_save_test.dart` then git add.
    if (Platform.environment['UPDATE_GOLDEN'] == '1') {
      golden.parent.createSync(recursive: true);
      golden.writeAsBytesSync(png);
    }
    expect(golden.existsSync(), isTrue,
        reason: 'golden missing — run with UPDATE_GOLDEN=1 to regenerate, then '
            'git add test/golden/bead_grid_12x12_k1.png');
    expect(png, orderedEquals(golden.readAsBytesSync()),
        reason: 'CPU raster drifted from the committed golden');
  });

  // ── F1: tick numbers stay in the margin band, never onto the board ──
  test('tick labels stay in the margin, never intrude on the board (k=0)', () {
    // W=H=20, k=0, default budget → cellPx=10 so marginTop=14 / marginLeft=18,
    // both smaller than the bitmap font: the label MUST be scaled into the band.
    const w = 20, h = 20, k = 0;
    final cells = [for (int i = 0; i < w * h; i++) i % 2];
    final png = renderPatternPng(
        cells: cells, width: w, height: h, palette: _bw, borderRings: k);

    final image = img.decodePng(png)!;
    final layout = saveLayoutFor(w, h, k);
    final ml = layout.marginTopLeft.dx; // board region: x >= ml && y >= mt
    final mt = layout.marginTopLeft.dy;
    expect(ml, greaterThan(0)); // both axes labelled at W=H=20
    expect(mt, greaterThan(0));

    // tickColor(70,70,76) from bead_grid_save.dart's fixed neutral palette.
    bool isTick(img.Pixel p) => p.r == 70 && p.g == 70 && p.b == 76;
    var boardTicks = 0, marginTicks = 0;
    for (final p in image) {
      if (!isTick(p)) continue;
      if (p.x >= ml && p.y >= mt) {
        boardTicks++;
      } else {
        marginTicks++;
      }
    }
    expect(boardTicks, 0,
        reason: 'labels must not spill onto the first row/column of beads');
    expect(marginTicks, greaterThan(0),
        reason: 'labels are actually drawn (non-empty) inside the margin band');
  });

  // ── edge bold line: k=0 + dimension a multiple of 10 keeps its last every-10
  //    separator (it sits on the canvas edge; drawLine would clip it off) ──
  test('last every-10 bold line survives at k=0 with a 10-multiple dim', () {
    const w = 20, h = 20, k = 0; // last bold line at col/row 20 == canvas edge
    final cells = [for (int i = 0; i < w * h; i++) i % 2];
    final png = renderPatternPng(
        cells: cells, width: w, height: h, palette: _bw, borderRings: k);
    final image = img.decodePng(png)!;

    bool isBold(img.Pixel p) => p.r == 90 && p.g == 90 && p.b == 96;
    var lastColBold = 0, lastRowBold = 0;
    for (final p in image) {
      if (!isBold(p)) continue;
      if (p.x == image.width - 1) lastColBold++;
      if (p.y == image.height - 1) lastRowBold++;
    }
    // Discriminator: at k=0 the interior row-10/col-10 lines already reach the
    // canvas edge and `drawLine` clips their endpoint onto width-1/height-1, so a
    // bare `> 0` would pass on the UNCLAMPED code too. Require the FULL edge line
    // (spans most of the axis), which only the clamped col-20/row-20 line draws —
    // a clipped interior endpoint leaves just a few pixels.
    expect(lastColBold, greaterThan(image.height ~/ 2),
        reason: 'the col-20 line must span the last pixel COLUMN, not just a '
            'clipped interior endpoint');
    expect(lastRowBold, greaterThan(image.width ~/ 2),
        reason: 'the row-20 line must span the last pixel ROW');
  });

  // ── 5.5 pixel budget: huge pattern hard-clamps to maxEdgePx, no OOM ──
  test('5.5 oversized pattern hard-clamps final canvas to maxEdgePx', () {
    const maxEdgePx = 4096;
    const w = 1050, h = 1050, k = 8; // even cellPx=4 overflows → clamp must fire
    final cells = List<int>.filled(w * h, 0);
    final png = renderPatternPng(
        cells: cells, width: w, height: h, palette: _bw, borderRings: k);

    final image = img.decodePng(png)!;
    expect(image.width, lessThanOrEqualTo(maxEdgePx));
    expect(image.height, lessThanOrEqualTo(maxEdgePx));
    // The clamp actually fired (image pushed up to the ceiling, not left tiny) —
    // an un-clamped cellPx=4 would have produced a ~4278px longest side.
    expect(math.max(image.width, image.height), greaterThan(4000),
        reason: 'longest side rides the hard clamp near maxEdgePx');
  });
}
