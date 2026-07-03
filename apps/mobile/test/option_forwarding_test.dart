// Task 5.1 — shell-level forwarding test.
//
// Guards the "dead control" bug: with default optional params, if _generate
// forgets to forward an option the control is written-but-never-read and the
// option stays dead. So we assert both directions through the FULL chain
// (_generate → GeneratePattern.call → PatternEngine.generate → bridge), landing
// on the injected fake bridge's own args:
//   * a SET option (generator=gerstner, max_colors=24, despeckle=3) arrives verbatim
//   * all-unset arrives as null / null / staged (byte-identical to the old default)
import 'dart:typed_data';

import 'package:beadsmith/application/providers.dart';
import 'package:beadsmith/infrastructure/bead_bridge.dart'
    show BeadPattern, GenerateOutput, GeneratorKind;
import 'package:beadsmith/infrastructure/pattern_engine.dart';
import 'package:beadsmith/presentation/generate_page.dart';
import 'package:beadsmith/presentation/session_providers.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';

/// Records the last args and returns a dummy output (no native lib needed).
class _FakeBridge {
  int? maxColors;
  int? despeckle;
  GeneratorKind? generator;
  bool called = false;

  Future<GenerateOutput> gen({
    required List<int> imageBytes,
    required String paletteJson,
    required int width,
    required int height,
    int? maxColors,
    int? despeckle,
    required GeneratorKind generator,
  }) async {
    called = true;
    this.maxColors = maxColors;
    this.despeckle = despeckle;
    this.generator = generator;
    return GenerateOutput(
      pattern: BeadPattern(width: 1, height: 1, cells: Uint16List(1)),
      stats: const [],
      summary: 's',
      brand: 'b',
      previewPng: Uint8List(0),
      gridPng: Uint8List(0),
      patternJson: '{}',
    );
  }
}

Future<ProviderContainer> _pumpGeneratePage(
  WidgetTester tester,
  _FakeBridge fake,
) async {
  final container = ProviderContainer(overrides: [
    patternEngineProvider.overrideWithValue(PatternEngine(gen: fake.gen)),
    paletteJsonProvider.overrideWith((ref) async => '{}'),
  ]);
  addTearDown(container.dispose);
  // A cropped image must be present or _generate bails early.
  container.read(croppedImageProvider.notifier).set(Uint8List.fromList([1, 2]));

  final router = GoRouter(
    initialLocation: '/generate',
    routes: [
      GoRoute(path: '/generate', builder: (_, _) => const GeneratePage()),
      GoRoute(
        path: '/result',
        builder: (_, _) => const Scaffold(body: Text('result')),
      ),
    ],
  );
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp.router(routerConfig: router),
    ),
  );
  return container;
}

void main() {
  testWidgets('set options reach the bridge verbatim', (tester) async {
    final fake = _FakeBridge();
    await _pumpGeneratePage(tester, fake);

    // The settings page scrolls; bring each control into view before tapping.
    Future<void> tapVisible(Finder f) async {
      await tester.ensureVisible(f);
      await tester.tap(f);
      await tester.pump();
    }

    // generator → gerstner (照片)
    await tapVisible(find.text('照片'));
    // max_colors → on (field defaults to 24)
    await tapVisible(find.widgetWithText(SwitchListTile, '限制颜色数'));
    // despeckle → on, then 3
    await tapVisible(find.widgetWithText(SwitchListTile, '去斑'));
    final despeckleField = find.widgetWithText(TextField, '阈值（豆）');
    await tester.ensureVisible(despeckleField);
    await tester.enterText(despeckleField, '3');

    await tapVisible(find.widgetWithText(FilledButton, '生成'));
    await tester.pumpAndSettle();

    expect(fake.called, isTrue);
    expect(fake.generator, GeneratorKind.gerstner);
    expect(fake.maxColors, 24);
    expect(fake.despeckle, 3);
  });

  testWidgets('unset options reach the bridge as null/null/staged',
      (tester) async {
    final fake = _FakeBridge();
    await _pumpGeneratePage(tester, fake);

    final generate = find.widgetWithText(FilledButton, '生成');
    await tester.ensureVisible(generate);
    await tester.tap(generate);
    await tester.pumpAndSettle();

    expect(fake.called, isTrue);
    expect(fake.generator, GeneratorKind.staged);
    expect(fake.maxColors, isNull);
    expect(fake.despeckle, isNull);
  });
}
