// Engine-on-iOS hard proof (tasks.md §3.1 / §3.2 / §6.2-iOS).
//
// Runs on a booted iOS Simulator. Proves the Rust `bead_ffi` staticlib is
// linked into the Runner AND its symbols survive dead-strip (the -force_load in
// ios/Flutter/bead_ffi.xcconfig): if either were wrong, `initBeadFfi()` /
// `generate` would throw a load / symbol-lookup error here.
//
// It does NOT compare bytes against bead-cli — iOS libm != host libm, so only
// the structural invariants are asserted (design risk §"跨目标浮点", Rule 3).
//
// iOS-only: this is a native-linkage proof; on other targets (e.g. Android,
// whose path is unverified this milestone) it would fail for the wrong reason.
@TestOn('ios')
library;

import 'package:beadsmith/infrastructure/bead_ffi_loader.dart';
import 'package:beadsmith/infrastructure/pattern_engine.dart';
import 'package:flutter/services.dart' show rootBundle;
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  const width = 16;
  const height = 20;
  const total = width * height; // 320

  testWidgets('bead_ffi loads on iOS and generate() returns a valid pattern',
      (tester) async {
    // 3.2: load the native bridge — throws on a missing/dead-stripped symbol.
    await initBeadFfi();

    final imageBytes =
        (await rootBundle.load('assets/test/gradient.png')).buffer.asUint8List();
    final paletteJson =
        await rootBundle.loadString('assets/palettes/artkal_s.json');

    // 3.2 (hard): generate actually returns a GenerateOutput on-device.
    final out = await const PatternEngine().generate(
      imageBytes: imageBytes,
      paletteJson: paletteJson,
      width: width,
      height: height,
    );

    // 6.2-iOS structural invariants.
    // Total beads is DERIVED (DTO has no total_beads field): cells.length.
    expect(out.pattern.width, width);
    expect(out.pattern.height, height);
    expect(out.pattern.cells.length, total);

    // stats schema: non-empty, each row has code/name/count; Σ count == total.
    expect(out.stats, isNotEmpty);
    var sum = 0;
    for (final s in out.stats) {
      expect(s.code, isNotEmpty);
      expect(s.name, isNotEmpty);
      expect(s.count, greaterThan(0));
      sum += s.count;
    }
    expect(sum, total, reason: 'Σ stats.count must equal width*height');

    // summary is the INIT format (statistics/mod.rs format string).
    expect(out.summary, startsWith('Bead Pattern Summary\n'));
    expect(out.summary, contains('Size: $width x $height\n'));
    expect(out.summary, contains('Total Beads: $total\n'));
    expect(out.summary, contains('Palette: '));

    // Engine emits a non-empty preview PNG (consumed verbatim by ResultPage).
    expect(out.previewPng, isNotEmpty);
  });
}
