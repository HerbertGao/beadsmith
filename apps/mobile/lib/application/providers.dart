import 'dart:convert' show jsonDecode;

import 'package:flutter/services.dart' show rootBundle;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../infrastructure/album_service.dart';
import '../infrastructure/clipboard_service.dart';
import '../infrastructure/palette_registry.dart';
import '../infrastructure/pattern_engine.dart';
import 'copy_summary.dart';
import 'generate_pattern.dart';
import 'generate_settings.dart';
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

/// Bundled, offline palette for the CURRENTLY SELECTED id: its
/// `assets/palettes/{id}.json` read as a String and passed to `generate` as
/// `paletteJson` (no
/// network — design D6/D8). Stays a plain `FutureProvider<String>` (NOT `.family`)
/// so existing `.overrideWith` test seams keep working. An unknown persisted id
/// falls back to the default (MARD) via [paletteEntryOrDefault].
final paletteJsonProvider = FutureProvider<String>((ref) {
  final id = ref.watch(generateSettingsProvider.select((s) => s.paletteId));
  return rootBundle.loadString(paletteEntryOrDefault(id).asset);
});

/// `id → color count` (`colors.length`) for the palette bottom sheet's "N 色"
/// label. Lazily parses each bundled palette (14 small files); a parse failure
/// yields `null` for that id so the sheet can show a non-crashing placeholder
/// (e.g. "—") without bringing the sheet down. Brand comes from the registry
/// (synchronous), so only the count is parsed here — no hardcoded count to drift.
final paletteColorCountsProvider =
    FutureProvider<Map<String, int?>>((ref) async {
  final counts = <String, int?>{};
  for (final e in paletteRegistry) {
    try {
      final json = await rootBundle.loadString(e.asset);
      final root = jsonDecode(json) as Map<String, dynamic>;
      counts[e.id] = (root['colors'] as List<dynamic>).length;
    } catch (_) {
      counts[e.id] = null;
    }
  }
  return counts;
});
