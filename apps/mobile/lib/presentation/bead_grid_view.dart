import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../infrastructure/palette_codec.dart' show PaletteColor;
import 'bead_grid_layout.dart';

/// A zoomable, tappable bead grid rendered from `BeadPattern.cells` + the
/// parsed palette. Each cell is a solid color square sized to the grid's
/// `width × height`; tap a content cell to get its `(row, col, paletteIndex)`.
///
/// **Geometry comes from [BeadGridLayout]** (the single layout source shared
/// with the CPU save renderer). The whole scaled canvas = tick margin (top/left)
/// + board area `(width+2k) × (height+2k)`; content sits `borderRings` (k) rings
/// in from the board edge, the border rings are empty whitespace (no bead, not
/// indexed). Fine grid lines, **every-10 bold separators**, and **1-based axis
/// labels** are all placed by the layout — this widget only draws them.
///
/// **Zoom/pan is self-managed** — a [GestureDetector] drives a [Transform]
/// (translate + scale) directly, rather than using [InteractiveViewer], which
/// did not respond to the emulator's Ctrl-drag pinch here. A [LayoutBuilder]
/// letterboxes the *whole canvas* (by `layout.canvasAspect`, which includes the
/// tick margin) so cells stay square; pinch-zoom scales the whole viewport
/// content, so the grid grows and consumes the surrounding whitespace — the
/// familiar photo-album zoom. Pan is clamped so the grid can't leave the screen.
///
/// **Cell boundaries are visible** — the painter fills the content area with the
/// line color first, then draws each cell deflated by a small gap, so the line
/// color shows through as fine grid lines (scales with cell size, clamped).
///
/// **Data source is the structured `cells` array, NOT the `previewPng`
/// bitmap** — keeps the "never derive info from the rendered image" hard
/// rule (CLAUDE.md rule 3), and gives exact hit-testing at any zoom level.
/// A tap in the tick margin or a border ring is a no-op (no bead to report).
///
/// `highlightedIndex`, when set, strokes a ring around every content cell
/// sharing that palette index — the "高亮同色" affordance driven by the parent
/// (ResultPage owns the selection state).
class BeadGridView extends StatefulWidget {
  const BeadGridView({
    super.key,
    required this.cells,
    required this.width,
    required this.height,
    required this.palette,
    this.borderRings = 0,
    this.highlightedIndex,
    this.onCellTap,
  });

  /// Row-major palette indices (length == [width] * [height]).
  final List<int> cells;

  /// Content grid dimensions in beads.
  final int width;
  final int height;

  /// Empty whitespace border rings (k) around the content, one cell per side
  /// per ring. Default 0 (no border, current behavior). Border cells hold no
  /// bead and are never tappable/indexed.
  final int borderRings;

  /// Palette colors indexed by `cells[i]`. Must be the full palette in engine
  /// order (see `parsePalette`).
  final List<PaletteColor> palette;

  /// Palette index to highlight (all matching cells get a ring). null = none.
  final int? highlightedIndex;

  /// Called when the user taps a content cell. `(row, col, paletteIndex)` are
  /// **content-relative 0-based** (border/margin excluded); the parent adds +1
  /// for display.
  final void Function(int row, int col, int paletteIndex)? onCellTap;

  @override
  State<BeadGridView> createState() => _BeadGridViewState();
}

class _BeadGridViewState extends State<BeadGridView> {
  /// Self-managed pan/zoom transform (same approach as CropFrame, which is
  /// proven to work — InteractiveViewer's built-in zoom did NOT respond to the
  /// emulator's Ctrl-drag pinch here, so we drive the Transform ourselves via
  /// GestureDetector.onScale*).
  double _scale = 1.0;
  Offset _offset = Offset.zero; // translation in viewport pixels
  double _scaleStart = 1.0;
  Offset _offsetStart = Offset.zero;
  Offset _focalStart = Offset.zero;

  // The whole canvas rect (tick margin + board) in viewport space at 1×, from
  // the last layout pass — used to (a) hit-test taps and (b) clamp pan so the
  // grid can't be dragged fully off-screen.
  Rect _gridRect1x = Rect.zero;
  Size _viewport = Size.zero;

  // Screen-scale layout (cellSize reverse-derived from the fitted rect); its
  // marginTopLeft/cellSize match _gridRect1x, so it hit-tests screen taps.
  BeadGridLayout? _layout;

  static const double _minScale = 1.0;
  static const double _maxScale = 20.0;

  void _onScaleStart(ScaleStartDetails d) {
    _scaleStart = _scale;
    _offsetStart = _offset;
    _focalStart = d.localFocalPoint;
  }

  void _onScaleUpdate(ScaleUpdateDetails d) {
    setState(() {
      final newScale = (_scaleStart * d.scale).clamp(_minScale, _maxScale);
      // Zoom about the gesture focal point so content under the fingers stays
      // put; pan comes from the CURRENT focal position (not the per-frame
      // delta), so a continuous drag accumulates its full travel.
      final focal = _focalStart;
      // Keep the scene point under `focal` fixed across the scale change.
      final scaleRatio = newScale / _scaleStart;
      // Pin the scene point that was under _focalStart to the CURRENT focal
      // position: this reconstructs both the zoom-about-focal and the full pan
      // travel from the fixed gesture-start baseline every frame (adding the
      // per-frame focalPointDelta instead loses all but the last frame's pan).
      _offset = (_offsetStart - focal) * scaleRatio + d.localFocalPoint;
      _scale = newScale;
      _clampOffset();
    });
  }

  /// Keep at least part of the grid on-screen (don't let it be flung away).
  void _clampOffset() {
    if (_viewport == Size.zero) return;
    final gw = _gridRect1x.width * _scale;
    final gh = _gridRect1x.height * _scale;
    // Allowed translation range: the grid may move until its far edge reaches
    // the opposite viewport edge (with a small margin), so it never fully
    // leaves the screen.
    final minX = _viewport.width - (_gridRect1x.right * _scale);
    final maxX = -_gridRect1x.left * _scale;
    final minY = _viewport.height - (_gridRect1x.bottom * _scale);
    final maxY = -_gridRect1x.top * _scale;
    double clampRange(double v, double a, double b) =>
        a <= b ? v.clamp(a, b) : v.clamp(b, a);
    // Only clamp on an axis where the grid overflows the viewport; otherwise
    // keep it centered on that axis.
    _offset = Offset(
      gw >= _viewport.width ? clampRange(_offset.dx, minX, maxX) : _centeredX(),
      gh >= _viewport.height
          ? clampRange(_offset.dy, minY, maxY)
          : _centeredY(),
    );
  }

  double _centeredX() =>
      (_viewport.width - _gridRect1x.width * _scale) / 2 -
      _gridRect1x.left * _scale;
  double _centeredY() =>
      (_viewport.height - _gridRect1x.height * _scale) / 2 -
      _gridRect1x.top * _scale;

  void _onTapUp(TapUpDetails d) {
    final layout = _layout;
    if (layout == null || _gridRect1x == Rect.zero) return;
    // Invert the transform: scene = (viewportPoint - offset) / scale, then
    // subtract the canvas 1× top-left to land in canvas-local coords.
    final scene = (d.localPosition - _offset) / _scale;
    final canvasLocal = scene - _gridRect1x.topLeft;
    // The layout strips the tick margin + k border rings and bounds-checks;
    // a tap in the margin/border/off-board returns null → no-op tap.
    final hit = layout.contentCellAt(canvasLocal);
    if (hit == null) return;
    widget.onCellTap
        ?.call(hit.row, hit.col, widget.cells[hit.row * widget.width + hit.col]);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    return LayoutBuilder(
      builder: (context, constraints) {
        _viewport = Size(constraints.maxWidth, constraints.maxHeight);
        // Unit layout (cellSize = 1) gives the scale-free canvas aspect + size;
        // letterbox by canvasAspect (INCLUDES the tick margin, so it is NOT
        // (W+2k)/(H+2k) whenever a margin exists).
        final unit = BeadGridLayout(
          width: widget.width,
          height: widget.height,
          borderRings: widget.borderRings,
          cellSize: 1,
        );
        final gridAspect = unit.canvasAspect;
        final viewAspect = _viewport.width / _viewport.height;
        double gw, gh;
        if (gridAspect > viewAspect) {
          gw = _viewport.width;
          gh = gw / gridAspect;
        } else {
          gh = _viewport.height;
          gw = gh * gridAspect;
        }
        final left = (_viewport.width - gw) / 2;
        final top = (_viewport.height - gh) / 2;
        _gridRect1x = Rect.fromLTWH(left, top, gw, gh);
        // Reverse-derive the screen-scale cellSize from the fitted rect (canvas
        // width is linear in cellSize with zero intercept, so this is exact),
        // then build the screen-scale layout: layout.canvasSize == gridRect.size.
        final cellSize = gw / unit.canvasSize.width;
        _layout = BeadGridLayout(
          width: widget.width,
          height: widget.height,
          borderRings: widget.borderRings,
          cellSize: cellSize,
        );

        return ClipRect(
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onScaleStart: _onScaleStart,
            onScaleUpdate: _onScaleUpdate,
            onTapUp: _onTapUp,
            child: Transform(
              // Scale + translate the whole viewport content (canvas + its
              // letterbox whitespace), so zooming makes the grid grow and the
              // whitespace shrink — the photo-album feel.
              transform: Matrix4.identity()
                ..translateByDouble(_offset.dx, _offset.dy, 0, 1)
                ..scaleByDouble(_scale, _scale, 1, 1),
              child: SizedBox(
                width: _viewport.width,
                height: _viewport.height,
                child: Center(
                  child: SizedBox(
                    width: gw,
                    height: gh,
                    child: CustomPaint(
                      painter: _BeadGridPainter(
                        cells: widget.cells,
                        width: widget.width,
                        height: widget.height,
                        borderRings: widget.borderRings,
                        colors: [for (final p in widget.palette) p.rgb],
                        highlightedIndex: widget.highlightedIndex,
                        accent: scheme.primary,
                        lineColor: scheme.outline,
                        boldLineColor: scheme.onSurfaceVariant,
                        borderColor: scheme.surfaceContainerHighest,
                        tickColor: scheme.onSurfaceVariant,
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        );
      },
    );
  }
}

/// Paints the bead grid via [BeadGridLayout]: content cells as solid color
/// squares with visible fine grid-line gaps, every-10 bold separators, 1-based
/// axis labels in the tick margin, k rings of empty border whitespace, plus an
/// optional highlight ring on every content cell sharing the selected index.
class _BeadGridPainter extends CustomPainter {
  _BeadGridPainter({
    required this.cells,
    required this.width,
    required this.height,
    required this.borderRings,
    required this.colors,
    required this.highlightedIndex,
    required this.accent,
    required this.lineColor,
    required this.boldLineColor,
    required this.borderColor,
    required this.tickColor,
  });

  final List<int> cells;
  final int width;
  final int height;
  final int borderRings;
  final List<Color> colors;
  final int? highlightedIndex;
  final Color accent;
  final Color lineColor;
  final Color boldLineColor;
  final Color borderColor;
  final Color tickColor;

  @override
  void paint(Canvas canvas, Size size) {
    // Reverse-derive the same screen-scale layout the state uses (size ==
    // gridRect.size == layout.canvasSize), so draw + hit-test never drift.
    final unit =
        BeadGridLayout(width: width, height: height, borderRings: borderRings, cellSize: 1);
    final cellSize = size.width / unit.canvasSize.width;
    final layout = BeadGridLayout(
        width: width, height: height, borderRings: borderRings, cellSize: cellSize);
    final content = layout.contentRect;
    final fill = Paint()..style = PaintingStyle.fill;

    // Border ring whitespace (light neutral) behind the content.
    if (layout.hasBorder) {
      canvas.drawRect(layout.boardRect, fill..color = borderColor);
    }
    // Content background = line color; shows through cell gaps as fine grid
    // lines so individual cells stay distinguishable (issue: "没有明确的格子").
    canvas.drawRect(content, fill..color = lineColor);

    // Gap shrinks with cell size; capped BELOW the cell so drawn size stays
    // positive even at 1000 beads/side (cellSize can be < 1px there).
    final gap = (cellSize * 0.08).clamp(0.0, 3.0).toDouble();
    final cellGap = math.min(gap, cellSize * 0.5);
    final drawSize = Size(cellSize - cellGap, cellSize - cellGap);
    final inset = cellGap / 2;

    for (int r = 0; r < height; r++) {
      for (int c = 0; c < width; c++) {
        final idx = cells[r * width + c];
        assert(idx >= 0 && idx < colors.length);
        // cells is a Uint16List so idx is always >= 0; the full bound is
        // belt-and-suspenders for the (unreachable) out-of-range case, so a
        // release build degrades to the grid-line color instead of throwing.
        final color =
            (idx >= 0 && idx < colors.length) ? colors[idx] : lineColor;
        fill.color = color;
        canvas.drawRect(
          Offset(content.left + c * cellSize + inset,
                  content.top + r * cellSize + inset) &
              drawSize,
          fill,
        );
      }
    }

    // Every-10 bold separators, drawn after cells so they win at boundaries.
    if (layout.boldColLineXs.isNotEmpty || layout.boldRowLineYs.isNotEmpty) {
      final bold = Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = (cellSize * 0.12).clamp(1.5, 4.0).toDouble()
        ..color = boldLineColor;
      for (final x in layout.boldColLineXs) {
        canvas.drawLine(Offset(x, content.top), Offset(x, content.bottom), bold);
      }
      for (final y in layout.boldRowLineYs) {
        canvas.drawLine(Offset(content.left, y), Offset(content.right, y), bold);
      }
    }

    _paintTicks(canvas, layout, cellSize);

    if (highlightedIndex != null) {
      final stroke = Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = gap.clamp(1.5, 3.0)
        ..color = accent;
      for (int r = 0; r < height; r++) {
        for (int c = 0; c < width; c++) {
          if (cells[r * width + c] == highlightedIndex) {
            canvas.drawRect(
              Offset(content.left + c * cellSize + inset,
                      content.top + r * cellSize + inset) &
                  drawSize,
              stroke,
            );
          }
        }
      }
    }
  }

  /// 1-based axis labels (10, 20, …) in the tick margin: column numbers in the
  /// top margin (right edge at the column's bold-line x), row numbers in the
  /// left margin (bottom edge at the row's bold-line y). Positions come from the
  /// layout's ticks; the preview font is not pixel-matched to the save renderer
  /// (D5 — drift is anchored at the layout, not the glyphs).
  void _paintTicks(Canvas canvas, BeadGridLayout layout, double cellSize) {
    if (layout.colTicks.isEmpty && layout.rowTicks.isEmpty) return;
    // Engine digit height ≈ 5*scale = cellSize; the top margin is 1.4*cellSize
    // and the left margin fits the widest label, so this clears both.
    final style = TextStyle(color: tickColor, fontSize: cellSize, height: 1.0);
    final pad = cellSize / 5; // engine `pad`, keeps the label off the board edge
    TextPainter make(int value) => TextPainter(
          text: TextSpan(text: '$value', style: style),
          textDirection: TextDirection.ltr,
        )..layout();

    // Column labels: right-align to the boundary x, bottom-align to board top.
    for (final t in layout.colTicks) {
      final tp = make(t.value);
      tp.paint(canvas, Offset(t.along - tp.width, layout.marginTopLeft.dy - tp.height));
      tp.dispose();
    }
    // Row labels: right-align inside the left margin, bottom-align to boundary y.
    for (final t in layout.rowTicks) {
      final tp = make(t.value);
      tp.paint(
          canvas, Offset(layout.marginTopLeft.dx - pad - tp.width, t.along - tp.height));
      tp.dispose();
    }
  }

  @override
  bool shouldRepaint(_BeadGridPainter old) =>
      old.highlightedIndex != highlightedIndex ||
      !identical(old.cells, cells) ||
      old.width != width ||
      old.height != height ||
      old.borderRings != borderRings ||
      old.accent != accent ||
      old.lineColor != lineColor ||
      old.boldLineColor != boldLineColor ||
      old.borderColor != borderColor ||
      old.tickColor != tickColor;
}
