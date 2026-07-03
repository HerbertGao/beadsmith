import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:beadsmith/main.dart';
import 'package:beadsmith/presentation/theme.dart';

/// Group A (主题基座) — dark-wiring is testable; contrast itself is manual (5.2).
void main() {
  MaterialApp findApp(WidgetTester tester) =>
      tester.widget<MaterialApp>(find.byType(MaterialApp));

  Brightness resolvedBrightness(WidgetTester tester) =>
      Theme.of(tester.element(find.byType(Scaffold))).brightness;

  testWidgets('darkTheme wired, distinct from light, follows system',
      (tester) async {
    addTearDown(tester.platformDispatcher.clearPlatformBrightnessTestValue);

    // Start in light.
    tester.platformDispatcher.platformBrightnessTestValue = Brightness.light;
    await tester.pumpWidget(const ProviderScope(child: BeadsmithApp()));
    await tester.pumpAndSettle();

    final app = findApp(tester);
    expect(app.themeMode, ThemeMode.system);
    expect(app.theme, isNotNull);
    expect(app.darkTheme, isNotNull);
    // Dark scheme must actually differ from light (not a copy).
    expect(app.darkTheme!.colorScheme, isNot(equals(app.theme!.colorScheme)));
    expect(app.theme!.colorScheme.brightness, Brightness.light);
    expect(app.darkTheme!.colorScheme.brightness, Brightness.dark);

    // System is light → resolved theme is light.
    expect(resolvedBrightness(tester), Brightness.light);

    // Flip system → dark: themeMode:system must make it take effect.
    tester.platformDispatcher.platformBrightnessTestValue = Brightness.dark;
    await tester.pumpAndSettle();
    expect(resolvedBrightness(tester), Brightness.dark);
  });

  test('tokens map onto scheme roles', () {
    expect(lightTheme.colorScheme.primary, BeadTokens.accent);
    expect(lightTheme.colorScheme.secondary, BeadTokens.secondary);
    expect(lightTheme.colorScheme.surface, BeadTokens.lightGround);
    expect(lightTheme.colorScheme.onSurface, BeadTokens.lightInk);
    expect(lightTheme.colorScheme.outline, BeadTokens.lightLine);

    expect(darkTheme.colorScheme.primary, BeadTokens.accent);
    expect(darkTheme.colorScheme.surface, BeadTokens.darkGround);
    expect(darkTheme.colorScheme.onSurface, BeadTokens.darkInk);
    expect(darkTheme.colorScheme.outline, BeadTokens.darkLine);
  });
}
