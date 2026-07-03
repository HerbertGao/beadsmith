import 'package:beadsmith/presentation/crop_geometry.dart';
import 'package:flutter_test/flutter_test.dart';

/// Asserts a crop rect stays inside an imgW x imgH image with w,h >= 1.
void expectInBounds(CropRect r, int imgW, int imgH) {
  expect(r.width, greaterThanOrEqualTo(1), reason: 'w>=1 ($r)');
  expect(r.height, greaterThanOrEqualTo(1), reason: 'h>=1 ($r)');
  expect(r.x, greaterThanOrEqualTo(0), reason: 'x>=0 ($r)');
  expect(r.y, greaterThanOrEqualTo(0), reason: 'y>=0 ($r)');
  expect(r.x + r.width, lessThanOrEqualTo(imgW), reason: 'x+w<=imgW ($r)');
  expect(r.y + r.height, lessThanOrEqualTo(imgH), reason: 'y+h<=imgH ($r)');
}

void main() {
  group('computeCropRect', () {
    test('identity: square image + square frame at cover = whole image', () {
      final r = computeCropRect(
        srcWidth: 100,
        srcHeight: 100,
        frameAspect: 1.0,
        zoom: 1.0,
      );
      expect(r, const CropRect(0, 0, 100, 100));
      expectInBounds(r, 100, 100);
    });

    test('2x zoom centered = centered half', () {
      final r = computeCropRect(
        srcWidth: 100,
        srcHeight: 100,
        frameAspect: 1.0,
        zoom: 2.0,
      );
      expect(r, const CropRect(25, 25, 50, 50));
      expectInBounds(r, 100, 100);
    });

    test('pan to corners at 2x zoom', () {
      final tl = computeCropRect(
        srcWidth: 100,
        srcHeight: 100,
        frameAspect: 1.0,
        zoom: 2.0,
        panX: -1.0,
        panY: -1.0,
      );
      expect(tl, const CropRect(0, 0, 50, 50));

      final br = computeCropRect(
        srcWidth: 100,
        srcHeight: 100,
        frameAspect: 1.0,
        zoom: 2.0,
        panX: 1.0,
        panY: 1.0,
      );
      expect(br, const CropRect(50, 50, 50, 50));
      expectInBounds(br, 100, 100);
    });

    test('pan beyond [-1,1] is clamped to the corner', () {
      final r = computeCropRect(
        srcWidth: 100,
        srcHeight: 100,
        frameAspect: 1.0,
        zoom: 2.0,
        panX: -9.0,
        panY: 9.0,
      );
      expect(r, const CropRect(0, 50, 50, 50));
      expectInBounds(r, 100, 100);
    });

    test('90 deg rotation swaps effective dims', () {
      // landscape 200x100 -> oriented portrait 100x200; square frame cover.
      final r = computeCropRect(
        srcWidth: 200,
        srcHeight: 100,
        frameAspect: 1.0,
        zoom: 1.0,
        quarterTurns: 1,
      );
      // cover = 100x100 centered vertically in 100x200 oriented image.
      expect(r, const CropRect(0, 50, 100, 100));
      expectInBounds(r, 100, 200);
    });

    test('270 deg rotation swaps dims the same way', () {
      final r = computeCropRect(
        srcWidth: 200,
        srcHeight: 100,
        frameAspect: 1.0,
        zoom: 1.0,
        quarterTurns: 3,
      );
      expect(r, const CropRect(0, 50, 100, 100));
      expectInBounds(r, 100, 200);
    });

    test('pan convention: +1 = max edge (right/bottom), matching CropFrame', () {
      // The widget reports panX/panY so +1 frames the image's max edge; the
      // rect must land at the max edge too — regression guard for the
      // widget<->geometry sign agreement (the bug this replaces).
      final maxEdge = computeCropRect(
        srcWidth: 100,
        srcHeight: 100,
        frameAspect: 1.0,
        zoom: 2.0,
        panX: 1.0,
      );
      expect(maxEdge, const CropRect(50, 25, 50, 50));
      expectInBounds(maxEdge, 100, 100);
    });

    test('aspect ratios: square / 3:4 / 9:16 and landscape on a 120x120 image',
        () {
      const w = 120, h = 120;
      // 3:4 portrait (a=0.75): width-limited -> cropW=120, cropH=160? no, must
      // fit: coverH=min(120,120/0.75=160)=120, coverW=0.75*120=90.
      final p34 = computeCropRect(
          srcWidth: w, srcHeight: h, frameAspect: 3 / 4, zoom: 1.0);
      expect(p34, const CropRect(15, 0, 90, 120));
      expect(p34.width / p34.height, closeTo(3 / 4, 0.02));
      expectInBounds(p34, w, h);

      // 9:16 portrait (a=0.5625): coverW=0.5625*120=67.5, coverH=120.
      final p916 = computeCropRect(
          srcWidth: w, srcHeight: h, frameAspect: 9 / 16, zoom: 1.0);
      expect(p916, const CropRect(26, 0, 68, 120));
      expectInBounds(p916, w, h);

      // 4:3 landscape (a=1.333): coverW=120, coverH=90.
      final l43 = computeCropRect(
          srcWidth: w, srcHeight: h, frameAspect: 4 / 3, zoom: 1.0);
      expect(l43, const CropRect(0, 15, 120, 90));
      expect(l43.width / l43.height, closeTo(4 / 3, 0.02));
      expectInBounds(l43, w, h);
    });

    test('combined: zoom + pan + 90 rotation', () {
      // src 300x150 -> oriented 150x300. frame 3:4 (a=0.75).
      // cover: coverW=min(150, 0.75*300=225)=150, coverH=min(300,150/0.75=200)=200.
      // zoom 2: cropW=75, cropH=100. slackX=75, slackY=200.
      // panX=1 -> x=slackX=75 (clamped to imgW-w=75). panY=-1 -> y=0.
      final r = computeCropRect(
        srcWidth: 300,
        srcHeight: 150,
        frameAspect: 3 / 4,
        zoom: 2.0,
        panX: 1.0,
        panY: -1.0,
        quarterTurns: 1,
      );
      expect(r, const CropRect(75, 0, 75, 100));
      expectInBounds(r, 150, 300);
    });

    test('extreme zoom does not degenerate (w,h >= 1, in bounds)', () {
      final r = computeCropRect(
        srcWidth: 100,
        srcHeight: 100,
        frameAspect: 1.0,
        zoom: 1e9,
        panX: 1.0,
        panY: 1.0,
      );
      expect(r.width, greaterThanOrEqualTo(1));
      expect(r.height, greaterThanOrEqualTo(1));
      expectInBounds(r, 100, 100);
    });

    test('extreme zoom on non-square oriented image after rotation', () {
      final r = computeCropRect(
        srcWidth: 640,
        srcHeight: 480,
        frameAspect: 9 / 16,
        zoom: 5000.0,
        panX: -1.0,
        panY: 1.0,
        quarterTurns: 1,
      );
      // oriented dims 480x640.
      expect(r.width, greaterThanOrEqualTo(1));
      expect(r.height, greaterThanOrEqualTo(1));
      expectInBounds(r, 480, 640);
    });
  });

  group('coverMinScale', () {
    // A scale s covers the frame (height 1, width a) iff effW*s >= a and
    // effH*s >= 1.
    void expectCovers(double s, double effW, double effH, double a) {
      expect(effW * s, greaterThanOrEqualTo(a - 1e-9), reason: 'covers width');
      expect(effH * s, greaterThanOrEqualTo(1 - 1e-9), reason: 'covers height');
    }

    test('matches max(a/effW, 1/effH)', () {
      expect(coverMinScale(200, 100, 1.0), closeTo(1 / 100, 1e-12));
      expect(coverMinScale(100, 200, 1.0), closeTo(1 / 100, 1e-12));
    });

    test('the returned scale actually covers the frame', () {
      const a = 3 / 4;
      final s = coverMinScale(300, 150, a);
      expectCovers(s, 300, 150, a);
    });

    test('after 90 rotation, using swapped dims still covers the frame', () {
      const a = 9 / 16;
      const effW = 640.0, effH = 480.0; // original landscape
      // rotate 90 -> swapped effective dims
      const rotW = effH, rotH = effW; // 480 x 640
      final sBefore = coverMinScale(effW, effH, a);
      final sAfter = coverMinScale(rotW, rotH, a);
      expectCovers(sBefore, effW, effH, a);
      expectCovers(sAfter, rotW, rotH, a);
      // orientation changed the required scale (regression guard against
      // forgetting to recompute on rotation).
      expect(sAfter, isNot(closeTo(sBefore, 1e-6)));
    });
  });
}
