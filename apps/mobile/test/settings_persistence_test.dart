// Task 6.2 — settings-page config persists across launches; height is
// re-derived (not persisted); an overflow rebase on entry is NOT written back;
// first launch falls back to defaults.
//
// Anti-vacuous discipline: the cross-launch case writes ONLY via the first
// container's Notifier, then reads via a freshly-built second container — the
// asserted values reach it solely through `shared_preferences`, never seeded
// directly into the second container.
import 'dart:typed_data';

import 'package:beadsmith/application/generate_settings.dart';
import 'package:beadsmith/infrastructure/bead_bridge.dart' show GeneratorKind;
import 'package:beadsmith/l10n/app_localizations.dart';
import 'package:beadsmith/presentation/generate_page.dart'
    show GeneratePage, lockedGridPair;
import 'package:beadsmith/presentation/session_providers.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Enter GeneratePage with [prefs] persisted and the crop at [aspect]; the aspect
/// is set on the container BEFORE `pumpWidget` so `initState` re-derives against
/// it. No bridge / paletteJson overrides — this exercises page ENTRY only.
Future<ProviderContainer> _pumpPage(
  WidgetTester tester, {
  required Map<String, Object> prefs,
  required double aspect,
}) async {
  SharedPreferences.setMockInitialValues(prefs);
  final sp = await SharedPreferences.getInstance();
  final container = ProviderContainer(overrides: [
    sharedPreferencesProvider.overrideWithValue(sp),
  ]);
  addTearDown(container.dispose);
  container.read(croppedImageProvider.notifier).set(Uint8List.fromList([1, 2]));
  container.read(cropAspectProvider.notifier).set(aspect); // before initState
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(
        locale: const Locale('zh'), // finders below match the zh UI strings
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: const GeneratePage(),
      ),
    ),
  );
  return container;
}

String _fieldText(WidgetTester tester, String label) =>
    tester.widget<TextField>(find.widgetWithText(TextField, label)).controller!.text;

void main() {
  test('6.2 edited config propagates to a rebuilt container (cross-launch)',
      () async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();

    // First launch: user edits every persisted field via the Notifier.
    final c1 = ProviderContainer(
        overrides: [sharedPreferencesProvider.overrideWithValue(prefs)]);
    final n = c1.read(generateSettingsProvider.notifier);
    n.setPaletteId('hama');
    n.setGenerator(GeneratorKind.gerstner);
    n.setLimitColors(true);
    n.setMaxColors(12);
    n.setDespeckleOn(true);
    n.setDespeckle(4);
    n.setWidth(64);
    n.setBorderRings(3);
    c1.dispose();

    // "Kill & reopen": a fresh container reading the SAME persisted store. The
    // values are NOT seeded here — they arrive only through the writes above.
    final prefs2 = await SharedPreferences.getInstance();
    final c2 = ProviderContainer(
        overrides: [sharedPreferencesProvider.overrideWithValue(prefs2)]);
    addTearDown(c2.dispose);
    final s = c2.read(generateSettingsProvider);

    expect(s.paletteId, 'hama');
    expect(s.generator, GeneratorKind.gerstner);
    expect(s.limitColors, isTrue);
    expect(s.maxColors, 12);
    expect(s.despeckleOn, isTrue);
    expect(s.despeckle, 4);
    expect(s.width, 64);
    expect(s.borderRings, 3); // anti-vacuous round-trip: written to c1, read from c2
  });

  test('6.2 stale over-limit borderRings clamps to kMaxBorderRings on read',
      () async {
    // A persisted value above the hard cap (e.g. predating a lower cap) must be
    // clamped by build()'s `.clamp(0, kMaxBorderRings)`, not read back verbatim.
    SharedPreferences.setMockInitialValues({'settings.borderRings': 99});
    final prefs = await SharedPreferences.getInstance();
    final c = ProviderContainer(
        overrides: [sharedPreferencesProvider.overrideWithValue(prefs)]);
    addTearDown(c.dispose);

    expect(c.read(generateSettingsProvider).borderRings, kMaxBorderRings);
  });

  test('6.2 first launch (no persisted values) falls back to defaults',
      () async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    final c = ProviderContainer(
        overrides: [sharedPreferencesProvider.overrideWithValue(prefs)]);
    addTearDown(c.dispose);

    expect(c.read(generateSettingsProvider), GenerateSettings.defaults);
    final s = c.read(generateSettingsProvider);
    expect(s.paletteId, 'mard');
    expect(s.generator, GeneratorKind.staged);
    expect(s.limitColors, isFalse);
    expect(s.despeckleOn, isFalse);
    expect(s.width, 100);
    expect(s.borderRings, 0);
  });

  testWidgets('6.2 height is re-derived from persisted width at the current aspect',
      (tester) async {
    // aspect 0.5 (portrait): width 100 stays; height = 100 / 0.5 = 200.
    await _pumpPage(tester, prefs: {'settings.width': 100}, aspect: 0.5);
    final (ew, eh) = lockedGridPair(100, 0.5, valueIsWidth: true);
    expect((ew, eh), (100, 200)); // guard the fixture itself
    expect(_fieldText(tester, '宽 (豆)'), '$ew');
    expect(_fieldText(tester, '高 (豆)'), '$eh');
  });

  testWidgets('6.2 persisted width overflowing at a narrower aspect scales the pair down',
      (tester) async {
    // aspect 0.05: 100 / 0.05 = 2000 > 1000 → whole pair scales down (50, 1000).
    await _pumpPage(tester, prefs: {'settings.width': 100}, aspect: 0.05);
    final (ew, eh) = lockedGridPair(100, 0.05, valueIsWidth: true);
    expect(ew, lessThan(100)); // overflow pushed the displayed width below 100
    expect(_fieldText(tester, '宽 (豆)'), '$ew');
    expect(_fieldText(tester, '高 (豆)'), '$eh');
  });

  testWidgets('6.2 entering under an overflowing aspect does NOT rewrite persisted width',
      (tester) async {
    final container =
        await _pumpPage(tester, prefs: {'settings.width': 100}, aspect: 0.05);

    // The displayed width was rebased down for THIS aspect...
    expect(_fieldText(tester, '宽 (豆)'), '50');

    // ...but leaving the page (dispose) must not persist that rebase (design D5):
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: SizedBox())),
      ),
    );

    expect(container.read(generateSettingsProvider).width, 100);
    expect(container.read(sharedPreferencesProvider).getInt('settings.width'), 100);
  });
}
