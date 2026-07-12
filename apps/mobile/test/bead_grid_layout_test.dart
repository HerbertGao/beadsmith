import 'package:beadsmith/presentation/bead_grid_layout.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('canvasAspect is scale-independent (cellSize cancels)', () {
    // Non-square + border so both margins and k are exercised.
    final a = BeadGridLayout(width: 30, height: 45, borderRings: 2, cellSize: 8);
    final b =
        BeadGridLayout(width: 30, height: 45, borderRings: 2, cellSize: 3.7);
    expect(a.canvasAspect, closeTo(b.canvasAspect, 1e-9));
    // Same for the drift anchors: tick values + normalized bold-line positions.
    expect(a.colTicks.map((t) => t.value).toList(),
        b.colTicks.map((t) => t.value).toList());
    expect(a.boldColLineXs.first / a.canvasSize.width,
        closeTo(b.boldColLineXs.first / b.canvasSize.width, 1e-9));
  });

  test('square W=H (≥10) with margin has canvasAspect ≠ 1', () {
    // Left margin (H≥10) is wider than top margin, so a square is NOT 1:1.
    final l = BeadGridLayout(width: 20, height: 20, borderRings: 0, cellSize: 6);
    expect(l.marginTopLeft.dx, greaterThan(0));
    expect(l.marginTopLeft.dy, greaterThan(0));
    expect(l.canvasAspect, isNot(closeTo(1.0, 1e-6)));
  });

  test('hit-test round-trip: board cell center maps back to (row,col)', () {
    final l = BeadGridLayout(width: 25, height: 40, borderRings: 3, cellSize: 9);
    for (final rc in const [(0, 0), (39, 24), (17, 5), (0, 24)]) {
      final center = l.contentCellCenter(rc.$1, rc.$2);
      final hit = l.contentCellAt(center);
      expect(hit, isNotNull);
      expect(hit!.row, rc.$1);
      expect(hit.col, rc.$2);
    }
    // Points in the margin, a border ring, and off-board are no-ops (null).
    expect(l.contentCellAt(Offset.zero), isNull); // top-left margin
    expect(l.contentCellAt(l.marginTopLeft + const Offset(1, 1)),
        isNull); // first border ring
    expect(l.contentCellAt(l.boardRect.bottomRight + const Offset(1, 1)),
        isNull); // off-board
  });

  test('W<10 axis has no ticks/margin; H≥10 axis keeps them', () {
    final l = BeadGridLayout(width: 8, height: 30, borderRings: 0, cellSize: 10);
    expect(l.colTicks, isEmpty); // W<10 → no column ticks
    expect(l.boldColLineXs, isEmpty);
    expect(l.marginTopLeft.dy, 0); // no top margin
    expect(l.rowTicks.map((t) => t.value), [10, 20, 30]); // H≥10 keeps rows
    expect(l.marginTopLeft.dx, greaterThan(0));
  });

  test('margins match engine integer geometry at integer scale (13×13 cell=10)',
      () {
    // renderer/mod.rs: margin_left=18, margin_top=14 for 13×13 cell=10.
    final l = BeadGridLayout(width: 13, height: 13, borderRings: 0, cellSize: 10);
    expect(l.marginTopLeft.dx, closeTo(18, 1e-9));
    expect(l.marginTopLeft.dy, closeTo(14, 1e-9));
  });

  // ── 5.2 ①② bold-line + tick existence (both axes / neither / mixed) ──
  // The painter draws bold lines/ticks from exactly these layout lists, so
  // asserting them is equivalent to asserting the painted every-10 lines & labels.

  test('5.2① W,H≥10: both axes have every-10 bold lines + 1-based ticks', () {
    final l = BeadGridLayout(width: 20, height: 20, borderRings: 0, cellSize: 7);
    expect(l.colTicks.map((t) => t.value), [10, 20]);
    expect(l.rowTicks.map((t) => t.value), [10, 20]);
    expect(l.boldColLineXs.length, 2);
    expect(l.boldRowLineYs.length, 2);
    // Every tick's `along` coincides with its bold separator line (painter anchors
    // the right/bottom-aligned number there).
    expect(l.colTicks.map((t) => t.along).toList(), l.boldColLineXs);
    expect(l.rowTicks.map((t) => t.along).toList(), l.boldRowLineYs);
    expect(l.marginTopLeft.dx, greaterThan(0));
    expect(l.marginTopLeft.dy, greaterThan(0));
  });

  test('5.2② W<10 AND H<10: no bold lines / no ticks / no margin on either axis',
      () {
    final l = BeadGridLayout(width: 5, height: 5, borderRings: 0, cellSize: 12);
    expect(l.colTicks, isEmpty);
    expect(l.rowTicks, isEmpty);
    expect(l.boldColLineXs, isEmpty);
    expect(l.boldRowLineYs, isEmpty);
    expect(l.marginTopLeft, Offset.zero);
  });

  test('5.2② mixed W≥10,H<10: top axis keeps col ticks, left axis drops row ticks',
      () {
    // Opposite direction to group A's 8×30 (W<10,H≥10) — covers the other gate.
    final l = BeadGridLayout(width: 30, height: 5, borderRings: 0, cellSize: 8);
    expect(l.colTicks.map((t) => t.value), [10, 20, 30]);
    expect(l.boldColLineXs.length, 3);
    expect(l.rowTicks, isEmpty); // H<10 → no row ticks
    expect(l.boldRowLineYs, isEmpty);
    expect(l.marginTopLeft.dy, greaterThan(0)); // top margin (col labels)
    expect(l.marginTopLeft.dx, 0); // no left margin (no row labels)
  });

  // ── 5.3 border whitespace + non-square geometry (layout level) ──

  test('5.3 k 0→2 insets contentRect from boardRect by k rings (whitespace)', () {
    const w = 12, h = 20, cs = 9.0;
    final k0 = BeadGridLayout(width: w, height: h, borderRings: 0, cellSize: cs);
    // k=0: content fills the board exactly.
    expect(k0.contentRect, k0.boardRect);

    final k2 = BeadGridLayout(width: w, height: h, borderRings: 2, cellSize: cs);
    final b = k2.boardRect, c = k2.contentRect;
    // Two empty rings on every side = 2*cellSize of whitespace all around.
    expect(c.left - b.left, closeTo(2 * cs, 1e-9));
    expect(c.top - b.top, closeTo(2 * cs, 1e-9));
    expect(b.right - c.right, closeTo(2 * cs, 1e-9));
    expect(b.bottom - c.bottom, closeTo(2 * cs, 1e-9));
    // Bead count is content-only → invariant to k (border holds no beads).
    expect(k2.width * k2.height, k0.width * k0.height);
    // …while the board grows by the border rings.
    expect(k2.boardCols * k2.boardRows,
        greaterThan(k0.boardCols * k0.boardRows));
  });

  test('5.3 non-square W≠H + k>0: cells stay square (single cellSize)', () {
    const w = 12, h = 20, k = 2, cs = 9.0;
    final l = BeadGridLayout(width: w, height: h, borderRings: k, cellSize: cs);
    // One cellSize governs both axes → every content cell is cs×cs.
    expect(l.contentRect.width / w, closeTo(cs, 1e-9));
    expect(l.contentRect.height / h, closeTo(cs, 1e-9));
    expect(l.contentRect.width / w, closeTo(l.contentRect.height / h, 1e-9));
  });

  test('5.3 non-square + margin + k: hit-test strips margin AND k (round trip)',
      () {
    // W,H≥10 so BOTH tick margins exist; k>0 so a border ring exists too.
    // New angle vs group A's 25×40 k=3.
    final l = BeadGridLayout(width: 12, height: 20, borderRings: 2, cellSize: 9);
    for (final rc in const [(0, 0), (19, 11), (10, 3), (0, 11)]) {
      final hit = l.contentCellAt(l.contentCellCenter(rc.$1, rc.$2));
      expect(hit, isNotNull);
      expect(hit!.row, rc.$1);
      expect(hit.col, rc.$2);
      // Content coords stay 1..W / 1..H after the caller's +1.
      expect(hit.row + 1, inInclusiveRange(1, 20));
      expect(hit.col + 1, inInclusiveRange(1, 12));
    }
    // Margin, a border-ring point, and off-board are all no-ops.
    expect(l.contentCellAt(Offset.zero), isNull); // top-left tick margin
    expect(l.contentCellAt(l.marginTopLeft + const Offset(1, 1)),
        isNull); // first border ring
    expect(l.contentCellAt(l.boardRect.bottomRight + const Offset(1, 1)),
        isNull); // off-board
  });
}
