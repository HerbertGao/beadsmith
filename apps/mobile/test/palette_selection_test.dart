// Tasks 6.1 + 6.3 — palette selection reaches the bridge, and the result is
// pinned to the palette used at generation time.
//
// Anti-vacuous discipline (task 6.1 / 6.3): the REAL bundled asset for the
// selected id must flow through — `paletteJsonProvider` is NEVER overridden to a
// constant here (that would let a dead selection pass). Two genuinely different
// real palettes (MARD vs Hama) are asserted by their distinct codes/counts, so a
// swapped/ignored selection can't slip through.
import 'dart:io';
import 'dart:typed_data';

import 'package:beadsmith/application/generate_settings.dart';
import 'package:beadsmith/application/providers.dart' show patternEngineProvider;
import 'package:beadsmith/infrastructure/bead_bridge.dart'
    show BeadPattern, GenerateOutput, GeneratorKind;
import 'package:beadsmith/infrastructure/palette_codec.dart'
    show PaletteColor, parsePalette;
import 'package:beadsmith/infrastructure/pattern_engine.dart';
import 'package:beadsmith/l10n/app_localizations.dart';
import 'package:beadsmith/presentation/bead_grid_view.dart';
import 'package:beadsmith/presentation/generate_page.dart';
import 'package:beadsmith/presentation/result_page.dart';
import 'package:beadsmith/presentation/session_providers.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Records the `paletteJson` arg landed on the bridge, and returns a caller-set
/// [output]. The recorded `paletteJson` is the exact string `_generate` forwarded
/// to `generate` — the value the "palette reaches the bridge" check lands on.
class _RecordingBridge {
  _RecordingBridge([GenerateOutput? output]) : output = output ?? _out1x1();

  String? paletteJson;
  bool called = false;
  final GenerateOutput output;

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
    this.paletteJson = paletteJson;
    return output;
  }
}

List<String> _codes(List<PaletteColor> p) => [for (final c in p) c.code];

GenerateOutput _out1x1() => GenerateOutput(
      pattern: BeadPattern(width: 1, height: 1, cells: Uint16List(1)),
      stats: const [],
      summary: 's',
      brand: 'b',
      previewPng: _fixturePng,
      gridPng: _fixturePng,
      patternJson: '{}',
    );

/// 2×2, every cell → palette index 1. Rendering this in ResultPage and tapping a
/// cell shows `palette[1]` (MARD[1] = "A02"), which differs from Hama[1] = "H02":
/// the observable that proves the result used the PINNED palette, not the live one.
GenerateOutput _out2x2Index1() => GenerateOutput(
      pattern: BeadPattern(
          width: 2, height: 2, cells: Uint16List.fromList(const [1, 1, 1, 1])),
      stats: const [],
      summary: 's',
      brand: 'b',
      previewPng: _fixturePng,
      gridPng: _fixturePng,
      patternJson: '{}',
    );

Future<ProviderContainer> _pump(
  WidgetTester tester,
  _RecordingBridge fake, {
  Map<String, Object> initialPrefs = const {},
  Widget resultPage = const Scaffold(body: Text('result-placeholder')),
}) async {
  // Prefs-backed settings Notifier needs a preloaded instance or it throws
  // MissingPluginException the moment GeneratePage reads it.
  SharedPreferences.setMockInitialValues(initialPrefs);
  final prefs = await SharedPreferences.getInstance();
  final container = ProviderContainer(overrides: [
    sharedPreferencesProvider.overrideWithValue(prefs),
    patternEngineProvider.overrideWithValue(PatternEngine(gen: fake.gen)),
    // paletteJsonProvider is intentionally NOT overridden — the real
    // `assets/palettes/<selected id>.json` must reach the bridge.
  ]);
  addTearDown(container.dispose);
  container.read(croppedImageProvider.notifier).set(Uint8List.fromList([1, 2]));

  final router = GoRouter(
    initialLocation: '/generate',
    routes: [
      GoRoute(path: '/generate', builder: (_, _) => const GeneratePage()),
      GoRoute(path: '/result', builder: (_, _) => resultPage),
    ],
  );
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp.router(
        routerConfig: router,
        locale: const Locale('zh'), // finders below match the zh UI strings
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
      ),
    ),
  );
  return container;
}

Future<void> _tapGenerate(WidgetTester tester, _RecordingBridge fake) async {
  final generate = find.widgetWithText(FilledButton, '生成');
  await tester.ensureVisible(generate);
  // `paletteJsonProvider` runs a REAL `rootBundle.loadString` (task 6.1 forbids a
  // constant override). The tap MUST be dispatched inside runAsync so `_generate`
  // and its asset load run in the real zone — a tap in fake-async binds the load
  // to the fake clock, which runAsync can't advance (deadlock). Poll the real loop
  // until `_generate` reaches the bridge; pumps run AFTER to build the resulting UI.
  await tester.runAsync(() async {
    await tester.tap(generate);
    for (var i = 0; i < 200 && !fake.called; i++) {
      await Future<void>.delayed(const Duration(milliseconds: 5));
    }
  });
  await tester.pump();
  await tester.pump();
}

void main() {
  // Real palettes read straight off disk (== the bundled asset, guaranteed
  // byte-identical by palette_assets_test). The expected results for the bridge.
  final mard = parsePalette(File('assets/palettes/mard.json').readAsStringSync());
  final hama = parsePalette(File('assets/palettes/hama.json').readAsStringSync());

  test('sanity: MARD and Hama are genuinely different palettes', () {
    expect(mard.first.code, 'A01');
    expect(hama.first.code, 'H01');
    expect(mard.length, isNot(hama.length));
    expect(_codes(mard), isNot(_codes(hama)));
  });

  testWidgets('6.1 default (no persisted id) sends MARD to the bridge',
      (tester) async {
    final fake = _RecordingBridge();
    await _pump(tester, fake); // empty prefs → default MARD
    await _tapGenerate(tester, fake);

    expect(fake.called, isTrue);
    expect(_codes(parsePalette(fake.paletteJson!)), _codes(mard));
    expect(_codes(parsePalette(fake.paletteJson!)), isNot(_codes(hama)));
  });

  testWidgets('6.1 selecting Hama sends Hama (a second, different palette)',
      (tester) async {
    final fake = _RecordingBridge();
    await _pump(tester, fake, initialPrefs: {'settings.paletteId': 'hama'});
    await _tapGenerate(tester, fake);

    expect(fake.called, isTrue);
    expect(_codes(parsePalette(fake.paletteJson!)), _codes(hama));
    expect(_codes(parsePalette(fake.paletteJson!)), isNot(_codes(mard)));
  });

  testWidgets('6.1 an invalid persisted id falls back to MARD at the bridge',
      (tester) async {
    final fake = _RecordingBridge();
    await _pump(tester, fake,
        initialPrefs: {'settings.paletteId': 'does_not_exist'});
    await _tapGenerate(tester, fake);

    expect(fake.called, isTrue);
    expect(_codes(parsePalette(fake.paletteJson!)), _codes(mard));
  });

  testWidgets(
      '6.3 result stays pinned to A (MARD) after switching selection to B (Hama)',
      (tester) async {
    final fake = _RecordingBridge(_out2x2Index1());
    final container = await _pump(tester, fake,
        initialPrefs: {'settings.paletteId': 'mard'},
        resultPage: const ResultPage());

    // Generate with MARD selected → GeneratePage pushes /result (ResultPage).
    await _tapGenerate(tester, fake);

    // Provider layer (mirrors result_page.build: `parsePalette(result.paletteJson)`).
    final pinned = container.read(generateResultProvider)!.paletteJson;
    expect(_codes(parsePalette(pinned)), _codes(mard));
    expect(_codes(parsePalette(pinned)), isNot(_codes(hama)));

    // Change the live selection to Hama WITHOUT regenerating.
    container.read(generateSettingsProvider.notifier).setPaletteId('hama');
    await tester.pumpAndSettle();

    // Pinned palette is untouched by the live switch.
    expect(_codes(parsePalette(container.read(generateResultProvider)!.paletteJson)),
        _codes(mard));

    // Widget layer: ResultPage still renders the pinned MARD palette. Tapping a
    // cell (index 1) shows MARD[1] = "A02"; Hama[1] = "H02" would appear only if
    // the page had recolored to the live selection.
    await tester.tap(find.byType(BeadGridView));
    await tester.pumpAndSettle();
    expect(find.text('A02'), findsWidgets); // MARD colors[1].code/name
    expect(find.text('H02'), findsNothing); // Hama colors[1].code — must NOT show

    // Drain the once-per-session "save tip" 7s timer so no timer is left pending.
    await tester.pump(const Duration(seconds: 8));
  });
}

/// A minimal valid 1×1 PNG (red dot) — decodes cleanly for ResultPage's
/// `Image.memory` thumbnails/grid.
final Uint8List _fixturePng = Uint8List.fromList(const [
  137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, //
  0, 0, 0, 1, 8, 2, 0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, //
  120, 156, 99, 248, 207, 192, 0, 0, 3, 1, 1, 0, 201, 254, 146, 239, 0, 0, 0, //
  0, 73, 69, 78, 68, 174, 66, 96, 130,
]);
