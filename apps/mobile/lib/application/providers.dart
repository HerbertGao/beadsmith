import 'package:flutter/services.dart' show rootBundle;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../infrastructure/album_service.dart';
import '../infrastructure/clipboard_service.dart';
import '../infrastructure/palette_codec.dart' show PaletteColor, parsePalette;
import '../infrastructure/pattern_engine.dart';
import 'copy_summary.dart';
import 'generate_pattern.dart';
import 'save_to_album.dart';

final patternEngineProvider =
    Provider<PatternEngine>((ref) => const PatternEngine());

final clipboardServiceProvider =
    Provider<ClipboardService>((ref) => const ClipboardService());

final albumServiceProvider =
    Provider<AlbumService>((ref) => const AlbumService());

final generatePatternProvider = Provider<GeneratePattern>(
  (ref) => GeneratePattern(ref.watch(patternEngineProvider)),
);

final copySummaryProvider = Provider<CopySummary>(
  (ref) => CopySummary(ref.watch(clipboardServiceProvider)),
);

final saveToAlbumProvider = Provider<SaveToAlbum>(
  (ref) => SaveToAlbum(ref.watch(albumServiceProvider)),
);

/// Bundled, offline default palette: `assets/palettes/artkal_s.json` read as a
/// String and passed to `generate` as `paletteJson` (no network — design D6).
final paletteJsonProvider = FutureProvider<String>(
  (ref) => rootBundle.loadString('assets/palettes/artkal_s.json'),
);

/// Parsed palette in engine order — `BeadPattern.cells[i]` indexes this list.
/// Derived from [paletteJsonProvider] so the UI side and the engine always
/// see the same colors (single source of truth = the bundled JSON).
final paletteProvider = FutureProvider<List<PaletteColor>>(
  (ref) async => parsePalette(await ref.watch(paletteJsonProvider.future)),
);
