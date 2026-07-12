import 'package:beadsmith/infrastructure/palette_codec.dart' show PaletteColor;
import 'package:beadsmith/presentation/bead_grid_view.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

/// Locks down BeadGridView tap hit-testing: a tap at a known offset resolves
/// to the correct (row, col, paletteIndex) through the self-managed
/// Transform → grid-local inversion (at 1× the transform is identity).
void main() {
  testWidgets('tap reports the correct row/col/paletteIndex', (tester) async {
    int? row, col, idx;
    await tester.pumpWidget(
      MaterialApp(
        home: Center(
          child: SizedBox(
            width: 200,
            height: 200,
            // 2×2 grid, square aspect → fills the 200×200 box exactly,
            // cellW = cellH = 100, no InteractiveViewer centering offset.
            child: BeadGridView(
              cells: [0, 1, 2, 3], // row-major: (0,0)=0 (0,1)=1 (1,0)=2 (1,1)=3
              width: 2,
              height: 2,
              palette: [
                for (var i = 0; i < 4; i++)
                  PaletteColor(
                      code: 'S$i', name: 'C$i', rgb: Color(0xFF000000 | i)),
              ],
              onCellTap: (r, c, i) {
                row = r;
                col = c;
                idx = i;
              },
            ),
          ),
        ),
      ),
    );

    final topLeft = tester.getTopLeft(find.byType(BeadGridView));

    // Top-left quadrant center → cell (0,0) → cells[0] = 0.
    await tester.tapAt(topLeft + const Offset(50, 50));
    expect(row, 0, reason: 'row');
    expect(col, 0, reason: 'col');
    expect(idx, 0, reason: 'paletteIndex');

    // Bottom-right quadrant center → cell (1,1) → cells[3] = 3.
    await tester.tapAt(topLeft + const Offset(150, 150));
    expect(row, 1);
    expect(col, 1);
    expect(idx, 3);

    // Top-right quadrant center → cell (0,1) → cells[1] = 1.
    await tester.tapAt(topLeft + const Offset(150, 50));
    expect(row, 0);
    expect(col, 1);
    expect(idx, 1);
  });

  testWidgets('two-finger pinch scales the grid (zoom responds)',
      (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Center(
            child: SizedBox(
              width: 300,
              height: 300,
              child: BeadGridView(
                cells: List<int>.generate(4 * 4, (i) => i % 4),
                width: 4,
                height: 4,
                palette: [
                  for (var i = 0; i < 4; i++)
                    PaletteColor(
                        code: 'S$i', name: 'C$i', rgb: Color(0xFF000000 | i)),
                ],
              ),
            ),
          ),
        ),
      ),
    );

    final center = tester.getCenter(find.byType(BeadGridView));
    // The scale lives in the Transform's matrix (Transform doesn't change the
    // child's layout size, only its painting), so read the matrix directly.
    double currentScale() {
      final t = tester.widget<Transform>(find.descendant(
          of: find.byType(BeadGridView), matching: find.byType(Transform)));
      return t.transform.getMaxScaleOnAxis();
    }

    expect(currentScale(), closeTo(1.0, 1e-6), reason: 'starts at 1×');

    // Two pointers moving apart from the center = pinch-out (zoom in).
    final g1 = await tester.startGesture(center - const Offset(20, 0));
    final g2 = await tester.startGesture(center + const Offset(20, 0));
    await tester.pump();
    for (var i = 0; i < 5; i++) {
      await g1.moveBy(const Offset(-20, 0));
      await g2.moveBy(const Offset(20, 0));
      await tester.pump();
    }
    await g1.up();
    await g2.up();
    await tester.pump();

    // The Transform's scale must now be > 1× — pinch was recognized and drove
    // the self-managed zoom (the thing InteractiveViewer failed to do here).
    expect(currentScale(), greaterThan(1.0),
        reason: 'pinch-out should scale the grid up');
  });

  testWidgets('pan after zoom accumulates the full drag (FIX A)',
      (tester) async {
    int? tappedRow, tappedCol;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Center(
            child: SizedBox(
              width: 300,
              height: 300,
              child: BeadGridView(
                cells: List<int>.generate(4 * 4, (i) => i % 4),
                width: 4,
                height: 4,
                palette: [
                  for (var i = 0; i < 4; i++)
                    PaletteColor(
                        code: 'S$i', name: 'C$i', rgb: Color(0xFF000000 | i)),
                ],
                onCellTap: (r, c, _) {
                  tappedRow = r;
                  tappedCol = c;
                },
              ),
            ),
          ),
        ),
      ),
    );

    final center = tester.getCenter(find.byType(BeadGridView));
    double translationX() {
      final t = tester.widget<Transform>(find.descendant(
          of: find.byType(BeadGridView), matching: find.byType(Transform)));
      return t.transform.getTranslation().x;
    }

    // Zoom in (pinch-out about the center) so the grid overflows the viewport
    // and there is room to pan — at 1× the clamp pins a fully-fitting grid.
    final g1 = await tester.startGesture(center - const Offset(20, 0));
    final g2 = await tester.startGesture(center + const Offset(20, 0));
    await tester.pump();
    for (var i = 0; i < 5; i++) {
      await g1.moveBy(const Offset(-20, 0));
      await g2.moveBy(const Offset(20, 0));
      await tester.pump();
    }
    await g1.up();
    await g2.up();
    await tester.pump();

    final xBeforePan = translationX();

    // Single-finger pan of 100px total across 5 frames. With FIX A the offset
    // follows the CURRENT focal position, so it moves by the full ~100px; the
    // pre-fix code added only the per-frame focalPointDelta and would move ~20px
    // (one frame). Asserting > 50px separates the fixed path from the bug.
    final pan = await tester.startGesture(center);
    await tester.pump();
    for (var i = 0; i < 5; i++) {
      await pan.moveBy(const Offset(20, 0));
      await tester.pump();
    }
    await pan.up();
    await tester.pump();

    expect(translationX() - xBeforePan, greaterThan(50),
        reason: 'pan after zoom must accumulate the full drag, not one frame');

    // Hit-test inversion still routes a tap to an in-range cell after zoom+pan.
    await tester.tapAt(center);
    expect(tappedRow, isNotNull);
    expect(tappedCol, isNotNull);
  });

  // ── 5.2 ③④ content tap → 1-based detail; border tap → no-op ──
  // W=H=4 (<10 → no tick margin) + k=1: the whole canvas is a 6×6 board that
  // fills a square 300×300 box exactly, cellSize=50. Content is the inner 4×4
  // ring-in-1; the outer ring is empty border. Clean, exact hit-test math.
  testWidgets('5.2③④ tap content → 0-based (+1=1-based); tap border ring → no-op',
      (tester) async {
    int? row, col, idx;
    int callbacks = 0;
    await tester.pumpWidget(
      MaterialApp(
        home: Center(
          child: SizedBox(
            width: 300,
            height: 300,
            child: BeadGridView(
              cells: List<int>.generate(4 * 4, (i) => i % 3),
              width: 4,
              height: 4,
              borderRings: 1,
              palette: [
                for (var i = 0; i < 3; i++)
                  PaletteColor(
                      code: 'S$i', name: 'C$i', rgb: Color(0xFF000000 | i)),
              ],
              onCellTap: (r, c, i) {
                row = r;
                col = c;
                idx = i;
                callbacks++;
              },
            ),
          ),
        ),
      ),
    );

    final topLeft = tester.getTopLeft(find.byType(BeadGridView));

    // Content cell (0,0) sits at board cell (1,1) → center at (50+25, 50+25).
    await tester.tapAt(topLeft + const Offset(75, 75));
    expect(callbacks, 1, reason: 'content tap fires detail');
    expect(row, 0);
    expect(col, 0);
    expect(idx, 0);
    // The UI shows 1-based: (row+1, col+1) = (1, 1).
    expect(row! + 1, 1);
    expect(col! + 1, 1);

    // Content cell (3,3) = board (4,4) → center (225,225). Max 1-based = (4,4).
    await tester.tapAt(topLeft + const Offset(225, 225));
    expect(callbacks, 2);
    expect(row! + 1, 4);
    expect(col! + 1, 4);

    // Border ring taps must NOT fire the detail callback.
    await tester.tapAt(topLeft + const Offset(25, 25)); // top-left border corner
    await tester.tapAt(topLeft + const Offset(125, 25)); // top-edge border
    await tester.tapAt(topLeft + const Offset(275, 275)); // bottom-right border
    expect(callbacks, 2, reason: 'taps in the k=1 border ring are no-ops');
  });

  // ── 5.3 non-square W≠H + k>0 widget hit-test (k offset stripped) ──
  // W=6,H=4 (both <10 → no margin) + k=1 → 8×6 board, aspect 4:3. A 320×240 box
  // (also 4:3) fits it exactly, cellSize=40, cells square. Content is inner 6×4.
  testWidgets('5.3 non-square + k>0: content tap maps correctly, border no-op',
      (tester) async {
    int? row, col;
    int callbacks = 0;
    await tester.pumpWidget(
      MaterialApp(
        home: Center(
          child: SizedBox(
            width: 320,
            height: 240,
            child: BeadGridView(
              cells: List<int>.generate(6 * 4, (i) => i % 2),
              width: 6,
              height: 4,
              borderRings: 1,
              palette: [
                for (var i = 0; i < 2; i++)
                  PaletteColor(
                      code: 'S$i', name: 'C$i', rgb: Color(0xFF000000 | i)),
              ],
              onCellTap: (r, c, _) {
                row = r;
                col = c;
                callbacks++;
              },
            ),
          ),
        ),
      ),
    );

    final topLeft = tester.getTopLeft(find.byType(BeadGridView));
    // Content (2,3): board cell (3,4) → center ((1+3+0.5)*40, (1+2+0.5)*40)
    // = (180, 140). Hit-test must strip the k=1 ring to land on (2,3).
    await tester.tapAt(topLeft + const Offset(180, 140));
    expect(callbacks, 1);
    expect(row, 2);
    expect(col, 3);
    expect(row! + 1, inInclusiveRange(1, 4)); // 1-based within 1..H
    expect(col! + 1, inInclusiveRange(1, 6)); // 1-based within 1..W

    // A tap in the surrounding border ring is a no-op.
    await tester.tapAt(topLeft + const Offset(20, 20));
    expect(callbacks, 1);
  });

  testWidgets('builds without error for a larger grid', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Center(
          child: SizedBox(
            width: 300,
            height: 300,
            child: BeadGridView(
              cells: List<int>.generate(20 * 20, (i) => i % 8),
              width: 20,
              height: 20,
              palette: [
                for (var i = 0; i < 8; i++)
                  PaletteColor(
                      code: 'S$i', name: 'C$i', rgb: Color(0xFF111111 * i)),
              ],
            ),
          ),
        ),
      ),
    );
    expect(find.byType(BeadGridView), findsOneWidget);
  });
}
