// Task 6.3 — language switch + fallback-resolution semantics (spec 需求2 回落语义).
//
// (1) Widget switch: HomePage rendered under Locale('en') shows English, under
//     Locale('zh') shows Chinese — the l10n wiring actually swaps at runtime.
// (2) Fallback resolution via the app's real `supportedLocales` (zh pinned
//     first by preferred-supported-locales): a preferred list with NEITHER zh
//     NOR en ([fr, de]) → zh (default-Chinese guard); a list containing en
//     ([fr, en]) → en (user listed English; cross-platform-consistent, NOT zh).
//     This is the widget-layer resolution only — iOS native locale negotiation
//     is out of scope here (task 6.4 / manual).
import 'package:beadsmith/l10n/app_localizations.dart';
import 'package:beadsmith/presentation/home_page.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

Future<void> _pumpHome(WidgetTester tester, Locale locale) async {
  await tester.pumpWidget(
    ProviderScope(
      child: MaterialApp(
        locale: locale,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: const HomePage(),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('Locale(en) renders English UI', (tester) async {
    await _pumpHome(tester, const Locale('en'));
    expect(find.text('Bead Pattern Generator'), findsOneWidget); // homeHeadline
    expect(find.text('拼豆图纸生成器'), findsNothing);
  });

  testWidgets('Locale(zh) renders Chinese UI', (tester) async {
    await _pumpHome(tester, const Locale('zh'));
    expect(find.text('拼豆图纸生成器'), findsOneWidget);
    expect(find.text('Bead Pattern Generator'), findsNothing);
  });

  test('fallback: preferred list without zh/en resolves to Chinese', () {
    final resolved = basicLocaleListResolution(
      const [Locale('fr'), Locale('de')],
      AppLocalizations.supportedLocales,
    );
    expect(resolved, const Locale('zh')); // preferred-supported-locales first
  });

  test('fallback: preferred list containing en resolves to English (not zh)',
      () {
    final resolved = basicLocaleListResolution(
      const [Locale('fr'), Locale('en')],
      AppLocalizations.supportedLocales,
    );
    expect(resolved, const Locale('en')); // en listed → en, cross-platform rule
  });
}
