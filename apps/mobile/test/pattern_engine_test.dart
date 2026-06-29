// Task 4.1 acceptance: PatternEngine.generate returns a non-empty GenerateOutput
// for a fixed fixture.
//
// Skipped in this environment: `generate` resolves Rust symbols via
// `ExternalLibrary.process()`, which needs the bead_ffi staticlib linked into an
// iOS/Android Runner. The host Dart VM that runs `flutter test` has no such lib,
// so the call would fail with a load/symbol error. Un-skip on an iOS simulator
// (or a host build that links the dylib) to actually exercise it.
@Skip('needs iOS-linked bead_ffi native lib / simulator')
library;

import 'dart:io';
import 'dart:typed_data';

import 'package:beadsmith/infrastructure/pattern_engine.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('PatternEngine.generate returns a non-empty GenerateOutput', () async {
    const engine = PatternEngine();
    final out = await engine.generate(
      imageBytes: _fixturePng,
      paletteJson: File('assets/palettes/artkal_s.json').readAsStringSync(),
      width: 8,
      height: 8,
    );
    // Structural invariants (not byte-exact vs CLI — cross-target caveat).
    expect(out.pattern.cells.length, 8 * 8);
    expect(out.summary, isNotEmpty);
    expect(out.previewPng, isNotEmpty);
  });
}

/// A minimal valid 1×1 PNG (red dot) used as the fixture image.
final Uint8List _fixturePng = Uint8List.fromList(const [
  137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, //
  0, 0, 0, 1, 8, 2, 0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, //
  120, 156, 99, 248, 207, 192, 0, 0, 3, 1, 1, 0, 201, 254, 146, 239, 0, 0, 0, //
  0, 73, 69, 78, 68, 174, 66, 96, 130,
]);
