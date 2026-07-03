import 'package:beadsmith/presentation/generate_page.dart';
import 'package:flutter_test/flutter_test.dart';

/// Both sides in 1..1000, and the locked ratio held to <1 bead residual
/// (`width ≈ height * aspect`) — the engine's crop_center then trims <1 bead.
void expectLocked((int, int) pair, double aspect) {
  final (w, h) = pair;
  expect(w, inInclusiveRange(1, 1000), reason: 'w in 1..1000 ($pair)');
  expect(h, inInclusiveRange(1, 1000), reason: 'h in 1..1000 ($pair)');
  expect((w - h * aspect).abs(), lessThan(1.0),
      reason: 'ratio residual <1 bead ($pair @ $aspect)');
}

void main() {
  group('lockedGridPair', () {
    test('feasible width edit derives the other side, no rebase', () {
      expect(lockedGridPair(40, 1.0, valueIsWidth: true), (40, 40));
      expect(lockedGridPair(40, 3 / 4, valueIsWidth: true), (40, 53));
      expectLocked(lockedGridPair(40, 9 / 16, valueIsWidth: true), 9 / 16);
    });

    test('REGRESSION: 9:16 width=800 rebases the pair, never 800x1000', () {
      // Old code clamped height to 1000 and left width=800 → ratio 0.8, breaking
      // the lock and re-triggering engine crop_center. Must rebase instead.
      final pair = lockedGridPair(800, 9 / 16, valueIsWidth: true);
      expect(pair, isNot((800, 1000)));
      expectLocked(pair, 9 / 16); // both <=1000 AND ratio preserved
    });

    test('landscape 16:9 height=800 rebases symmetrically', () {
      final pair = lockedGridPair(800, 16 / 9, valueIsWidth: false);
      expect(pair, isNot((1000, 800)));
      expectLocked(pair, 16 / 9);
    });

    test('extreme value stays in bounds with ratio preserved', () {
      expectLocked(lockedGridPair(99999, 9 / 16, valueIsWidth: true), 9 / 16);
      expectLocked(lockedGridPair(99999, 16 / 9, valueIsWidth: false), 16 / 9);
    });

    test('smallest value floors at 1 on both sides', () {
      final (w, h) = lockedGridPair(1, 16 / 9, valueIsWidth: false);
      expect(w, greaterThanOrEqualTo(1));
      expect(h, greaterThanOrEqualTo(1));
    });
  });
}
