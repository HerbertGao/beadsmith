import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../infrastructure/bead_bridge.dart' show GeneratorKind;
import '../infrastructure/palette_registry.dart' show kDefaultPaletteId;

/// Cross-launch settings-page config (design D4). A plain value object; the
/// [GenerateSettingsNotifier] owns persistence.
///
/// `width` is the horizontal bead count only — NOT the long side (design D5):
/// a portrait crop makes width the SHORT side, so persisting `max(w,h)` would
/// break the "store width, re-derive height by aspect" model. Height is never
/// persisted (re-derived on entry via lockedGridPair).
///
/// `maxColors` / `despeckle` hold the remembered field value; the `*On` toggle
/// decides whether it is forwarded (off ⇒ the shell sends `null`). They are
/// non-null so this stays a trivially-comparable value object with no nullable
/// copyWith ambiguity.
class GenerateSettings {
  const GenerateSettings({
    required this.paletteId,
    required this.generator,
    required this.limitColors,
    required this.maxColors,
    required this.despeckleOn,
    required this.despeckle,
    required this.width,
  });

  final String paletteId;
  final GeneratorKind generator;
  final bool limitColors;
  final int maxColors;
  final bool despeckleOn;
  final int despeckle;
  final int width;

  /// First-launch defaults (spec): MARD / staged / limit off / despeckle off /
  /// width 100. The 24 / 2 are the field values shown when a toggle is first
  /// switched on (they match the pre-existing controller defaults) — inert while
  /// their toggle is off, so generation stays byte-identical to the old path.
  static const GenerateSettings defaults = GenerateSettings(
    paletteId: kDefaultPaletteId,
    generator: GeneratorKind.staged,
    limitColors: false,
    maxColors: 24,
    despeckleOn: false,
    despeckle: 2,
    width: 100,
  );

  GenerateSettings copyWith({
    String? paletteId,
    GeneratorKind? generator,
    bool? limitColors,
    int? maxColors,
    bool? despeckleOn,
    int? despeckle,
    int? width,
  }) {
    return GenerateSettings(
      paletteId: paletteId ?? this.paletteId,
      generator: generator ?? this.generator,
      limitColors: limitColors ?? this.limitColors,
      maxColors: maxColors ?? this.maxColors,
      despeckleOn: despeckleOn ?? this.despeckleOn,
      despeckle: despeckle ?? this.despeckle,
      width: width ?? this.width,
    );
  }

  @override
  bool operator ==(Object other) =>
      other is GenerateSettings &&
      other.paletteId == paletteId &&
      other.generator == generator &&
      other.limitColors == limitColors &&
      other.maxColors == maxColors &&
      other.despeckleOn == despeckleOn &&
      other.despeckle == despeckle &&
      other.width == width;

  @override
  int get hashCode => Object.hash(paletteId, generator, limitColors, maxColors,
      despeckleOn, despeckle, width);
}

/// Injected `SharedPreferences`. `main()` pre-loads `getInstance()` and overrides
/// this with the instance BEFORE building `ProviderScope`, so the settings
/// Notifier reads persisted values synchronously on the first frame (no
/// not-ready window / no "default then overwrite" race — design D4). Left
/// unimplemented on purpose: an un-overridden read is a wiring bug, not a
/// runtime state.
final sharedPreferencesProvider = Provider<SharedPreferences>(
  (ref) => throw UnimplementedError(
      'sharedPreferencesProvider must be overridden with a preloaded '
      'SharedPreferences instance in main() (and in widget tests).'),
);

const String _kPaletteId = 'settings.paletteId';
const String _kGenerator = 'settings.generator';
const String _kLimitColors = 'settings.limitColors';
const String _kMaxColors = 'settings.maxColors';
const String _kDespeckleOn = 'settings.despeckleOn';
const String _kDespeckle = 'settings.despeckle';
const String _kWidth = 'settings.width';

/// Persisted settings-page state. Reads synchronously from the injected prefs in
/// [build]; every setter writes back immediately ("any field change ⇒ write").
///
/// Only user-driven setters call [_persist]. Seeding / re-deriving height on page
/// entry (group B) must NOT write width back — that path lives in GeneratePage,
/// not here, so this Notifier simply exposes an explicit `setWidth` for the
/// user-edit case and never auto-writes on read.
class GenerateSettingsNotifier extends Notifier<GenerateSettings> {
  @override
  GenerateSettings build() {
    final p = ref.watch(sharedPreferencesProvider);
    const d = GenerateSettings.defaults;
    return GenerateSettings(
      paletteId: p.getString(_kPaletteId) ?? d.paletteId,
      generator: _generatorFromName(p.getString(_kGenerator)),
      limitColors: p.getBool(_kLimitColors) ?? d.limitColors,
      maxColors: p.getInt(_kMaxColors) ?? d.maxColors,
      despeckleOn: p.getBool(_kDespeckleOn) ?? d.despeckleOn,
      despeckle: p.getInt(_kDespeckle) ?? d.despeckle,
      width: p.getInt(_kWidth) ?? d.width,
    );
  }

  static GeneratorKind _generatorFromName(String? name) {
    if (name == null) return GenerateSettings.defaults.generator;
    for (final g in GeneratorKind.values) {
      if (g.name == name) return g;
    }
    return GenerateSettings.defaults.generator;
  }

  void setPaletteId(String id) => _apply(state.copyWith(paletteId: id));
  void setGenerator(GeneratorKind g) => _apply(state.copyWith(generator: g));
  void setLimitColors(bool on) => _apply(state.copyWith(limitColors: on));
  void setMaxColors(int v) => _apply(state.copyWith(maxColors: v));
  void setDespeckleOn(bool on) => _apply(state.copyWith(despeckleOn: on));
  void setDespeckle(int v) => _apply(state.copyWith(despeckle: v));

  /// Persist an explicit, user-edited width (post aspect-lock / overflow). Seeding
  /// or re-derive-on-entry must NOT call this (design D5 write-back rule).
  void setWidth(int w) => _apply(state.copyWith(width: w));

  void _apply(GenerateSettings next) {
    if (next == state) return;
    state = next;
    _persist(next);
  }

  void _persist(GenerateSettings s) {
    final p = ref.read(sharedPreferencesProvider);
    p.setString(_kPaletteId, s.paletteId);
    p.setString(_kGenerator, s.generator.name);
    p.setBool(_kLimitColors, s.limitColors);
    p.setInt(_kMaxColors, s.maxColors);
    p.setBool(_kDespeckleOn, s.despeckleOn);
    p.setInt(_kDespeckle, s.despeckle);
    p.setInt(_kWidth, s.width);
  }
}

final generateSettingsProvider =
    NotifierProvider<GenerateSettingsNotifier, GenerateSettings>(
  GenerateSettingsNotifier.new,
);
