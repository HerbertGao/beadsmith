// Regression guard for the iOS FRB SSE panic: `GenerateOutput` must NOT hold
// `#[frb(mirror)]` types. A mirror type in the RESPONSE routes `generate`
// through an FRB SSE codec path that panics on the iOS target with a +6-byte
// accounting mismatch (flutter_rust_bridge codec/sse.rs:129) — for ANY image,
// so even this tiny fixture reproduces it if the DTOs regress to mirrors.
//
// No @TestOn('ios'): that selector made engine_on_ios_test.dart silently never
// run, which is exactly how the bug shipped undetected. Runs on any booted
// device; the panic is iOS-specific but the assertion is harmless elsewhere.
import 'dart:typed_data';
import 'package:beadsmith/infrastructure/bead_ffi_loader.dart';
import 'package:beadsmith/infrastructure/pattern_engine.dart';
import 'package:flutter/services.dart' show rootBundle;
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:image/image.dart' as img;

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets('generate() returns a valid pattern on-device (no SSE panic)',
      (tester) async {
    await initBeadFfi();
    final palette =
        await rootBundle.loadString('assets/palettes/artkal_s.json');
    final im = img.Image(width: 200, height: 200, numChannels: 3);
    for (final p in im) {
      final n = p.x * 131 + p.y * 977 + p.x * p.y * 31;
      p..r = n % 251..g = (n * 7) % 253..b = (n * 13) % 249;
    }
    final png = Uint8List.fromList(img.encodePng(im));
    final out = await const PatternEngine()
        .generate(imageBytes: png, paletteJson: palette, width: 40, height: 40);
    expect(out.pattern.width, 40);
    expect(out.pattern.height, 40);
    expect(out.pattern.cells.length, 1600);
    expect(out.stats, isNotEmpty);
    expect(out.previewPng, isNotEmpty);
  });
}
