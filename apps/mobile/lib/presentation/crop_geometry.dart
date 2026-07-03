import 'dart:math' as math;

/// Pure crop-coordinate geometry for the self-drawn cropper.
///
/// No Flutter/render dependency — host-testable under `flutter test`.
///
/// Coordinate convention (pinned by design 决策 2):
/// the crop [CropRect] is expressed in the **oriented image**'s pixel space —
/// i.e. after `copyRotate` (90° steps) then `flipHorizontal` have been applied
/// to the decoded source. That is exactly the space `image.copyCrop` operates
/// in. Callers must therefore first orient the bytes, then crop with this rect.
///
/// Pan/zoom convention (the widget in group B must feed these):
/// - [quarterTurns]: rotation in 90° steps. Odd turns swap effective W/H.
///   (Flip is NOT a param — it is baked into the oriented bytes by the caller
///   via `flipHorizontal`, so it does not change the rect, only the pixels.)
/// - [zoom]: scale **relative to cover** (1.0 = cover minScale = image just
///   fills the frame). Always >= 1; larger = more zoomed in = smaller crop.
/// - [panX]/[panY]: normalized in [-1, 1]. 0 = centered; -1 = crop flush to the
///   min edge (left/top), +1 = flush to the max edge (right/bottom). Slack is 0
///   along any axis the cover rect already spans, so pan there is a no-op.
class CropRect {
  const CropRect(this.x, this.y, this.width, this.height);

  final int x;
  final int y;
  final int width;
  final int height;

  @override
  bool operator ==(Object other) =>
      other is CropRect &&
      other.x == x &&
      other.y == y &&
      other.width == width &&
      other.height == height;

  @override
  int get hashCode => Object.hash(x, y, width, height);

  @override
  String toString() => 'CropRect(x: $x, y: $y, w: $width, h: $height)';
}

/// Minimum scale so an image of [effectiveW] x [effectiveH] fully covers a
/// frame of aspect [frameAspect] (= frameW / frameH).
///
/// Returned in units of "frame-height per image pixel" (frame normalized to
/// height 1, width [frameAspect]); the widget multiplies by the real frame
/// height in px. Matches design 决策 3: `max(frameW/effW, frameH/effH)`.
///
/// After a 90°/270° rotation the caller passes the **swapped** effective dims,
/// so the frame stays covered post-rotation.
double coverMinScale(double effectiveW, double effectiveH, double frameAspect) {
  return math.max(frameAspect / effectiveW, 1.0 / effectiveH);
}

/// Maps a view framing (pan/zoom + rotate/flip flags) to the crop rectangle in
/// oriented-image pixel space, clamped in bounds with `width,height >= 1`.
CropRect computeCropRect({
  required int srcWidth,
  required int srcHeight,
  required double frameAspect,
  required double zoom,
  double panX = 0.0,
  double panY = 0.0,
  int quarterTurns = 0,
}) {
  // 1. Oriented (post-rotation) dims. Odd 90° turns swap W/H.
  final bool swap = quarterTurns.abs() % 2 == 1;
  final int imgW = swap ? srcHeight : srcWidth;
  final int imgH = swap ? srcWidth : srcHeight;

  // 2. Cover rect at zoom=1: largest rect of aspect frameAspect inside the
  //    oriented image (touches two opposite edges), centered.
  final double coverW = math.min(imgW.toDouble(), frameAspect * imgH);
  final double coverH = math.min(imgH.toDouble(), imgW / frameAspect);

  // 3. Zoom in (never below cover) shrinks the crop.
  final double z = math.max(zoom, 1.0);
  final double cropW = coverW / z;
  final double cropH = coverH / z;

  // 4. Pan within the leftover slack.
  final double slackX = math.max(0.0, imgW - cropW);
  final double slackY = math.max(0.0, imgH - cropH);
  final double px = panX.clamp(-1.0, 1.0);
  final double py = panY.clamp(-1.0, 1.0);
  final double x = slackX / 2.0 * (1.0 + px);
  final double y = slackY / 2.0 * (1.0 + py);

  // 5. Round then clamp: w,h in [1, img]; x,y keep x+w / y+h in bounds.
  int wi = cropW.round().clamp(1, imgW);
  int hi = cropH.round().clamp(1, imgH);
  final int xi = x.round().clamp(0, imgW - wi);
  final int yi = y.round().clamp(0, imgH - hi);
  return CropRect(xi, yi, wi, hi);
}
