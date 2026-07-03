import 'package:flutter/material.dart';

/// Pegboard workshop design tokens + light/dark [ThemeData] for the app.
///
/// Tokens are the source of truth (not a seed). Screens read roles off
/// `Theme.of(context).colorScheme` — accent→primary, secondary→secondary,
/// ground→surface, ink→onSurface, line→outline.
class BeadTokens {
  const BeadTokens._();

  // Shared brand accents (identical in light + dark).
  static const Color accent = Color(0xFF6C4BF4);
  static const Color secondary = Color(0xFF12A594);

  // Light neutrals.
  static const Color lightInk = Color(0xFF1C1830); // text/data on ground
  static const Color lightGround = Color(0xFFF4F3F7); // page background
  static const Color lightSurface = Color(0xFFFFFFFF); // cards, raised over ground
  static const Color lightLine = Color(0xFFE6E3EF); // borders/dividers

  // Dark neutrals (flipped; ink-on-ground ≥ 4.5:1).
  static const Color darkGround = Color(0xFF141019); // page background
  static const Color darkSurface = Color(0xFF1E1830); // cards, slightly raised
  static const Color darkInk = Color(0xFFECEAF2); // text/data on ground
  static const Color darkLine = Color(0xFF2A2436); // low-contrast borders
}

/// Monospace style for bead codes/counts. Platform monospace fallback chain —
/// no bundled font asset. Leaves color null so it inherits from context.
const TextStyle monoTextStyle = TextStyle(
  fontFamily: 'monospace',
  fontFamilyFallback: <String>['Menlo', 'Courier New', 'monospace'],
  fontFeatures: <FontFeature>[FontFeature.tabularFigures()],
);

// Bead-like rounded controls: one radius drives cards/inputs/buttons.
const double _beadRadius = 18.0;
final BorderRadius _beadBorderRadius = BorderRadius.circular(_beadRadius);
final RoundedRectangleBorder _beadShape =
    RoundedRectangleBorder(borderRadius: _beadBorderRadius);

ThemeData _buildTheme(ColorScheme scheme) {
  final OutlineInputBorder inputBorder = OutlineInputBorder(
    borderRadius: _beadBorderRadius,
    borderSide: BorderSide(color: scheme.outline),
  );
  return ThemeData(
    useMaterial3: true,
    colorScheme: scheme,
    scaffoldBackgroundColor: scheme.surface,
    cardTheme: CardThemeData(
      shape: _beadShape,
      color: scheme.surfaceContainerHighest,
    ),
    inputDecorationTheme: InputDecorationTheme(
      border: inputBorder,
      enabledBorder: inputBorder,
      focusedBorder: OutlineInputBorder(
        borderRadius: _beadBorderRadius,
        borderSide: BorderSide(color: scheme.primary, width: 2),
      ),
    ),
    filledButtonTheme: FilledButtonThemeData(
      style: FilledButton.styleFrom(shape: _beadShape),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(shape: _beadShape),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(shape: _beadShape),
    ),
    segmentedButtonTheme: SegmentedButtonThemeData(
      style: SegmentedButton.styleFrom(shape: _beadShape),
    ),
  );
}

/// Light theme — pegboard tokens over a Material 3 scheme.
final ThemeData lightTheme = _buildTheme(
  ColorScheme.fromSeed(seedColor: BeadTokens.accent).copyWith(
    primary: BeadTokens.accent,
    onPrimary: Colors.white,
    secondary: BeadTokens.secondary,
    onSecondary: Colors.white,
    surface: BeadTokens.lightGround,
    onSurface: BeadTokens.lightInk,
    surfaceContainerHighest: BeadTokens.lightSurface,
    outline: BeadTokens.lightLine,
  ),
);

/// Dark theme — brand accents kept, neutrals flipped.
final ThemeData darkTheme = _buildTheme(
  ColorScheme.fromSeed(
    seedColor: BeadTokens.accent,
    brightness: Brightness.dark,
  ).copyWith(
    primary: BeadTokens.accent,
    onPrimary: Colors.white,
    secondary: BeadTokens.secondary,
    onSecondary: Colors.white,
    surface: BeadTokens.darkGround,
    onSurface: BeadTokens.darkInk,
    surfaceContainerHighest: BeadTokens.darkSurface,
    outline: BeadTokens.darkLine,
  ),
);
