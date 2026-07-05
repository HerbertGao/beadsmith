// Engine-on-Android hard proof — the parallel of engine_on_ios_test.dart for
// the Android path that M9 shipped as an unverified scaffold.
//
// Runs on a booted Android emulator (or device). Proves the Rust `bead_ffi`
// cdylib ships as `libbead_ffi.so` under jniLibs/<abi>/ AND loads by name via
// `ExternalLibrary.open('libbead_ffi.so')` (lib/infrastructure/
// bead_ffi_loader.dart, task 7.2): if the .so were missing from the APK, the
// wrong ABI, or the loader branch dead, `initBeadFfi()` / `generate` would
// throw a load / symbol-lookup error here.
//
// Like the iOS test, it does NOT compare bytes against bead-cli — Android's
// libm != host libm, so only the structural invariants are asserted (design
// risk §"跨目标浮点", Rule 3). The byte-exact CLI == FFI gate stays on the
// host side (crates/bead-ffi/dart).
//
// No @TestOn('android'): that selector makes `flutter test` silently skip the
// test at host-level pre-filter (same root cause as engine_on_ios_test.dart's
// @TestOn('ios') — see generate_ios_regression_test.dart's header). Target the
// Android device explicitly: `flutter test integration_test/
// engine_on_android_test.dart -d emulator-5554` (from `apps/mobile/`). A
// runtime `Platform.isAndroid` guard below fails the test LOUDLY if it is ever
// swept into a non-Android run — fail, not silently skip, is the point.
import 'dart:io' show Platform;

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

  testWidgets(
      'bead_ffi loads on Android and generate() returns a valid pattern',
      (tester) async {
    // Hard-guard: this is the Android-linkage proof. On any other platform
    // the loader throws UnsupportedError — fail loudly rather than pretend.
    if (!Platform.isAndroid) {
      throw StateError(
        'engine_on_android_test must run on Android '
        '(flutter test ... -d emulator-5554)',
      );
    }

    // 7.2: load the native bridge — throws if the .so is missing or the
    // wrong ABI shipped.
    await initBeadFfi();

    final imageBytes =
        (await rootBundle.load('assets/test/gradient.png')).buffer.asUint8List();
    final paletteJson =
        await rootBundle.loadString('assets/palettes/artkal_s.json');

    // generate actually returns a GenerateOutput on-device.
    final out = await const PatternEngine().generate(
      imageBytes: imageBytes,
      paletteJson: paletteJson,
      width: width,
      height: height,
    );

    // Structural invariants (same as the iOS test — not byte-exact).
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
