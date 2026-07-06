import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../infrastructure/palette_codec.dart' show PaletteColor;

/// A zoomable, tappable bead grid rendered from `BeadPattern.cells` + the
/// parsed palette. Each cell is a solid color square sized to the grid's
/// `width × height`; tap a cell to get its `(row, col, paletteIndex)`.
///
/// **Zoom/pan is self-managed** — a [GestureDetector] drives a [Transform]
/// (translate + scale) directly, rather than using [InteractiveViewer], which
/// did not respond to the emulator's Ctrl-drag pinch here. A [LayoutBuilder]
/// computes the grid's letterboxed rect at 1× (contain fit, centered) so cells
/// stay square; pinch-zoom scales the whole viewport content, so the grid
/// grows and consumes the surrounding whitespace — the familiar photo-album
/// zoom. Pan is clamped so the grid can't be dragged fully off-screen.
///
/// **Cell boundaries are visible** — the painter fills the canvas with the
/// line color first, then draws each cell deflated by a small gap, so the
/// line color shows through as grid lines (scales with cell size, clamped).
///
/// **Data source is the structured `cells` array, NOT the `previewPng`
/// bitmap** — keeps the "never derive info from the rendered image" hard
/// rule (CLAUDE.md rule 3), and gives exact hit-testing at any zoom level.
///
/// `highlightedIndex`, when set, strokes a ring around every cell sharing
/// that palette index — the "高亮同色" affordance driven by the parent
/// (ResultPage owns the selection state).
class BeadGridView extends StatefulWidget {
  const BeadGridView({
    super.key,
    required this.cells,
    required this.width,
    required this.height,
    required this.palette,
    this.highlightedIndex,
    this.onCellTap,
  });

  /// Row-major palette indices (length == [width] * [height]).
  final List<int> cells;

  /// Grid dimensions in beads.
  final int width;
  final int height;

  /// Palette colors indexed by `cells[i]`. Must be the full palette in engine
  /// order (see `parsePalette`).
  final List<PaletteColor> palette;

  /// Palette index to highlight (all matching cells get a ring). null = none.
  final int? highlightedIndex;

  /// Called when the user taps a cell. `(row, col, paletteIndex)`.
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

  // The grid's rect (in viewport space) at 1×, from the last layout pass —
  // used to (a) hit-test taps and (b) clamp pan so the grid can't be dragged
  // fully off-screen.
  Rect _gridRect1x = Rect.zero;
  Size _viewport = Size.zero;

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
    if (_gridRect1x == Rect.zero) return;
    // Invert the transform: scene = (viewportPoint - offset) / scale, then
    // subtract the grid's 1× top-left to land in grid-local coords.
    final scene = (d.localPosition - _offset) / _scale;
    final gx = scene.dx - _gridRect1x.left;
    final gy = scene.dy - _gridRect1x.top;
    final cellW = _gridRect1x.width / widget.width;
    final cellH = _gridRect1x.height / widget.height;
    final col = (gx / cellW).floor();
    final row = (gy / cellH).floor();
    if (col >= 0 && col < widget.width && row >= 0 && row < widget.height) {
      widget.onCellTap?.call(row, col, widget.cells[row * widget.width + col]);
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return LayoutBuilder(
      builder: (context, constraints) {
        _viewport = Size(constraints.maxWidth, constraints.maxHeight);
        // Compute the grid's letterboxed rect at 1× (contain fit, centered).
        final gridAspect = widget.width / widget.height;
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

        return ClipRect(
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onScaleStart: _onScaleStart,
            onScaleUpdate: _onScaleUpdate,
            onTapUp: _onTapUp,
            child: Transform(
              // Scale + translate the whole viewport content (grid + its
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
                        colors: [for (final p in widget.palette) p.rgb],
                        highlightedIndex: widget.highlightedIndex,
                        accent: theme.colorScheme.primary,
                        lineColor: theme.colorScheme.outline,
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

/// Paints the bead grid as solid color squares with visible grid-line gaps,
/// plus an optional highlight ring on every cell sharing the selected palette
/// index.
class _BeadGridPainter extends CustomPainter {
  _BeadGridPainter({
    required this.cells,
    required this.width,
    required this.height,
    required this.colors,
    required this.highlightedIndex,
    required this.accent,
    required this.lineColor,
  });

  final List<int> cells;
  final int width;
  final int height;
  final List<Color> colors;
  final int? highlightedIndex;
  final Color accent;
  final Color lineColor;

  @override
  void paint(Canvas canvas, Size size) {
    final cellW = size.width / width;
    final cellH = size.height / height;
    final fill = Paint()..style = PaintingStyle.fill;

    // Background = line color; shows through the cell gaps as grid lines so
    // the user can distinguish individual cells (issue: "没有明确的格子").
    canvas.drawRect(Offset.zero & size, fill..color = lineColor);

    // Gap shrinks with cell size; capped BELOW the cell so drawW/drawH stay
    // positive even at 1000 beads/side (cellW can be < 1px there). Beads matter
    // more than the separator line at extreme density, so the gap → ~0 is fine.
    final gap = (math.min(cellW, cellH) * 0.08).clamp(0.0, 3.0).toDouble();
    final cellGap = math.min(gap, math.min(cellW, cellH) * 0.5);
    final drawW = cellW - cellGap;
    final drawH = cellH - cellGap;
    final inset = cellGap / 2;

    for (int r = 0; r < height; r++) {
      for (int c = 0; c < width; c++) {
        final idx = cells[r * width + c];
        assert(idx >= 0 && idx < colors.length);
        // cells is a Uint16List so idx is always >= 0; the full bound is
        // belt-and-suspenders for the (unreachable) out-of-range case, so a
        // release build degrades to the grid-line color instead of throwing.
        final color = (idx >= 0 && idx < colors.length)
            ? colors[idx]
            : lineColor;
        fill.color = color;
        canvas.drawRect(
          Offset(c * cellW + inset, r * cellH + inset) & Size(drawW, drawH),
          fill,
        );
      }
    }

    if (highlightedIndex != null) {
      final stroke = Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = gap.clamp(1.5, 3.0)
        ..color = accent;
      for (int r = 0; r < height; r++) {
        for (int c = 0; c < width; c++) {
          if (cells[r * width + c] == highlightedIndex) {
            canvas.drawRect(
              Offset(c * cellW + inset, r * cellH + inset) & Size(drawW, drawH),
              stroke,
            );
          }
        }
      }
    }
  }

  @override
  bool shouldRepaint(_BeadGridPainter old) =>
      old.highlightedIndex != highlightedIndex ||
      !identical(old.cells, cells) ||
      old.width != width ||
      old.height != height ||
      old.accent != accent ||
      old.lineColor != lineColor;
}
