import 'dart:typed_data';

import 'package:flutter/material.dart';

import 'crop_geometry.dart';

/// The pan/zoom/rotate/flip framing state, expressed exactly in the geometry
/// model group A consumes: `zoom` is RELATIVE to cover (>=1), pan is normalized
/// to [-1,1], plus the 90°-step rotation and horizontal-flip flags. On confirm
/// `crop_page` feeds these straight into [computeCropRect] — no Matrix4
/// conversion, avoiding the InteractiveViewer pitfall.
class CropFrameState {
  const CropFrameState({
    required this.aspect,
    required this.zoom,
    required this.panX,
    required this.panY,
    required this.quarterTurns,
    required this.flipH,
  });

  /// Frame aspect = width / height (1.0 = square).
  final double aspect;
  final double zoom;
  final double panX;
  final double panY;
  final int quarterTurns;
  final bool flipH;

  static const initial =
      CropFrameState(aspect: 1, zoom: 1, panX: 0, panY: 0, quarterTurns: 0, flipH: false);
}

/// One aspect choice: portrait ratio + its landscape swap.
class _AspectChoice {
  const _AspectChoice(this.label, this.w, this.h);
  final String label;
  final int w;
  final int h;
  double portrait() => w / h;
  double landscape() => h / w;
}

const _aspects = [
  _AspectChoice('正方形', 1, 1),
  _AspectChoice('2:3', 2, 3),
  _AspectChoice('3:4', 3, 4),
  _AspectChoice('4:5', 4, 5),
  _AspectChoice('9:16', 9, 16),
];

// ponytail: cap zoom so `coverW / zoom` never collapses the crop rect toward 0
// (design 决策 3 maxScale). 8× keeps a real crop for any sane source; raise if
// megapixel sources ever need a tighter framing.
const _maxZoom = 8.0;

/// Fixed-aspect viewfinder (frame stays put) with drag/pinch pan+zoom of the
/// image beneath. minScale = cover (image always fills the frame), so panning
/// can never expose an empty corner. Reports its framing via [onChanged]; the
/// host reads the latest state on confirm.
class CropFrame extends StatefulWidget {
  const CropFrame({
    super.key,
    required this.imageBytes,
    required this.imageSize,
    required this.onChanged,
  });

  /// Source bytes, rendered for preview (the confirm path re-decodes for crop).
  final Uint8List imageBytes;

  /// Source pixel size (un-oriented, as decoded).
  final Size imageSize;

  final ValueChanged<CropFrameState> onChanged;

  @override
  State<CropFrame> createState() => _CropFrameState();
}

class _CropFrameState extends State<CropFrame> {
  double _aspect = 1;
  double _zoom = 1;
  double _panX = 0;
  double _panY = 0;
  int _quarterTurns = 0;
  bool _flipH = false;

  // Stashed each build so gesture handlers can map screen deltas → normalized
  // pan. Frame + oriented dims in px.
  double _frameW = 1, _frameH = 1, _ew = 1, _eh = 1, _coverS = 1;
  double _zoomStart = 1;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _report());
  }

  void _report() {
    if (!mounted) return;
    widget.onChanged(CropFrameState(
      aspect: _aspect,
      zoom: _zoom,
      panX: _panX,
      panY: _panY,
      quarterTurns: _quarterTurns,
      flipH: _flipH,
    ));
  }

  // Oriented source dims (odd quarter-turns swap W/H).
  double get _orientedW =>
      _quarterTurns.isOdd ? widget.imageSize.height : widget.imageSize.width;
  double get _orientedH =>
      _quarterTurns.isOdd ? widget.imageSize.width : widget.imageSize.height;

  void _onScaleStart(ScaleStartDetails d) => _zoomStart = _zoom;

  void _onScaleUpdate(ScaleUpdateDetails d) {
    setState(() {
      _zoom = (_zoomStart * d.scale).clamp(1.0, _maxZoom);
      final s = _coverS * _zoom;
      final slackX = _ew * s - _frameW;
      final slackY = _eh * s - _frameH;
      if (slackX > 0) {
        _panX = (_panX - 2 * d.focalPointDelta.dx / slackX).clamp(-1.0, 1.0);
      }
      if (slackY > 0) {
        _panY = (_panY - 2 * d.focalPointDelta.dy / slackY).clamp(-1.0, 1.0);
      }
    });
    _report();
  }

  void _rotate() {
    // Rotating swaps effective W/H; because zoom is cover-relative and pan is
    // normalized, the model stays valid and the frame stays covered — the next
    // build recomputes cover from the swapped dims (design 决策 3 re-clamp).
    setState(() => _quarterTurns = (_quarterTurns + 1) % 4);
    _report();
  }

  void _flip() {
    setState(() => _flipH = !_flipH);
    _report();
  }

  void _reset() {
    setState(() {
      _aspect = 1;
      _zoom = 1;
      _panX = 0;
      _panY = 0;
      _quarterTurns = 0;
      _flipH = false;
    });
    _report();
  }

  Future<void> _pickAspect() async {
    var landscape = false;
    final chosen = await showModalBottomSheet<double>(
      context: context,
      builder: (ctx) => StatefulBuilder(
        builder: (ctx, setSheet) {
          final scheme = Theme.of(ctx).colorScheme;
          return SafeArea(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Padding(
                  padding: const EdgeInsets.all(12),
                  child: SegmentedButton<bool>(
                    segments: const [
                      ButtonSegment(value: false, label: Text('纵向')),
                      ButtonSegment(value: true, label: Text('横向')),
                    ],
                    selected: {landscape},
                    onSelectionChanged: (s) => setSheet(() => landscape = s.first),
                  ),
                ),
                for (final a in _aspects)
                  ListTile(
                    leading: Icon(Icons.crop, color: scheme.primary),
                    title: Text(a.w == a.h
                        ? a.label
                        : (landscape ? '${a.h}:${a.w}' : a.label)),
                    onTap: () => Navigator.pop(
                        ctx, landscape ? a.landscape() : a.portrait()),
                  ),
              ],
            ),
          );
        },
      ),
    );
    if (chosen != null && mounted) {
      setState(() => _aspect = chosen);
      _report();
    }
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Column(
      children: [
        Expanded(
          child: LayoutBuilder(
            builder: (context, constraints) {
              // Largest frame of `_aspect` fitting the area (with margin).
              final avail = Size(
                constraints.maxWidth - 32,
                constraints.maxHeight - 32,
              );
              var fw = avail.width;
              var fh = fw / _aspect;
              if (fh > avail.height) {
                fh = avail.height;
                fw = fh * _aspect;
              }
              final frame = Rect.fromCenter(
                center: Offset(constraints.maxWidth / 2, constraints.maxHeight / 2),
                width: fw,
                height: fh,
              );

              _ew = _orientedW;
              _eh = _orientedH;
              _frameW = fw;
              _frameH = fh;
              // coverMinScale (2.1) returns frame-normalized units (height=1);
              // multiply by real frame height (px) → screen px per image px.
              _coverS = coverMinScale(_ew, _eh, _aspect) * fh;

              final s = _coverS * _zoom;
              final dispW = _ew * s;
              final dispH = _eh * s;
              final slackX = dispW - fw;
              final slackY = dispH - fh;
              final left =
                  frame.center.dx - dispW / 2 - _panX * slackX / 2;
              final top =
                  frame.center.dy - dispH / 2 - _panY * slackY / 2;

              return GestureDetector(
                onScaleStart: _onScaleStart,
                onScaleUpdate: _onScaleUpdate,
                child: Stack(
                  clipBehavior: Clip.hardEdge,
                  children: [
                    Positioned(
                      left: left,
                      top: top,
                      width: dispW,
                      height: dispH,
                      child: Transform.scale(
                        scaleX: _flipH ? -1.0 : 1.0,
                        child: RotatedBox(
                          quarterTurns: _quarterTurns,
                          child: Image.memory(
                            widget.imageBytes,
                            fit: BoxFit.fill,
                            gaplessPlayback: true,
                          ),
                        ),
                      ),
                    ),
                    Positioned.fill(
                      child: IgnorePointer(
                        child: CustomPaint(
                          painter: _MaskPainter(
                            frame,
                            scrim: scheme.scrim.withValues(alpha: 0.54),
                            grid: scheme.onSurface.withValues(alpha: 0.5),
                            bracket: scheme.primary,
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              );
            },
          ),
        ),
        _toolbar(),
      ],
    );
  }

  Widget _toolbar() {
    return SafeArea(
      top: false,
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 8),
        child: Row(
          mainAxisAlignment: MainAxisAlignment.spaceEvenly,
          children: [
            _tool(Icons.aspect_ratio, '比例', _pickAspect),
            _tool(Icons.rotate_90_degrees_cw, '旋转', _rotate),
            _tool(Icons.flip, '翻转', _flip),
            _tool(Icons.restart_alt, '重置', _reset),
          ],
        ),
      ),
    );
  }

  Widget _tool(IconData icon, String label, VoidCallback onTap) {
    final scheme = Theme.of(context).colorScheme;
    return TextButton(
      onPressed: onTap,
      style: TextButton.styleFrom(foregroundColor: scheme.primary),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [Icon(icon), Text(label, style: const TextStyle(fontSize: 12))],
      ),
    );
  }
}

/// Dims everything outside [frame], then draws the rule-of-thirds grid and
/// bright corner brackets over the frame.
class _MaskPainter extends CustomPainter {
  _MaskPainter(
    this.frame, {
    required this.scrim,
    required this.grid,
    required this.bracket,
  });
  final Rect frame;
  final Color scrim;
  final Color grid;
  final Color bracket;

  @override
  void paint(Canvas canvas, Size size) {
    final scrimPath = Path()..addRect(Offset.zero & size);
    final hole = Path()..addRect(frame);
    canvas.drawPath(
      Path.combine(PathOperation.difference, scrimPath, hole),
      Paint()..color = scrim,
    );

    final gridPaint = Paint()
      ..color = grid
      ..strokeWidth = 1;
    for (var i = 1; i < 3; i++) {
      final dx = frame.left + frame.width * i / 3;
      final dy = frame.top + frame.height * i / 3;
      canvas.drawLine(Offset(dx, frame.top), Offset(dx, frame.bottom), gridPaint);
      canvas.drawLine(Offset(frame.left, dy), Offset(frame.right, dy), gridPaint);
    }

    final cornerPaint = Paint()
      ..color = bracket
      ..strokeWidth = 3
      ..strokeCap = StrokeCap.round;
    const len = 18.0;
    void corner(Offset o, double sx, double sy) {
      canvas.drawLine(o, o.translate(len * sx, 0), cornerPaint);
      canvas.drawLine(o, o.translate(0, len * sy), cornerPaint);
    }

    corner(frame.topLeft, 1, 1);
    corner(frame.topRight, -1, 1);
    corner(frame.bottomLeft, 1, -1);
    corner(frame.bottomRight, -1, -1);
  }

  @override
  bool shouldRepaint(_MaskPainter old) =>
      old.frame != frame ||
      old.scrim != scrim ||
      old.grid != grid ||
      old.bracket != bracket;
}
