import 'package:flutter/services.dart' show rootBundle;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../infrastructure/clipboard_service.dart';
import '../infrastructure/pattern_engine.dart';
import 'copy_summary.dart';
import 'generate_pattern.dart';

final patternEngineProvider =
    Provider<PatternEngine>((ref) => const PatternEngine());

final clipboardServiceProvider =
    Provider<ClipboardService>((ref) => const ClipboardService());

final generatePatternProvider = Provider<GeneratePattern>(
  (ref) => GeneratePattern(ref.watch(patternEngineProvider)),
);

final copySummaryProvider = Provider<CopySummary>(
  (ref) => CopySummary(ref.watch(clipboardServiceProvider)),
);

/// Bundled, offline default palette: `assets/palettes/artkal_s.json` read as a
/// String and passed to `generate` as `paletteJson` (no network — design D6).
final paletteJsonProvider = FutureProvider<String>(
  (ref) => rootBundle.loadString('assets/palettes/artkal_s.json'),
);
