import 'dart:ui' show Offset, Rect, Size;

/// Pure geometry for the pegboard bead grid — the **single layout source** the
/// on-screen preview (`ui.Canvas`) and the CPU save renderer (`image` package)
/// both consume, so the two never drift (design D5).
///
/// Inputs are only **scale parameters**: content beads `width × height`, border
/// rings `borderRings` (k), and a `cellSize` in pixels supplied by each adapter
/// (preview: fit-to-viewport px; save: pixel-budget px). It takes **no `cells`
/// / palette** — coloring is the draw adapter's job; this only computes where
/// things sit.
///
/// Two-layer geometry (D6): the whole scaled canvas =
/// **tick margin (top/left) + board area `(W+2k) × (H+2k)`**. Content sits k
/// rings in from the board edge; the border rings are empty whitespace (no bead,
/// not indexed).
///
/// Scale-independent products — [canvasAspect], tick **values**, k, board bead
/// dims, and any normalized position — are identical across cellSize and are the
/// drift-test anchors. Pixel-absolute products ([cellSize], [marginTopLeft],
/// rects) scale with cellSize and MUST NOT be asserted equal across adapters.
///
/// Margins use `cellSize/5` as a **real proportion** (not the engine's integer
/// `scale = cell/5` truncation), so [canvasAspect] is exactly cellSize-free
/// (D5). At integer scales the values match `renderer/mod.rs` exactly.
class BeadGridLayout {
  BeadGridLayout._({
    required this.width,
    required this.height,
    required this.borderRings,
    required this.cellSize,
    required this.marginTopLeft,
    required this.colTicks,
    required this.rowTicks,
    required this.boldColLineXs,
    required this.boldRowLineYs,
  });

  /// Content bead dimensions (may be non-square; a portrait crop has W < H).
  final int width;
  final int height;

  /// Border rings k (each ring = 1 empty cell per side). 0 = no border.
  final int borderRings;

  /// Pixel edge length of one bead cell — the adapter's scale parameter.
  final double cellSize;

  /// Top-left tick margin in pixels: `(marginLeft, marginTop)`. The board area
  /// begins here. `marginLeft > 0` iff H ≥ 10 (row labels), `marginTop > 0` iff
  /// W ≥ 10 (column labels).
  final Offset marginTopLeft;

  /// Column ticks (top axis), every 10th 1-based content column (10, 20, …);
  /// empty when W < 10. `GridTick.along` = canvas-x of the boundary right of
  /// column n (where the right-aligned label ends) = the bold vertical line x.
  final List<GridTick> colTicks;

  /// Row ticks (left axis), every 10th 1-based content row; empty when H < 10.
  /// `GridTick.along` = canvas-y of the boundary below row n (its bold horizontal
  /// line), symmetric with [colTicks]'s right-of-column-n anchor.
  final List<GridTick> rowTicks;

  /// Canvas-x of every 10th content-column boundary (bold vertical separators).
  final List<double> boldColLineXs;

  /// Canvas-y of every 10th content-row boundary (bold horizontal separators).
  final List<double> boldRowLineYs;

  /// Compute the layout. `width, height ≥ 1`, `borderRings ≥ 0`, `cellSize > 0`.
  factory BeadGridLayout({
    required int width,
    required int height,
    required int borderRings,
    required double cellSize,
  }) {
    assert(width >= 1 && height >= 1, 'content must be ≥ 1×1');
    assert(borderRings >= 0, 'borderRings must be ≥ 0');
    assert(cellSize > 0, 'cellSize must be > 0');

    const step = 10;
    final k = borderRings;
    final scaleF = cellSize / 5.0; // real proportion, not integer truncation

    // Column labels (top) exist iff W ≥ 10; row labels (left) iff H ≥ 10 —
    // same gate as engine `has_col = width >= STEP` / `has_row`.
    final hasCol = width >= step;
    final hasRow = height >= step;

    // marginTop is fixed height (labels are 5 rows tall, drawn along the top).
    final marginTop = hasCol ? 7 * scaleF : 0.0;
    // marginLeft fits the widest row label: num_w(d) + 2*pad = scaleF*(4d+1).
    final rowDigits = hasRow ? '${(height ~/ step) * step}'.length : 0;
    final marginLeft = hasRow ? scaleF * (4 * rowDigits + 1) : 0.0;
    final margin = Offset(marginLeft, marginTop);

    // Content origin = margin + k rings.
    final contentLeft = marginLeft + k * cellSize;
    final contentTop = marginTop + k * cellSize;

    final colTicks = <GridTick>[];
    final boldColLineXs = <double>[];
    for (int n = step; n <= width; n += step) {
      final x = contentLeft + n * cellSize; // boundary right of content col n
      colTicks.add(GridTick(n, x));
      boldColLineXs.add(x);
    }
    final rowTicks = <GridTick>[];
    final boldRowLineYs = <double>[];
    for (int n = step; n <= height; n += step) {
      rowTicks.add(GridTick(n, contentTop + n * cellSize)); // boundary below row n = bold line (matches colTick)
      boldRowLineYs.add(contentTop + n * cellSize); // boundary below row n
    }

    return BeadGridLayout._(
      width: width,
      height: height,
      borderRings: k,
      cellSize: cellSize,
      marginTopLeft: margin,
      colTicks: colTicks,
      rowTicks: rowTicks,
      boldColLineXs: boldColLineXs,
      boldRowLineYs: boldRowLineYs,
    );
  }

  /// Board bead dimensions `(W+2k) × (H+2k)`.
  int get boardCols => width + 2 * borderRings;
  int get boardRows => height + 2 * borderRings;

  /// Whole scaled canvas size = tick margin + board area (pixels).
  Size get canvasSize => Size(
        marginTopLeft.dx + boardCols * cellSize,
        marginTopLeft.dy + boardRows * cellSize,
      );

  /// Aspect of the whole letterboxed canvas (**includes margin**, cellSize-free
  /// — the drift anchor). Only equals `W:H` when both margins are 0 (both axes
  /// < 10) and k = 0.
  double get canvasAspect {
    final s = canvasSize;
    return s.width / s.height;
  }

  /// Board area rect in canvas pixels (margin-offset). Fill this with the border
  /// whitespace color, then draw content over [contentRect].
  Rect get boardRect =>
      Rect.fromLTWH(marginTopLeft.dx, marginTopLeft.dy,
          boardCols * cellSize, boardRows * cellSize);

  /// Content rect in canvas pixels: board area inset by k rings.
  Rect get contentRect => Rect.fromLTWH(
        marginTopLeft.dx + borderRings * cellSize,
        marginTopLeft.dy + borderRings * cellSize,
        width * cellSize,
        height * cellSize,
      );

  /// True when there is a border ring to render (k > 0).
  bool get hasBorder => borderRings > 0;

  /// Canvas-x of every content-column boundary 0..W (thin grid lines). Derived
  /// on demand; adapters that fill-and-inset cells needn't call this.
  List<double> get fineColLineXs => [
        for (int i = 0; i <= width; i++) contentRect.left + i * cellSize,
      ];

  /// Canvas-y of every content-row boundary 0..H (thin grid lines).
  List<double> get fineRowLineYs => [
        for (int i = 0; i <= height; i++) contentRect.top + i * cellSize,
      ];

  /// Map a point in **canvas-local** space (already inverse-transformed, relative
  /// to the canvas top-left) to a 1-based-free content cell `(row, col)`, or null
  /// if it falls in the margin, a border ring, or off the board — the hit-test
  /// core (D6): subtract marginTopLeft → /cellSize → subtract k → bounds-check.
  /// Callers add +1 for display; a null result is a no-op tap.
  ({int row, int col})? contentCellAt(Offset canvasLocal) {
    final col = ((canvasLocal.dx - marginTopLeft.dx) / cellSize).floor() -
        borderRings;
    final row = ((canvasLocal.dy - marginTopLeft.dy) / cellSize).floor() -
        borderRings;
    if (col < 0 || col >= width || row < 0 || row >= height) return null;
    return (row: row, col: col);
  }

  /// Canvas-space center of content cell `(row, col)` (0-based) — inverse of
  /// [contentCellAt], used for round-trip checks and per-cell drawing.
  Offset contentCellCenter(int row, int col) => Offset(
        contentRect.left + (col + 0.5) * cellSize,
        contentRect.top + (row + 0.5) * cellSize,
      );
}

/// A tick label: its 1-based content coordinate [value] (10, 20, …) and [along]
/// = the pixel position along its axis where the adapter anchors the number
/// (column: boundary right of the column; row: top of the row).
class GridTick {
  const GridTick(this.value, this.along);
  final int value;
  final double along;
}
