// Task 4.2 acceptance: the GeneratePattern use case returns a non-empty
// GenerateOutput. Skipped for the same native-lib reason as
// pattern_engine_test.dart.
@Skip('needs iOS-linked bead_ffi native lib / simulator')
library;

import 'dart:io';
import 'dart:typed_data';

import 'package:beadsmith/application/generate_pattern.dart';
import 'package:beadsmith/infrastructure/pattern_engine.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('GeneratePattern returns a non-empty GenerateOutput', () async {
    const useCase = GeneratePattern(PatternEngine());
    final out = await useCase.call(
      imageBytes: _fixturePng,
      paletteJson: File('assets/palettes/artkal_s.json').readAsStringSync(),
      width: 8,
      height: 8,
    );
    expect(out.pattern.cells.length, 8 * 8);
    expect(out.stats, isNotEmpty);
  });
}

/// A minimal valid 1×1 PNG (red dot) used as the fixture image.
final Uint8List _fixturePng = Uint8List.fromList(const [
  137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, //
  0, 0, 0, 1, 8, 2, 0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, //
  120, 156, 99, 248, 207, 192, 0, 0, 3, 1, 1, 0, 201, 254, 146, 239, 0, 0, 0, //
  0, 73, 69, 78, 68, 174, 66, 96, 130,
]);
